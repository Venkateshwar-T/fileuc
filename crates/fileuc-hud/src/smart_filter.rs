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
    Csv,
    Json,
    Markdown,
    Text,
    Html,
    Video,
    Audio,
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
        "xlsx" | "xls" => FileCategory::Excel,
        "csv" => FileCategory::Csv,
        "json" => FileCategory::Json,
        "md" | "markdown" => FileCategory::Markdown,
        "txt" => FileCategory::Text,
        "html" | "htm" => FileCategory::Html,
        "mp4" | "mov" | "mkv" | "webm" | "avi" => FileCategory::Video,
        "mp3" | "wav" | "ogg" | "aac" | "flac" | "m4a" | "wma" | "opus" | "aiff" => {
            FileCategory::Audio
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => FileCategory::Archive,
        _ => FileCategory::Other,
    }
}

pub fn get_action_children(action: ConversionAction) -> Option<&'static [ConversionAction]> {
    match action {
        ConversionAction::OfficeGroup => Some(&[
            ConversionAction::PdfToDocx,
            ConversionAction::PdfToExcel,
        ]),
        ConversionAction::IconGroup => Some(&[
            ConversionAction::ToIco,
            ConversionAction::CreateIconPack,
        ]),
        _ => None,
    }
}

fn canonicalize_ext(ext: &str) -> &str {
    match ext {
        "jpeg" => "jpg",
        "tiff" => "tif",
        "htm" => "html",
        other => other,
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
                    ConversionAction::ToSvg,
                    ConversionAction::IconGroup,
                    ConversionAction::ToPdf,
                    ConversionAction::ReduceSize,
                    ConversionAction::StripMetadata,
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
                    let first_canon = canonicalize_ext(&all_exts[0]);
                    if all_exts.iter().all(|e| canonicalize_ext(e) == first_canon) {
                        match first_canon {
                            "png" => img_actions.retain(|&a| a != ConversionAction::ToPng),
                            "jpg" => img_actions
                                .retain(|&a| a != ConversionAction::ToJpg && a != ConversionAction::ToJpeg),
                            "webp" => img_actions.retain(|&a| a != ConversionAction::ToWebp),
                            "bmp" => img_actions.retain(|&a| a != ConversionAction::ToBmp),
                            "tif" => img_actions.retain(|&a| a != ConversionAction::ToTiff),
                            "ico" => img_actions.retain(|&a| a != ConversionAction::IconGroup && a != ConversionAction::ToIco),
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
                ConversionAction::IconGroup,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Pdf => {
                let all_locked = files.iter().all(|f| is_pdf_encrypted(f.as_ref()));
                let mut actions = Vec::with_capacity(8);
                if files.len() > 1 {
                    if all_locked {
                        actions.push(ConversionAction::UnlockPdf);
                    }
                    actions.extend([
                        ConversionAction::MergePdf,
                        ConversionAction::OfficeGroup,
                        ConversionAction::PdfOcr,
                        ConversionAction::PdfToImages,
                        ConversionAction::ReduceSize,
                        ConversionAction::Share,
                    ]);
                } else {
                    if all_locked {
                        actions.push(ConversionAction::UnlockPdf);
                    } else {
                        actions.push(ConversionAction::LockPdf);
                    }
                    actions.extend([
                        ConversionAction::OfficeGroup,
                        ConversionAction::PdfOcr,
                        ConversionAction::PdfToImages,
                        ConversionAction::ReduceSize,
                        ConversionAction::SplitPdf,
                        ConversionAction::Share,
                    ]);
                }
                actions
            }
            FileCategory::Word => vec![
                ConversionAction::DocToPdf,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Pptx => vec![
                ConversionAction::PptxToPdf,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Excel => vec![
                ConversionAction::ExcelToPdf,
                ConversionAction::XlsxToCsv,
                ConversionAction::XlsxToJson,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Csv => vec![
                ConversionAction::CsvToXlsx,
                ConversionAction::CsvToJson,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Json => vec![
                ConversionAction::JsonToXlsx,
                ConversionAction::JsonToCsv,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Markdown => vec![
                ConversionAction::MdToPdf,
                ConversionAction::MdToTxt,
                ConversionAction::ToHtml,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Text => vec![
                ConversionAction::TxtToPdf,
                ConversionAction::TxtToMd,
                ConversionAction::ToHtml,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Html => vec![
                ConversionAction::HtmlToPdf,
                ConversionAction::CreateZip,
                ConversionAction::Share,
            ],
            FileCategory::Video => {
                if has_ffmpeg {
                    vec![
                        ConversionAction::VideoToGif,
                        ConversionAction::VideoToMp3,
                        ConversionAction::ReduceSize,
                        ConversionAction::StripMetadata,
                        ConversionAction::Share,
                    ]
                } else {
                    vec![ConversionAction::CreateZip, ConversionAction::Share]
                }
            }
            FileCategory::Audio => {
                if has_ffmpeg {
                    let mut audio_actions = vec![
                        ConversionAction::ToMp3,
                        ConversionAction::ToWav,
                        ConversionAction::ToOgg,
                        ConversionAction::ToAac,
                        ConversionAction::ToFlac,
                        ConversionAction::ReduceSize,
                        ConversionAction::StripMetadata,
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
                                "mp3" => audio_actions.retain(|&a| a != ConversionAction::ToMp3),
                                "wav" => audio_actions.retain(|&a| a != ConversionAction::ToWav),
                                "ogg" => audio_actions.retain(|&a| a != ConversionAction::ToOgg),
                                "aac" => audio_actions.retain(|&a| a != ConversionAction::ToAac),
                                "flac" => audio_actions.retain(|&a| a != ConversionAction::ToFlac),
                                _ => {}
                            }
                        }
                    }
                    audio_actions
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

    if files.len() > 1 && !actions.contains(&ConversionAction::BulkRename) {
        if let Some(pos) = actions.iter().position(|&a| a == ConversionAction::Share) {
            actions.insert(pos, ConversionAction::BulkRename);
        } else {
            actions.push(ConversionAction::BulkRename);
        }
    }

    actions
}

fn has_encrypt_dictionary(buf: &[u8]) -> bool {
    let pattern = b"/Encrypt";
    let mut offset = 0;
    while let Some(pos) = buf[offset..].windows(pattern.len()).position(|w| w == pattern) {
        let abs_pos = offset + pos;
        offset = abs_pos + pattern.len();

        // Must not be preceded by alphanumeric char (part of longer token)
        if abs_pos > 0 && buf[abs_pos - 1].is_ascii_alphanumeric() {
            continue;
        }

        // Skip whitespace following /Encrypt
        let rest = &buf[abs_pos + pattern.len()..];
        let mut idx = 0;
        while idx < rest.len() && rest[idx].is_ascii_whitespace() {
            idx += 1;
        }

        if idx >= rest.len() {
            continue;
        }

        // Case 1: Direct dictionary syntax "<<"
        if rest[idx..].starts_with(b"<<") {
            return true;
        }

        // Case 2: Indirect object reference syntax, e.g. "4 0 R"
        let after_ws = &rest[idx..];
        let mut p = 0;
        while p < after_ws.len() && after_ws[p].is_ascii_digit() {
            p += 1;
        }
        if p > 0 && p < after_ws.len() && after_ws[p].is_ascii_whitespace() {
            while p < after_ws.len() && after_ws[p].is_ascii_whitespace() {
                p += 1;
            }
            let gen_start = p;
            while p < after_ws.len() && after_ws[p].is_ascii_digit() {
                p += 1;
            }
            if p > gen_start && p < after_ws.len() && after_ws[p].is_ascii_whitespace() {
                while p < after_ws.len() && after_ws[p].is_ascii_whitespace() {
                    p += 1;
                }
                if p < after_ws.len() && after_ws[p] == b'R' {
                    if p + 1 == after_ws.len()
                        || after_ws[p + 1].is_ascii_whitespace()
                        || matches!(after_ws[p + 1], b'/' | b'>' | b']')
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
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
            if has_encrypt_dictionary(&buf) {
                return true;
            }
        }
    }
    // Also check first 4KB for linearized PDF header
    let head_size = (len as usize).min(4096);
    let mut head_buf = vec![0u8; head_size];
    if file.seek(SeekFrom::Start(0)).is_ok() {
        if file.read_exact(&mut head_buf).is_ok() {
            if has_encrypt_dictionary(&head_buf) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_has_encrypt_dictionary() {
        // Direct dictionary
        let sample1 = b"<< /Size 25 /Encrypt << /Filter /Standard /V 2 >> /Root 1 0 R >>";
        assert!(has_encrypt_dictionary(sample1));

        // Indirect object reference
        let sample2 = b"trailer << /Size 50 /Root 1 0 R /Encrypt 4 0 R /ID [<1234><5678>] >>";
        assert!(has_encrypt_dictionary(sample2));

        // Multi-digit indirect object reference
        let sample3 = b"/Encrypt  128  0  R /Info 2 0 R";
        assert!(has_encrypt_dictionary(sample3));

        // Plain text mentioning /Encrypt inside a content stream or metadata
        let sample4 = b"BT /F1 12 Tf (The /Encrypt algorithm is AES-256) Tj ET";
        assert!(!has_encrypt_dictionary(sample4));

        // Delimited or compounded keyword
        let sample5 = b"/NonEncrypt 4 0 R";
        assert!(!has_encrypt_dictionary(sample5));
    }

    #[test]
    fn test_filter_actions_synonym_extensions() {
        let files = vec![
            PathBuf::from(r"C:\test\photo1.jpg"),
            PathBuf::from(r"C:\test\photo2.jpeg"),
        ];
        let actions = filter_actions(&files, false);
        // Both are JPEG synonyms, so ToJpg should be suppressed
        assert!(!actions.contains(&ConversionAction::ToJpg));
        assert!(!actions.contains(&ConversionAction::ToJpeg));
        // Other image conversions remain
        assert!(actions.contains(&ConversionAction::ToPng));
        assert!(actions.contains(&ConversionAction::ToWebp));
    }
}
