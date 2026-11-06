use fileuc_types::ConversionAction;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileCategory {
    Image,
    Svg,
    Pdf,
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
        "mp4" | "mov" | "mkv" | "webm" | "avi" => FileCategory::Video,
        "zip" | "7z" | "tar" | "gz" => FileCategory::Archive,
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
                FileCategory::Archive
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
            FileCategory::Image => vec![
                ConversionAction::ToPng,
                ConversionAction::ToJpg,
                ConversionAction::ToWebp,
                ConversionAction::ToPdf,
                ConversionAction::Resize,
                ConversionAction::ToGrayscale,
            ],
            FileCategory::Svg => vec![
                ConversionAction::SvgToPng,
                ConversionAction::SvgToJpg,
                ConversionAction::CreateZip,
            ],
            FileCategory::Pdf => {
                if files.len() > 1 {
                    vec![
                        ConversionAction::MergePdf,
                        ConversionAction::PdfToImages,
                        ConversionAction::CompressPdf,
                    ]
                } else {
                    vec![
                        ConversionAction::PdfToImages,
                        ConversionAction::SplitPdf,
                        ConversionAction::CompressPdf,
                    ]
                }
            }
            FileCategory::Video => {
                if has_ffmpeg {
                    vec![
                        ConversionAction::VideoToGif,
                        ConversionAction::VideoToMp3,
                        ConversionAction::CompressVideo,
                    ]
                } else {
                    vec![ConversionAction::CreateZip]
                }
            }
            FileCategory::Archive | FileCategory::Other => vec![
                ConversionAction::CreateZip,
                ConversionAction::ExtractArchive,
            ],
        }
    } else {
        vec![ConversionAction::CreateZip]
    };

    actions.truncate(8);
    actions
}
