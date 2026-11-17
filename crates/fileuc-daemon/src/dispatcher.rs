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
        let mut result = ConversionResult::default();
        if request.files.is_empty() {
            on_progress(1.0);
            return result;
        }

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
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    convert_image(file, fmt, &opts)
                });
            }
            ConversionAction::ToGrayscale => {
                let opts = ImageOptions {
                    grayscale: true,
                    ..Default::default()
                };
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    convert_image(file, ImageFormat::Png, &opts)
                });
            }
            ConversionAction::ToJpeg => {
                let opts = ImageOptions {
                    custom_ext: Some("jpeg"),
                    ..Default::default()
                };
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    convert_image(file, ImageFormat::Jpeg, &opts)
                });
            }
            ConversionAction::ToSvg => {
                process_with_resolved_output(&request.files, "svg", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::image::image_to_svg(src, dst)
                });
            }
            ConversionAction::DocToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::doc::doc_to_pdf(src, dst)
                });
            }
            ConversionAction::PdfToDocx => {
                process_with_resolved_output(&request.files, "docx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::doc::pdf_to_docx(src, dst)
                });
            }
            ConversionAction::PptxToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pptx::pptx_to_pdf(src, dst)
                });
            }
            ConversionAction::MdToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::text::md_to_pdf(src, dst)
                });
            }
            ConversionAction::TxtToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::text::txt_to_pdf(src, dst)
                });
            }
            ConversionAction::HtmlToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::text::html_to_pdf(src, dst)
                });
            }
            ConversionAction::ExcelToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::excel::excel_to_pdf(src, dst)
                });
            }
            ConversionAction::PdfToExcel => {
                process_with_resolved_output(&request.files, "xlsx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::excel::pdf_to_excel(src, dst)
                });
            }
            ConversionAction::PdfOcr => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::ocr_searchable::pdf_to_searchable_pdf(src, dst)
                });
            }
            ConversionAction::Resize => {
                let opts = ImageOptions {
                    resize: Some(fileuc_convert::image::ResizeSpec {
                        width: 1920,
                        height: 1080,
                    }),
                    ..Default::default()
                };
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    convert_image(file, ImageFormat::Png, &opts)
                });
            }
            ConversionAction::SvgToPng => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    fileuc_convert::svg::render_svg(file, ImageFormat::Png, None)
                });
            }
            ConversionAction::SvgToJpg => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    fileuc_convert::svg::render_svg(file, ImageFormat::Jpeg, None)
                });
            }
            ConversionAction::SvgToWebp => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    fileuc_convert::svg::render_svg(file, ImageFormat::WebP, None)
                });
            }
            ConversionAction::ToPdf => {
                on_progress(0.0);
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
                on_progress(0.0);
                let first = &request.files[0];
                let out_pdf = fileuc_convert::output::resolve_suffixed_output_path(first, "_merged", "pdf", None);
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
                let total = request.files.len().max(1) as f32;
                for (i, file) in request.files.iter().enumerate() {
                    on_progress(i as f32 / total);
                    let parent = file.parent().unwrap_or_else(|| Path::new(""));
                    let split_dir = parent.join(format!(
                        "{}_pages",
                        file.file_stem().unwrap_or_default().to_string_lossy()
                    ));
                    match split_pdf(file, &split_dir) {
                        Ok(pages) => {
                            let final_dir = apply_output_mode(split_dir, &request.settings.output_mode);
                            for page in pages {
                                if let Some(file_name) = page.file_name() {
                                    result.outputs.push(final_dir.join(file_name));
                                } else {
                                    result.outputs.push(page);
                                }
                            }
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::PdfToImages => {
                let total = request.files.len().max(1) as f32;
                for (i, file) in request.files.iter().enumerate() {
                    on_progress(i as f32 / total);
                    let parent = file.parent().unwrap_or_else(|| Path::new(""));
                    let stem = file
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let out_dir = parent.join(&stem);

                    match fileuc_convert::pdfium::render_pdf_to_images(file, &out_dir, "png") {
                        Ok(pages) => {
                            let final_dir = apply_output_mode(out_dir, &request.settings.output_mode);
                            for page in pages {
                                if let Some(file_name) = page.file_name() {
                                    result.outputs.push(final_dir.join(file_name));
                                } else {
                                    result.outputs.push(page);
                                }
                            }
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }
            }
            ConversionAction::CompressPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    compress_pdf(src, dst)
                });
            }
            ConversionAction::LockPdf => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |_file| {
                    Err(ConvertError::ConverterError(
                        "LockPdf requires interactive user password specification".to_string(),
                    ))
                });
            }
            ConversionAction::UnlockPdf => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    let out_pdf = fileuc_convert::output::resolve_suffixed_output_path(file, "_unlocked", "pdf", Some("_locked"));
                    match fileuc_convert::pdf::unlock_pdf(file, None, &out_pdf)? {
                        true => Ok(out_pdf),
                        false => Err(ConvertError::ConverterError(
                            "This document is highly secured and cannot be unlocked without a valid password.".to_string(),
                        )),
                    }
                });
            }
            ConversionAction::CreateZip | ConversionAction::OneZip => {
                let first = &request.files[0];
                let out_zip = fileuc_convert::output::resolve_suffixed_output_path(first, "_zipped", "zip", None);
                match fileuc_convert::archive::create_zip_with_progress(&request.files, &out_zip, |p| {
                    on_progress(p);
                }) {
                    Ok(()) => {
                        let final_out = apply_output_mode(out_zip, &request.settings.output_mode);
                        result.outputs.push(final_out);
                    }
                    Err(e) => result.errors.push((first.clone(), e)),
                }
                on_progress(1.0);
            }
            ConversionAction::Create7z => {
                let first = &request.files[0];
                let out_7z = fileuc_convert::output::resolve_suffixed_output_path(first, "_zipped", "7z", None);
                match fileuc_convert::archive::create_7z_with_progress(&request.files, &out_7z, |p| {
                    on_progress(p);
                }) {
                    Ok(()) => {
                        let final_out = apply_output_mode(out_7z, &request.settings.output_mode);
                        result.outputs.push(final_out);
                    }
                    Err(e) => result.errors.push((first.clone(), e)),
                }
                on_progress(1.0);
            }
            ConversionAction::ExtractArchive => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    let parent = file.parent().unwrap_or_else(|| Path::new(""));
                    let extract_dir = parent.join(
                        file.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .as_ref(),
                    );
                    fileuc_convert::archive::extract_archive(file, &extract_dir)?;
                    Ok(extract_dir)
                });
            }
            ConversionAction::VideoToGif => {
                let ffmpeg = find_ffmpeg().unwrap_or_else(|| PathBuf::from("ffmpeg"));
                process_with_resolved_output(&request.files, "gif", &request.settings, &on_progress, &mut result, |src, dst| {
                    video_to_gif(&ffmpeg, src, dst)
                });
            }
            ConversionAction::VideoToMp3 => {
                let ffmpeg = find_ffmpeg().unwrap_or_else(|| PathBuf::from("ffmpeg"));
                process_with_resolved_output(&request.files, "mp3", &request.settings, &on_progress, &mut result, |src, dst| {
                    extract_audio(&ffmpeg, src, dst)
                });
            }
            ConversionAction::CompressVideo => {
                let ffmpeg = find_ffmpeg().unwrap_or_else(|| PathBuf::from("ffmpeg"));
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    let ext = file.extension().unwrap_or_default().to_string_lossy();
                    let out_vid = fileuc_convert::output::resolve_output_path(file, &ext);
                    compress_video(&ffmpeg, file, &out_vid)?;
                    Ok(out_vid)
                });
            }
            ConversionAction::ToHtml => {
                process_with_resolved_output(&request.files, "html", &request.settings, &on_progress, &mut result, |src, dst| {
                    let ext = src
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_ascii_lowercase())
                        .unwrap_or_default();
                    if ext == "md" || ext == "markdown" {
                        fileuc_convert::text::md_to_html(src, dst)
                    } else {
                        fileuc_convert::text::txt_to_html(src, dst)
                    }
                });
            }
            ConversionAction::XlsxToCsv => {
                process_with_resolved_output(&request.files, "csv", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::data::xlsx_to_csv(src, dst)
                });
            }
            ConversionAction::XlsxToJson => {
                process_with_resolved_output(&request.files, "json", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::data::xlsx_to_json(src, dst)
                });
            }
            ConversionAction::CsvToXlsx => {
                process_with_resolved_output(&request.files, "xlsx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::data::csv_to_xlsx(src, dst)
                });
            }
            ConversionAction::CsvToJson => {
                process_with_resolved_output(&request.files, "json", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::data::csv_to_json(src, dst)
                });
            }
            ConversionAction::JsonToXlsx => {
                process_with_resolved_output(&request.files, "xlsx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::data::json_to_xlsx(src, dst)
                });
            }
            ConversionAction::JsonToCsv => {
                process_with_resolved_output(&request.files, "csv", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::data::json_to_csv(src, dst)
                });
            }
            ConversionAction::MdToTxt => {
                process_with_resolved_output(&request.files, "txt", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::text::markdown_to_text(src, dst)
                });
            }
            ConversionAction::TxtToMd => {
                process_with_resolved_output(&request.files, "md", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::text::text_to_markdown(src, dst)
                });
            }
            ConversionAction::StripMetadata => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    fileuc_convert::metadata_strip::strip_metadata(file, None)
                });
            }
            ConversionAction::ToMp3
            | ConversionAction::ToWav
            | ConversionAction::ToOgg
            | ConversionAction::ToAac
            | ConversionAction::ToFlac => {
                let target_ext = match request.action {
                    ConversionAction::ToMp3 => "mp3",
                    ConversionAction::ToWav => "wav",
                    ConversionAction::ToOgg => "ogg",
                    ConversionAction::ToAac => "aac",
                    ConversionAction::ToFlac => "flac",
                    _ => "mp3",
                };
                if let Some(ffmpeg) = fileuc_convert::media::find_ffmpeg() {
                    process_with_resolved_output(&request.files, target_ext, &request.settings, &on_progress, &mut result, |src, dst| {
                        fileuc_convert::media::convert_audio(&ffmpeg, src, dst, target_ext)
                    });
                } else {
                    for f in &request.files {
                        result.errors.push((f.clone(), ConvertError::EngineMissing("FFmpeg is required for audio conversions".to_string())));
                    }
                }
            }
            ConversionAction::OfficeGroup => {
                process_with_resolved_output(&request.files, "docx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::doc::pdf_to_docx(src, dst)
                });
            }
            _ => {}
        }

        result
    })
}

fn process_each_file<F>(
    files: &[PathBuf],
    settings: &AppSettings,
    on_progress: &(impl Fn(f32) + ?Sized),
    result: &mut ConversionResult,
    mut convert_fn: F,
) where
    F: FnMut(&Path) -> Result<PathBuf, ConvertError>,
{
    let total = files.len().max(1) as f32;
    for (i, file) in files.iter().enumerate() {
        on_progress(i as f32 / total);
        match convert_fn(file) {
            Ok(out) => {
                let final_out = apply_output_mode(out, &settings.output_mode);
                result.outputs.push(final_out);
            }
            Err(e) => result.errors.push((file.clone(), e)),
        }
        on_progress((i + 1) as f32 / total);
    }
}

fn process_with_resolved_output<F>(
    files: &[PathBuf],
    ext: &str,
    settings: &AppSettings,
    on_progress: &(impl Fn(f32) + ?Sized),
    result: &mut ConversionResult,
    mut convert_fn: F,
) where
    F: FnMut(&Path, &Path) -> Result<(), ConvertError>,
{
    process_each_file(files, settings, on_progress, result, |file| {
        let out_path = match &settings.output_mode {
            OutputMode::NextToOriginal => fileuc_convert::output::resolve_output_path(file, ext),
            OutputMode::CustomFolder(folder) => {
                let _ = std::fs::create_dir_all(folder);
                let stem = file.file_stem().unwrap_or_default();
                let initial_name = format!("{}.{}", stem.to_string_lossy(), ext);
                resolve_target_path(folder, std::ffi::OsStr::new(&initial_name))
            }
        };
        match convert_fn(file, &out_path) {
            Ok(_) => Ok(out_path),
            Err(e) => {
                if out_path.exists() {
                    let _ = std::fs::remove_file(&out_path);
                }
                Err(e)
            }
        }
    });
}

pub fn apply_output_mode(path: PathBuf, mode: &OutputMode) -> PathBuf {
    match mode {
        OutputMode::NextToOriginal => path,
        OutputMode::CustomFolder(folder) => {
            if let Some(name) = path.file_name() {
                let _ = std::fs::create_dir_all(folder);
                let initial_target = folder.join(name);
                if path == initial_target {
                    return path;
                }
                let target = resolve_target_path(folder, name);
                if std::fs::rename(&path, &target).is_err() {
                    if path.is_dir() {
                        if copy_dir_all(&path, &target).is_ok() {
                            let _ = std::fs::remove_dir_all(&path);
                        }
                    } else if path.is_file() {
                        if std::fs::copy(&path, &target).is_ok() {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
                target
            } else {
                path
            }
        }
    }
}

fn resolve_target_path(folder: &Path, filename: &std::ffi::OsStr) -> PathBuf {
    let p = folder.join(filename);
    if !p.exists() {
        return p;
    }
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let ext = path.extension().and_then(|s| s.to_str());
    let mut counter = 2;
    loop {
        let candidate_name = match ext {
            Some(e) => format!("{} ({}).{}", stem, counter, e),
            None => format!("{} ({})", stem, counter),
        };
        let candidate = folder.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn dispatch_reduce_size(
    files: Vec<PathBuf>,
    settings: Arc<AppSettings>,
    target_bytes: u64,
    progress_val: Arc<std::sync::atomic::AtomicU32>,
) -> JoinHandle<ConversionResult> {
    std::thread::spawn(move || {
        progress_val.store(200, std::sync::atomic::Ordering::Relaxed);
        let mut outputs = Vec::new();
        let mut errors = Vec::new();
        let total = files.len();
        for (idx, src_file) in files.into_iter().enumerate() {
            match fileuc_convert::size_reducer::reduce_size(&src_file, None, target_bytes) {
                Ok(out_path) => {
                    let final_out = apply_output_mode(out_path, &settings.output_mode);
                    outputs.push(final_out);
                }
                Err(e) => {
                    errors.push((src_file, e));
                }
            }
            let progress = ((idx + 1) as f32 / total as f32) * 800.0 + 200.0;
            progress_val.store(progress as u32, std::sync::atomic::Ordering::Relaxed);
        }
        progress_val.store(1000, std::sync::atomic::Ordering::Relaxed);
        ConversionResult { outputs, errors }
    })
}
