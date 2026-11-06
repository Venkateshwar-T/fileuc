use fileuc_convert::archive::create_zip;
use fileuc_convert::image::{convert_image, ImageOptions};
use fileuc_convert::media::{compress_video, extract_audio, find_ffmpeg, video_to_gif};
use fileuc_convert::pdf::{compress_pdf, images_to_pdf, merge_pdfs, split_pdf};
use fileuc_settings::store::{AppSettings, OutputMode};
use fileuc_types::{ConversionAction, ConvertError, ImageFormat};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

#[derive(Debug, Clone)]
pub struct ConversionRequest {
    pub files: Vec<PathBuf>,
    pub action: ConversionAction,
    pub settings: Arc<AppSettings>,
}

#[derive(Debug, Clone, Default)]
pub struct ConversionResult {
    pub outputs: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, ConvertError)>,
}

pub fn dispatch(
    request: ConversionRequest,
    on_progress: impl Fn(f32) + Send + 'static,
) -> JoinHandle<ConversionResult> {
    std::thread::spawn(move || {
        let total = request.files.len().max(1) as f32;
        let mut result = ConversionResult::default();

        match request.action {
            ConversionAction::ToPng
            | ConversionAction::ToJpg
            | ConversionAction::ToWebp
            | ConversionAction::ToBmp
            | ConversionAction::ToIco
            | ConversionAction::ToTiff => {
                let fmt = match request.action {
                    ConversionAction::ToPng => ImageFormat::Png,
                    ConversionAction::ToJpg => ImageFormat::Jpeg,
                    ConversionAction::ToWebp => ImageFormat::WebP,
                    ConversionAction::ToBmp => ImageFormat::Bmp,
                    ConversionAction::ToIco => ImageFormat::Ico,
                    ConversionAction::ToTiff => ImageFormat::Tiff,
                    _ => unreachable!(),
                };
                let opts = ImageOptions::default();
                for (i, file) in request.files.iter().enumerate() {
                    match convert_image(file, fmt, &opts) {
                        Ok(out) => {
                            let final_out = apply_output_mode(out, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::ToGrayscale => {
                let opts = ImageOptions {
                    grayscale: true,
                    ..Default::default()
                };
                for (i, file) in request.files.iter().enumerate() {
                    match convert_image(file, ImageFormat::Png, &opts) {
                        Ok(out) => {
                            let final_out = apply_output_mode(out, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::Resize => {
                let opts = ImageOptions {
                    resize: Some(fileuc_convert::image::ResizeSpec {
                        width: 1920,
                        height: 1080,
                    }),
                    ..Default::default()
                };
                for (i, file) in request.files.iter().enumerate() {
                    match convert_image(file, ImageFormat::Png, &opts) {
                        Ok(out) => {
                            let final_out = apply_output_mode(out, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::SvgToPng => {
                for (i, file) in request.files.iter().enumerate() {
                    match fileuc_convert::svg::render_svg(file, ImageFormat::Png, None) {
                        Ok(out) => {
                            let final_out = apply_output_mode(out, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::SvgToJpg => {
                for (i, file) in request.files.iter().enumerate() {
                    match fileuc_convert::svg::render_svg(file, ImageFormat::Jpeg, None) {
                        Ok(out) => {
                            let final_out = apply_output_mode(out, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::ToPdf => {
                let first = &request.files[0];
                let out_pdf = fileuc_convert::output::resolve_output_path(first, "pdf");
                match images_to_pdf(&request.files, &out_pdf) {
                    Ok(()) => {
                        let final_out = apply_output_mode(out_pdf, &request.settings.output_mode);
                        result.outputs.push(final_out);
                    }
                    Err(e) => result.errors.push((first.clone(), e)),
                }
                on_progress(1.0);
            }
            ConversionAction::MergePdf => {
                let first = &request.files[0];
                let out_pdf = fileuc_convert::output::resolve_output_path(first, "pdf");
                match merge_pdfs(&request.files, &out_pdf) {
                    Ok(()) => {
                        let final_out = apply_output_mode(out_pdf, &request.settings.output_mode);
                        result.outputs.push(final_out);
                    }
                    Err(e) => result.errors.push((first.clone(), e)),
                }
                on_progress(1.0);
            }
            ConversionAction::SplitPdf => {
                for (i, file) in request.files.iter().enumerate() {
                    let parent = file.parent().unwrap_or_else(|| Path::new(""));
                    let split_dir = parent.join(format!(
                        "{}_pages",
                        file.file_stem().unwrap_or_default().to_string_lossy()
                    ));
                    match split_pdf(file, &split_dir) {
                        Ok(mut pages) => result.outputs.append(&mut pages),
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::CompressPdf => {
                for (i, file) in request.files.iter().enumerate() {
                    let out_pdf = fileuc_convert::output::resolve_output_path(file, "pdf");
                    match compress_pdf(file, &out_pdf) {
                        Ok(()) => {
                            let final_out = apply_output_mode(out_pdf, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::CreateZip => {
                let first = &request.files[0];
                let out_zip = fileuc_convert::output::resolve_output_path(first, "zip");
                match create_zip(&request.files, &out_zip) {
                    Ok(()) => {
                        let final_out = apply_output_mode(out_zip, &request.settings.output_mode);
                        result.outputs.push(final_out);
                    }
                    Err(e) => result.errors.push((first.clone(), e)),
                }
                on_progress(1.0);
            }
            ConversionAction::ExtractArchive => {
                for (i, file) in request.files.iter().enumerate() {
                    let parent = file.parent().unwrap_or_else(|| Path::new(""));
                    let extract_dir = parent.join(
                        file.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    );
                    match fileuc_convert::archive::extract_archive(file, &extract_dir) {
                        Ok(()) => result.outputs.push(extract_dir),
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::VideoToGif => {
                let ffmpeg = find_ffmpeg().unwrap_or_else(|| PathBuf::from("ffmpeg"));
                for (i, file) in request.files.iter().enumerate() {
                    let out_gif = fileuc_convert::output::resolve_output_path(file, "gif");
                    match video_to_gif(&ffmpeg, file, &out_gif) {
                        Ok(()) => {
                            let final_out = apply_output_mode(out_gif, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::VideoToMp3 => {
                let ffmpeg = find_ffmpeg().unwrap_or_else(|| PathBuf::from("ffmpeg"));
                for (i, file) in request.files.iter().enumerate() {
                    let out_mp3 = fileuc_convert::output::resolve_output_path(file, "mp3");
                    match extract_audio(&ffmpeg, file, &out_mp3) {
                        Ok(()) => {
                            let final_out = apply_output_mode(out_mp3, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::CompressVideo => {
                let ffmpeg = find_ffmpeg().unwrap_or_else(|| PathBuf::from("ffmpeg"));
                for (i, file) in request.files.iter().enumerate() {
                    let ext = file.extension().unwrap_or_default().to_string_lossy();
                    let out_vid = fileuc_convert::output::resolve_output_path(file, &ext);
                    match compress_video(&ffmpeg, file, &out_vid) {
                        Ok(()) => {
                            let final_out = apply_output_mode(out_vid, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            _ => {}
        }

        result
    })
}

fn apply_output_mode(path: PathBuf, mode: &OutputMode) -> PathBuf {
    match mode {
        OutputMode::NextToOriginal => path,
        OutputMode::CustomFolder(folder) => {
            if let Some(name) = path.file_name() {
                let target = folder.join(name);
                if std::fs::rename(&path, &target).is_err() {
                    let _ = std::fs::copy(&path, &target);
                    let _ = std::fs::remove_file(&path);
                }
                target
            } else {
                path
            }
        }
    }
}
