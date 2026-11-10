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
        let total = request.files.len() as f32;

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
            ConversionAction::PdfToDocx => {
                process_with_resolved_output(&request.files, "docx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pdf::pdf_to_docx(src, dst)
                });
            }
            ConversionAction::PdfToDoc => {
                process_with_resolved_output(&request.files, "doc", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pdf::pdf_to_doc(src, dst)
                });
            }
            ConversionAction::DocToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::doc::doc_to_pdf(src, dst)
                });
            }
            ConversionAction::PdfToPptx => {
                process_with_resolved_output(&request.files, "pptx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pptx::pdf_to_pptx(src, dst)
                });
            }
            ConversionAction::PdfToXlsx => {
                process_with_resolved_output(&request.files, "xlsx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pdf::pdf_to_xlsx(src, dst)
                });
            }
            ConversionAction::PptxToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pptx::pptx_to_pdf(src, dst)
                });
            }
            ConversionAction::DocxToPptx => {
                process_with_resolved_output(&request.files, "pptx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pptx::docx_to_pptx(src, dst)
                });
            }
            ConversionAction::PptxToDocx => {
                process_with_resolved_output(&request.files, "docx", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::pptx::pptx_to_docx(src, dst)
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
            ConversionAction::ExcelToPdf => {
                process_with_resolved_output(&request.files, "pdf", &request.settings, &on_progress, &mut result, |src, dst| {
                    fileuc_convert::excel::excel_to_pdf(src, dst)
                });
            }
            ConversionAction::LockPdf => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    let out_pdf = fileuc_convert::output::resolve_suffixed_output_path(file, "_locked", "pdf", None);
                    fileuc_convert::pdf::lock_pdf(file, "fileuc", &out_pdf)?;
                    Ok(out_pdf)
                });
            }
            ConversionAction::UnlockPdf => {
                process_each_file(&request.files, &request.settings, &on_progress, &mut result, |file| {
                    let out_pdf = fileuc_convert::output::resolve_suffixed_output_path(file, "_unlocked", "pdf", Some("_locked"));
                    match fileuc_convert::pdf::unlock_pdf(file, None, &out_pdf)? {
                        true => Ok(out_pdf),
                        false => Err(ConvertError::ConverterError("Password required to unlock".to_string())),
                    }
                });
            }
            ConversionAction::CreateZip | ConversionAction::OneZip => {
                let first = &request.files[0];
                let out_zip = fileuc_convert::output::resolve_output_path(first, "zip");
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
                let out_7z = fileuc_convert::output::resolve_output_path(first, "7z");
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
            ConversionAction::Ocr => {
                let mut combined_text = String::new();
                for (i, file) in request.files.iter().enumerate() {
                    on_progress(i as f32 / total);
                    let out_txt = fileuc_convert::output::resolve_output_path(file, "txt");
                    let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    let res = if ext == "pdf" {
                        match fileuc_convert::pdf::extract_pdf_paragraphs(file) {
                            Ok(paras) => {
                                let mut text = paras.join("\r\n\r\n");
                                if text.starts_with("Document: ") && text.lines().count() <= 1 {
                                    if let Ok(rendered_img) = fileuc_convert::pdfium::render_page_to_image(file, 0, 2048) {
                                        let rgba = rendered_img.to_rgba8();
                                        if let Ok(ocr_text) = fileuc_convert::ocr::ocr_rgba_bytes(rendered_img.width(), rendered_img.height(), &rgba) {
                                            if !ocr_text.trim().is_empty() {
                                                text = ocr_text;
                                            }
                                        }
                                    }
                                }
                                let _ = std::fs::write(&out_txt, &text);
                                Ok(text)
                            }
                            Err(e) => Err(e),
                        }
                    } else {
                        fileuc_convert::ocr::image_to_text(file, &out_txt)
                    };

                    match res {
                        Ok(text) => {
                            if !combined_text.is_empty() {
                                combined_text.push_str("\r\n\r\n");
                            }
                            combined_text.push_str(&text);
                            let final_out = apply_output_mode(out_txt, &request.settings.output_mode);
                            result.outputs.push(final_out);
                        }
                        Err(e) => result.errors.push((file.clone(), e)),
                    }
                    on_progress((i + 1) as f32 / total);
                }

                if !combined_text.is_empty() {
                    copy_to_clipboard(&combined_text);
                }
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
        let out_path = fileuc_convert::output::resolve_output_path(file, ext);
        convert_fn(file, &out_path)?;
        Ok(out_path)
    });
}

fn copy_to_clipboard(text: &str) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
        use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
        if OpenClipboard(None).is_ok() {
            let _ = EmptyClipboard();
            let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes_len = utf16.len() * std::mem::size_of::<u16>();
            if let Ok(handle) = GlobalAlloc(GHND, bytes_len) {
                let ptr = GlobalLock(handle);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, ptr as *mut u8, bytes_len);
                    let _ = GlobalUnlock(handle);
                    // CF_UNICODETEXT = 13
                    let _ = SetClipboardData(13, windows::Win32::Foundation::HANDLE(handle.0));
                }
            }
            let _ = CloseClipboard();
        }
    }
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
