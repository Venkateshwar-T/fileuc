use fileuc_types::ConversionAction;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileCategory {
    Image,
    Svg,
    Pdf,
    Word,
    Pptx,
    Excel,
    Markdown,
    Text,
    Video,
    Archive,
    Other,
}

fn categorize_extension(ext: &str) -> FileCategory {
    let lower = ext.to_ascii_lowercase();
    match lower.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tiff" | "tif" | "ico" | "heic" | "avif" => {
            FileCategory::Image
        }
        "svg" => FileCategory::Svg,
        "pdf" => FileCategory::Pdf,
        "doc" | "docx" => FileCategory::Word,
        "ppt" | "pptx" => FileCategory::Pptx,
        "xlsx" | "xls" | "csv" => FileCategory::Excel,
        "md" | "markdown" => FileCategory::Markdown,
        "txt" => FileCategory::Text,
        "mp4" | "mov" | "mkv" | "webm" | "avi" => FileCategory::Video,
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => FileCategory::Archive,
        _ => FileCategory::Other,
    }
}

pub fn filter_actions(files: &[impl AsRef<Path>], has_ffmpeg: bool) -> Vec<ConversionAction> {
    if files.is_empty() {
        return vec![ConversionAction::CreateZip];
    }

    let categories: Vec<FileCategory> = files
        .iter()
        .map(|f| {
            let path = f.as_ref();
            if path.is_dir() {
                FileCategory::Other
            } else {
                path.extension()
                    .and_then(|s| s.to_str())
                    .map(categorize_extension)
                    .unwrap_or(FileCategory::Other)
            }
        })
        .collect();

    let first = categories[0];
    let all_same = categories.iter().all(|&c| c == first);

    let mut actions = if all_same {
        match first {
            FileCategory::Image => {
                let mut img_actions = vec![
                    ConversionAction::ToPng,
                    ConversionAction::ToJpg,
                    ConversionAction::ToWebp,
                    ConversionAction::ToPdf,
                    ConversionAction::ToSvg,
                    ConversionAction::Ocr,
                    ConversionAction::Share,
                ];
                let all_exts: Vec<String> = files
                    .iter()
                    .filter_map(|f| {
                        f.as_ref()
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_ascii_lowercase())
                    })
                    .collect();
                if !all_exts.is_empty() {
                    let first_ext = &all_exts[0];
                    if all_exts.iter().all(|e| e == first_ext) {
                        match first_ext.as_str() {
                            "png" => img_actions.retain(|&a| a != ConversionAction::ToPng),
                            "jpg" | "jpeg" => img_actions
                                .retain(|&a| a != ConversionAction::ToJpg && a != ConversionAction::ToJpeg),
                            "webp" => img_actions.retain(|&a| a != ConversionAction::ToWebp),
                            "bmp" => img_actions.retain(|&a| a != ConversionAction::ToBmp),
                            "tiff" | "tif" => img_actions.retain(|&a| a != ConversionAction::ToTiff),
                            "ico" => img_actions.retain(|&a| a != ConversionAction::ToIco),
                            "svg" => img_actions.retain(|&a| a != ConversionAction::ToSvg),
                            _ => {}
                        }
                    }
                }
                img_actions
            }
            FileCategory::Svg => vec![
                ConversionAction::SvgToPng,
                ConversionAction::SvgToJpg,
                ConversionAction::SvgToWebp,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Pdf => {
                let all_locked = files.iter().all(|f| is_pdf_encrypted(f.as_ref()));
                let all_unlocked = files.iter().all(|f| !is_pdf_encrypted(f.as_ref()));
                let mut actions = Vec::with_capacity(8);
                if files.len() > 1 {
                    if all_locked {
                        actions.push(ConversionAction::UnlockPdf);
                    } else if all_unlocked {
                        actions.push(ConversionAction::LockPdf);
                    }
                    actions.extend([
                        ConversionAction::MergePdf,
                        ConversionAction::PdfToDocx,
                        ConversionAction::PdfToPptx,
                        ConversionAction::PdfToXlsx,
                        ConversionAction::PdfToImages,
                        ConversionAction::CompressPdf,
                        ConversionAction::CreateZip,
                    ]);
                } else {
                    if all_locked {
                        actions.push(ConversionAction::UnlockPdf);
                    } else {
                        actions.push(ConversionAction::LockPdf);
                    }
                    actions.extend([
                        ConversionAction::PdfToDocx,
                        ConversionAction::PdfToPptx,
                        ConversionAction::PdfToXlsx,
                        ConversionAction::PdfToImages,
                        ConversionAction::CompressPdf,
                        ConversionAction::SplitPdf,
                        ConversionAction::Share,
                    ]);
                }
                actions
            }
            FileCategory::Word => vec![
                ConversionAction::DocToPdf,
                ConversionAction::DocxToPptx,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Pptx => vec![
                ConversionAction::PptxToPdf,
                ConversionAction::PptxToDocx,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Excel => vec![
                ConversionAction::ExcelToPdf,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Markdown => vec![
                ConversionAction::ToHtml,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Text => vec![
                ConversionAction::ToHtml,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Video => {
                if has_ffmpeg {
                    vec![
                        ConversionAction::VideoToGif,
                        ConversionAction::VideoToMp3,
                        ConversionAction::CompressVideo,
                        ConversionAction::Share,
                    ]
                } else {
                    vec![ConversionAction::CreateZip, ConversionAction::Share]
                }
            }
            FileCategory::Archive => vec![
                ConversionAction::ExtractArchive,
                ConversionAction::CreateZip,
                ConversionAction::Create7z,
                ConversionAction::Share,
            ],
            FileCategory::Other => vec![
                ConversionAction::CreateZip,
                ConversionAction::Create7z,
                ConversionAction::Share,
            ],
        }
    } else {
        vec![
            ConversionAction::CreateZip,
            ConversionAction::Create7z,
            ConversionAction::Share,
        ]
    };

    actions.truncate(8);
    actions
}

pub fn is_pdf_encrypted(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    use std::io::{Read, Seek, SeekFrom};
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return false;
    };
    if len < 32 {
        return false;
    }
    // Read up to last 16KB where trailer / Encrypt dictionary is located
    let read_size = (len as usize).min(16384);
    let mut buf = vec![0u8; read_size];
    if file.seek(SeekFrom::End(-(read_size as i64))).is_ok() {
        if file.read_exact(&mut buf).is_ok() {
            if buf.windows(8).any(|w| w == b"/Encrypt") {
                return true;
            }
        }
    }
    // Also check first 4KB for linearized PDF header
    let head_size = (len as usize).min(4096);
    let mut head_buf = vec![0u8; head_size];
    if file.seek(SeekFrom::Start(0)).is_ok() {
        if file.read_exact(&mut head_buf).is_ok() {
            if head_buf.windows(8).any(|w| w == b"/Encrypt") {
                return true;
            }
        }
    }
    false
}
