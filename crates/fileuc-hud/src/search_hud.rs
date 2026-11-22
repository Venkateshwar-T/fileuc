//! High-DPI Search Box HUD Component.
//! Displays a floating rounded search bar with animated sliding placeholders,
//! real-time keyword intent detection, and adaptive floating action chips.
//!
//! Designed for zero-lag responsiveness, maintainability, and clean Win32 rendering.

use crate::card_common::{
    draw_card_shadow, draw_text_cursor, is_cursor_visible, push_rounded_rect,
};
use crate::font::FontContext;
use crate::smart_filter::get_action_children;
use crate::spring::Spring;
use crate::theme::Theme;
use fileuc_types::ConversionAction;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tiny_skia::{
    Color, FillRule, FilterQuality, Paint, PathBuilder, Pattern, Pixmap, PremultipliedColorU8,
    SpreadMode, Stroke, Transform,
};

pub const SEARCH_BAR_WIDTH_DIP: f32 = 350.0;
pub const SEARCH_BAR_HEIGHT_DIP: f32 = 48.0;
pub const CHIP_HEIGHT_DIP: f32 = 38.0;
pub const CHIP_CORNER_RADIUS_DIP: f32 = 19.0;
pub const CHIP_PADDING_X_DIP: f32 = 16.0;
pub const CHIP_GAP_X_DIP: f32 = 8.0;
pub const CHIP_GAP_Y_DIP: f32 = 8.0;
pub const BAR_CHIP_GAP_DIP: f32 = 12.0;
pub const MAX_CHIPS: usize = 6;
pub const PLACEHOLDER_CYCLE_SECS: f32 = 2.6;
pub const PLACEHOLDER_TRANSITION_SECS: f32 = 0.28;

pub const SEARCH_ICON_ICO: &[u8] = include_bytes!("../../../assets/search-icon.ico");

/// Extracts the most appropriate PNG chunk from the authentic multi-res ICO for the target pixel size.
pub fn get_ico_png(ico_bytes: &[u8], target_size: u32) -> Option<&[u8]> {
    if ico_bytes.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([ico_bytes[4], ico_bytes[5]]) as usize;
    let mut best_diff = i32::MAX;
    let mut best_entry = None;

    for i in 0..count {
        let offset = 6 + i * 16;
        if offset + 16 > ico_bytes.len() {
            break;
        }
        let w_byte = ico_bytes[offset];
        let w = if w_byte == 0 { 256 } else { w_byte as u32 };
        let img_size = u32::from_le_bytes([
            ico_bytes[offset + 8],
            ico_bytes[offset + 9],
            ico_bytes[offset + 10],
            ico_bytes[offset + 11],
        ]) as usize;
        let img_offset = u32::from_le_bytes([
            ico_bytes[offset + 12],
            ico_bytes[offset + 13],
            ico_bytes[offset + 14],
            ico_bytes[offset + 15],
        ]) as usize;

        let diff = (w as i32 - target_size as i32).abs();
        let penalty = if (w as i32) < (target_size as i32) { 40 } else { 0 };
        if diff + penalty < best_diff {
            best_diff = diff + penalty;
            best_entry = Some((img_offset, img_size));
        }
    }

    if let Some((offset, size)) = best_entry {
        if offset + size <= ico_bytes.len() {
            return Some(&ico_bytes[offset..offset + size]);
        }
    }
    None
}

static SEARCH_ICON_PIXMAP: OnceLock<Pixmap> = OnceLock::new();

pub fn get_search_icon_pixmap() -> Option<&'static Pixmap> {
    SEARCH_ICON_PIXMAP.get_or_init(|| {
        let png_bytes = get_ico_png(SEARCH_ICON_ICO, 256)
            .unwrap_or(include_bytes!("../../../assets/search-icon.png"));
        Pixmap::decode_png(png_bytes).expect("Valid search icon PNG from ICO")
    });
    SEARCH_ICON_PIXMAP.get()
}

/// Expands group actions (e.g. `IconGroup`, `OfficeGroup`) into their constituent leaf children
/// so the search bar matches directly against executable target actions.
pub fn expand_to_leaf_actions(actions: &[ConversionAction]) -> Vec<ConversionAction> {
    let mut expanded = Vec::with_capacity(actions.len() + 4);
    for &action in actions {
        if let Some(children) = get_action_children(action) {
            for &child in children {
                if !expanded.contains(&child) {
                    expanded.push(child);
                }
            }
        } else if !expanded.contains(&action) {
            expanded.push(action);
        }
    }
    expanded
}

/// Generates a curated list of animated placeholder strings tailored to the available actions.
pub fn get_placeholders_for_actions(actions: &[ConversionAction]) -> Vec<&'static str> {
    if actions.is_empty() {
        return vec!["Search action (e.g. Convert, Share, Compress)..."];
    }

    let mut placeholders = Vec::with_capacity(actions.len());
    for &action in actions {
        let prompt = action.center_prompt();
        if !placeholders.contains(&prompt) {
            placeholders.push(prompt);
        }
    }

    if placeholders.is_empty() {
        vec!["Search action..."]
    } else {
        placeholders
    }
}

/// Computes Levenshtein edit distance between two lowercase ASCII/UTF-8 strings.
pub fn levenshtein_dist(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let m = s1_chars.len();
    let n = s2_chars.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut dp = vec![vec![0; n + 1]; m + 1];
    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// Returns domain-specific keyword aliases for each action to enable natural language matching.
fn get_action_aliases(action: ConversionAction) -> &'static [&'static str] {
    match action {
        ConversionAction::ToPng | ConversionAction::SvgToPng => &[
            "png", "to png", "convert to png", "image", "photo", "picture", "transparent", "raster", "lossless",
        ],
        ConversionAction::ToJpg | ConversionAction::ToJpeg | ConversionAction::SvgToJpg => &[
            "jpg", "jpeg", "to jpg", "to jpeg", "convert to jpg", "photo", "image", "picture", "lossy",
        ],
        ConversionAction::ToWebp | ConversionAction::SvgToWebp => &[
            "webp", "to webp", "convert to webp", "web image", "shrink image", "modern image",
        ],
        ConversionAction::ToSvg => &[
            "svg", "to svg", "convert to svg", "vector", "scalable", "vectorize", "vector graphics", "lines",
        ],
        ConversionAction::ToIco => &[
            "ico", "to ico", "convert to ico", "icon", "favicon", "windows icon",
        ],
        ConversionAction::CreateIconPack => &[
            "icon pack", "pack", "app icon", "web icon", "icons", "assets", "sizes", "favicon pack", "multi size",
        ],
        ConversionAction::ToBmp => &["bmp", "bitmap", "to bmp", "uncompressed"],
        ConversionAction::ToTiff => &["tiff", "tif", "to tiff", "high quality print"],
        ConversionAction::ToPdf
        | ConversionAction::DocToPdf
        | ConversionAction::PptxToPdf
        | ConversionAction::MdToPdf
        | ConversionAction::TxtToPdf
        | ConversionAction::HtmlToPdf
        | ConversionAction::ExcelToPdf => &[
            "pdf", "to pdf", "convert to pdf", "document", "acrobat", "print", "export pdf",
        ],
        ConversionAction::PdfToDocx => &[
            "docx", "word", "to word", "to docx", "convert to docx", "doc", "editable word", "office", "microsoft word",
        ],
        ConversionAction::PdfToExcel => &[
            "excel", "to excel", "xlsx", "spreadsheet", "extract to excel", "sheet", "table", "csv", "office", "workbook",
        ],
        ConversionAction::PdfOcr => &[
            "ocr", "searchable", "make searchable", "text recognition", "recognize", "extract text", "read text", "scan text", "copy text",
        ],
        ConversionAction::SplitPdf => &[
            "split", "separate", "split pdf", "extract pages", "pages", "divide", "break", "slice", "cut", "isolate", "part", "range", "extract",
        ],
        ConversionAction::MergePdf => &[
            "merge", "combine", "join", "merge pdf", "concat", "append", "attach", "stitch", "unite", "together", "bind", "glue", "fuse",
        ],
        ConversionAction::CompressPdf => &[
            "compress", "shrink", "reduce", "smaller pdf", "optimize pdf", "compact pdf", "reduction", "downsize", "compress pdf",
        ],
        ConversionAction::CompressVideo => &[
            "compress", "shrink video", "reduce video", "smaller video", "mp4 compress", "compact video", "reduce size", "compress video",
        ],
        ConversionAction::LockPdf => &[
            "lock", "protect", "password", "encrypt", "secure pdf", "lock pdf", "passphrase", "secret", "guard", "restrict",
        ],
        ConversionAction::UnlockPdf => &[
            "unlock", "decrypt", "remove password", "open pdf", "unlock pdf", "unprotect", "decode", "unsecure",
        ],
        ConversionAction::PdfToImages => &[
            "extract images", "images", "extract photos", "pdf to images", "pages to png", "photos", "pictures",
        ],
        ConversionAction::ReduceSize => &[
            "reduce", "reduce size", "compress", "size", "limit", "target size", "mb", "kb",
            "shrink", "resize", "scale", "reduction", "downsize", "compact", "compressor", "smaller", "make smaller",
        ],
        ConversionAction::BulkRename => &[
            "rename", "bulk rename", "batch rename", "rename files", "name", "prefix", "suffix", "numbering",
        ],
        ConversionAction::Share => &[
            "share", "send", "mobile", "phone", "qr", "qr code", "code", "instant qr", "wifi", "transfer", "airdrop",
            "share to phone", "share to mobile", "push", "beam", "drop", "receive",
        ],
        ConversionAction::CreateZip | ConversionAction::OneZip => &[
            "zip", "archive", "compress to zip", "compress", "zip file", "pack", "bundle",
        ],
        ConversionAction::Create7z => &[
            "7z", "archive", "7zip", "compress 7z", "ultra compress", "high compression",
        ],
        ConversionAction::ExtractArchive => &[
            "extract", "unzip", "unpack", "decompress", "open archive", "un7z", "unrar",
        ],
        ConversionAction::VideoToGif => &[
            "gif", "video to gif", "animated gif", "make gif", "clip",
        ],
        ConversionAction::VideoToMp3 => &[
            "mp3", "extract audio", "video to mp3", "audio", "rip audio", "sound", "track",
        ],
        ConversionAction::ToMp3 => &["mp3", "to mp3", "audio", "sound", "music"],
        ConversionAction::ToWav => &["wav", "to wav", "uncompressed audio"],
        ConversionAction::ToOgg => &["ogg", "to ogg", "vorbis"],
        ConversionAction::ToAac => &["aac", "to aac"],
        ConversionAction::ToFlac => &["flac", "to flac", "lossless", "hi-res audio"],
        ConversionAction::ToHtml => &["html", "to html", "markup", "webpage"],
        ConversionAction::MdToTxt => &["txt", "plain text", "text"],
        ConversionAction::TxtToMd => &["md", "markdown", "to md"],
        ConversionAction::XlsxToCsv | ConversionAction::JsonToCsv => &["csv", "to csv", "data"],
        ConversionAction::XlsxToJson | ConversionAction::CsvToJson => &["json", "to json", "data"],
        ConversionAction::CsvToXlsx | ConversionAction::JsonToXlsx => &[
            "excel", "xlsx", "to excel", "spreadsheet", "table",
        ],
        ConversionAction::StripMetadata => &[
            "strip", "metadata", "clean", "exif", "privacy", "remove exif", "clean metadata", "clear exif", "sanitize",
        ],
        ConversionAction::Resize => &["resize", "scale", "dimensions", "width", "height"],
        ConversionAction::ToGrayscale => &["grayscale", "black and white", "monochrome", "b&w", "gray"],
        _ => &[],
    }
}

/// Matches user search query against supported actions and returns up to `MAX_CHIPS` ranked actions.
pub fn match_actions(query: &str, supported: &[ConversionAction]) -> Vec<ConversionAction> {
    let trimmed = query.trim().to_lowercase();
    if trimmed.is_empty() {
        return supported.iter().take(MAX_CHIPS).copied().collect();
    }

    // Strip common leading words e.g. "convert to png" -> "png", "to png" -> "png"
    let clean_query = if let Some(stripped) = trimmed.strip_prefix("convert to ") {
        stripped.trim()
    } else if let Some(stripped) = trimmed.strip_prefix("to ") {
        stripped.trim()
    } else if let Some(stripped) = trimmed.strip_prefix("into ") {
        stripped.trim()
    } else {
        &trimmed
    };

    let query_tokens: Vec<&str> = clean_query.split_whitespace().collect();
    let mut scored: Vec<(ConversionAction, i32, usize)> = Vec::with_capacity(supported.len());

    for (orig_idx, &action) in supported.iter().enumerate() {
        let label_lower = action.label().to_lowercase();
        let prompt_lower = action.center_prompt().to_lowercase();
        let caption_lower = action.caption().to_lowercase();
        let aliases = get_action_aliases(action);

        let mut best_score = 0;

        // 1. Exact full query matches (Score: 100)
        if label_lower == trimmed || label_lower == clean_query {
            best_score = best_score.max(100);
        }
        for &alias in aliases {
            if alias == trimmed || alias == clean_query {
                best_score = best_score.max(98);
                break;
            }
        }

        // 2. Starts-with whole clean query matches (Score: 88)
        if best_score < 88 {
            if label_lower.starts_with(clean_query) || label_lower.starts_with(&trimmed) {
                best_score = best_score.max(88);
            }
            for &alias in aliases {
                if alias.starts_with(clean_query) || alias.starts_with(&trimmed) {
                    best_score = best_score.max(85);
                    break;
                }
            }
        }

        // 3. Substring whole query in prompt, label, or aliases (Score: 70)
        if best_score < 70 {
            if prompt_lower.contains(clean_query) || prompt_lower.contains(&trimmed) {
                best_score = best_score.max(70);
            } else if label_lower.contains(clean_query) || label_lower.contains(&trimmed) {
                best_score = best_score.max(68);
            } else if caption_lower.contains(clean_query) || caption_lower.contains(&trimmed) {
                best_score = best_score.max(65);
            } else {
                for &alias in aliases {
                    if alias.contains(clean_query) || alias.contains(&trimmed) {
                        best_score = best_score.max(62);
                        break;
                    }
                }
            }
        }

        // 4. Token-level keyword and typo-tolerant matching
        // Ensures multi-word inputs like "dakhfdk share" or typos like "pnf" match their intent
        let mut matching_tokens_count = 0;
        let mut max_token_score = 0;

        for &tok in &query_tokens {
            if tok.is_empty() {
                continue;
            }
            let mut tok_score = 0;

            // Direct token match against label, prompt, caption, aliases
            if label_lower == tok {
                tok_score = 92;
            } else if aliases.iter().any(|&a| a == tok) {
                tok_score = 90;
            } else if label_lower.starts_with(tok) || aliases.iter().any(|&a| a.starts_with(tok)) {
                tok_score = 80;
            } else if prompt_lower.contains(tok) || caption_lower.contains(tok) || aliases.iter().any(|&a| a.contains(tok)) {
                tok_score = 65;
            }

            // Typo tolerance: Levenshtein distance <= 1 for tokens with len >= 3
            if tok_score == 0 && tok.len() >= 3 {
                if levenshtein_dist(tok, &label_lower) <= 1 {
                    tok_score = 75;
                } else {
                    for &alias in aliases {
                        // Check exact alias or individual words in multi-word alias
                        if alias.len() >= 3 && levenshtein_dist(tok, alias) <= 1 {
                            tok_score = 73;
                            break;
                        }
                        for alias_word in alias.split_whitespace() {
                            if alias_word.len() >= 3 && levenshtein_dist(tok, alias_word) <= 1 {
                                tok_score = 72;
                                break;
                            }
                        }
                        if tok_score > 0 {
                            break;
                        }
                    }
                }
            }

            if tok_score > 0 {
                matching_tokens_count += 1;
                max_token_score = max_token_score.max(tok_score);
            }
        }

        if matching_tokens_count > 0 {
            let combined_token_score = max_token_score + (matching_tokens_count as i32 - 1) * 4;
            best_score = best_score.max(combined_token_score);
        }

        if best_score > 0 {
            scored.push((action, best_score, orig_idx));
        }
    }

    // Sort descending by score, tie-break by original index
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));

    scored
        .into_iter()
        .take(MAX_CHIPS)
        .map(|(action, _, _)| action)
        .collect()
}

/// Adaptive layout engine for floating action chips.
/// Computes chip widths and wraps into centered rows strictly within `bar_w`.
pub fn compute_chip_layout(
    actions: &[ConversionAction],
    is_clipboard_link: bool,
    bar_left: f32,
    bar_y: f32,
    bar_w: f32,
    bar_h: f32,
    dpi_scale: f32,
    flip_above: bool,
    fonts: &FontContext,
) -> Vec<(f32, f32, f32, f32)> {
    if actions.is_empty() {
        return Vec::new();
    }

    let chip_h = CHIP_HEIGHT_DIP * dpi_scale;
    let pad_x = CHIP_PADDING_X_DIP * dpi_scale;
    let gap_x = CHIP_GAP_X_DIP * dpi_scale;
    let gap_y = CHIP_GAP_Y_DIP * dpi_scale;
    let bar_chip_gap = BAR_CHIP_GAP_DIP * dpi_scale;
    let font_size = 16.5 * dpi_scale;

    // 1. Measure width of each chip
    let mut chip_widths = Vec::with_capacity(actions.len());
    for action in actions {
        let label = if is_clipboard_link && *action == ConversionAction::Share {
            "QR Code"
        } else {
            action.label()
        };
        let text_w = fonts.measure_text(label, font_size, true);
        let chip_w = (text_w + pad_x * 2.0).ceil();
        chip_widths.push(chip_w);
    }

    // 2. Pack chips into rows strictly constrained to bar_w
    let mut rows: Vec<Vec<(usize, f32)>> = Vec::new();
    let mut current_row: Vec<(usize, f32)> = Vec::new();
    let mut current_row_w = 0.0f32;

    for (idx, &w) in chip_widths.iter().enumerate() {
        let needed_w = if current_row.is_empty() {
            w
        } else {
            current_row_w + gap_x + w
        };

        if needed_w <= bar_w || current_row.is_empty() {
            current_row.push((idx, w));
            current_row_w = needed_w;
        } else {
            rows.push(current_row);
            current_row = vec![(idx, w)];
            current_row_w = w;
        }
    }
    if !current_row.is_empty() {
        rows.push(current_row);
    }

    let total_rows = rows.len() as f32;
    let total_chips_height = total_rows * chip_h + (total_rows - 1.0).max(0.0) * gap_y;

    // 3. Compute (x, y, w, h) for each chip
    let mut rects = vec![(0.0f32, 0.0f32, 0.0f32, 0.0f32); actions.len()];

    for (row_idx, row) in rows.iter().enumerate() {
        let row_total_w: f32 = row.iter().map(|(_, w)| *w).sum::<f32>()
            + (row.len() as f32 - 1.0).max(0.0) * gap_x;

        // Center row within search bar width
        let row_start_x = bar_left + ((bar_w - row_total_w) / 2.0).max(0.0);

        let row_y = if flip_above {
            // Chips render above search bar
            bar_y - bar_chip_gap - total_chips_height + (row_idx as f32) * (chip_h + gap_y)
        } else {
            // Chips render below search bar
            bar_y + bar_h + bar_chip_gap + (row_idx as f32) * (chip_h + gap_y)
        };

        let mut current_x = row_start_x;
        for &(chip_idx, chip_w) in row {
            rects[chip_idx] = (current_x, row_y, chip_w, chip_h);
            current_x += chip_w + gap_x;
        }
    }

    rects
}

/// Preview payload for items triggered directly from the Windows clipboard.
#[derive(Clone, Debug)]
pub enum ClipboardPreview {
    Image(PathBuf),
    Link(String),
    Text(String),
    Files(usize, String),
}

/// Helper function to smartly truncate long directory paths in the middle (e.g. C:\Users\...\Folder)
pub fn truncate_path_middle(path_str: &str, max_len: usize) -> String {
    if path_str.chars().count() <= max_len {
        path_str.to_string()
    } else {
        let chars: Vec<char> = path_str.chars().collect();
        let front = (max_len.saturating_sub(3)) / 2;
        let back = max_len.saturating_sub(3).saturating_sub(front);
        let s_front: String = chars[..front].iter().collect();
        let s_back: String = chars[chars.len().saturating_sub(back)..].iter().collect();
        format!("{}...{}", s_front, s_back)
    }
}

/// Persistent state of the Search Box HUD.
pub struct SearchHudState {
    pub input_text: String,
    pub supported_actions: Vec<ConversionAction>,
    pub matched_actions: Vec<ConversionAction>,
    pub placeholders: Vec<&'static str>,
    pub chip_rects: Vec<(f32, f32, f32, f32)>,
    pub selected_chip_idx: usize,
    pub hovered_chip_idx: Option<usize>,
    pub flip_above: bool,
    pub pop_in: Spring,
    pub placeholder_timer: f32,
    pub placeholder_idx: usize,
    pub dpi_scale: f32,
    pub last_cursor_visible: bool,
    pub clipboard_title: Option<String>,
    pub clipboard_subtitle: Option<String>,
    pub clipboard_preview: Option<ClipboardPreview>,
    pub clipboard_image_pixmap: Option<Pixmap>,
    pub is_clipboard_link: bool,
    pub file_extension: String,
}

impl SearchHudState {
    pub fn new(
        supported: &[ConversionAction],
        dpi_scale: f32,
        flip_above: bool,
        fonts: &FontContext,
    ) -> Self {
        Self::new_with_ext(supported, "", dpi_scale, flip_above, fonts)
    }

    pub fn new_with_ext(
        supported: &[ConversionAction],
        extension: &str,
        dpi_scale: f32,
        flip_above: bool,
        fonts: &FontContext,
    ) -> Self {
        let mut expanded = expand_to_leaf_actions(supported);
        let ext = extension.trim().trim_start_matches('.').to_lowercase();
        if !ext.is_empty() {
            crate::action_history::sort_actions_by_frequency(&ext, &mut expanded);
        }
        let placeholders = get_placeholders_for_actions(&expanded);
        let matched = match_actions("", &expanded);

        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        let dpi = if dpi_scale <= 0.0 { 1.0 } else { dpi_scale };

        let mut state = Self {
            input_text: String::new(),
            supported_actions: expanded,
            matched_actions: matched,
            placeholders,
            chip_rects: Vec::new(),
            selected_chip_idx: 0,
            hovered_chip_idx: None,
            flip_above,
            pop_in,
            placeholder_timer: 0.0,
            placeholder_idx: 0,
            dpi_scale: dpi,
            last_cursor_visible: true,
            clipboard_title: None,
            clipboard_subtitle: None,
            clipboard_preview: None,
            clipboard_image_pixmap: None,
            is_clipboard_link: false,
            file_extension: ext,
        };

        state.update_layout(fonts);
        state
    }

    /// Sets clipboard preview and preloads cached image thumbnail if applicable.
    pub fn set_clipboard_preview(&mut self, preview: ClipboardPreview, fonts: &FontContext) {
        if let ClipboardPreview::Image(ref path) = preview {
            self.load_clipboard_image(path);
        }
        self.clipboard_preview = Some(preview);
        self.update_layout(fonts);
    }

    /// Preloads and decodes clipboard image into a cached thumbnail Pixmap once.
    pub fn load_clipboard_image(&mut self, path: &Path) {
        if let Ok(dyn_img) = image::open(path) {
            let target_size = (60.0 * self.dpi_scale).round() as u32;
            let target_size = target_size.max(32).min(128);
            let thumb = dyn_img.thumbnail(target_size, target_size).to_rgba8();
            let (w, h) = thumb.dimensions();
            if let Some(mut pix) = Pixmap::new(w, h) {
                let pixels = pix.pixels_mut();
                let raw = thumb.as_raw();
                for (i, p) in pixels.iter_mut().enumerate() {
                    let r = raw[i * 4];
                    let g = raw[i * 4 + 1];
                    let b = raw[i * 4 + 2];
                    let a = raw[i * 4 + 3];
                    *p = PremultipliedColorU8::from_rgba(r, g, b, a)
                        .unwrap_or(PremultipliedColorU8::TRANSPARENT);
                }

                // Pre-clip rounded corners on load to eliminate per-frame Mask allocation during rendering
                let pw = pix.width();
                let ph = pix.height();
                let r = 8.0 * self.dpi_scale;
                if let Some(mut mask) = tiny_skia::Mask::new(pw, ph) {
                    let mut pb = PathBuilder::new();
                    push_rounded_rect(&mut pb, 0.0, 0.0, pw as f32, ph as f32, r);
                    if let Some(path) = pb.finish() {
                        mask.fill_path(&path, FillRule::Winding, false, Transform::identity());
                        let mask_data = mask.data();
                        for (i, p) in pix.pixels_mut().iter_mut().enumerate() {
                            let m_val = mask_data[i] as u32;
                            if m_val < 255 {
                                let red = (p.red() as u32 * m_val / 255) as u8;
                                let green = (p.green() as u32 * m_val / 255) as u8;
                                let blue = (p.blue() as u32 * m_val / 255) as u8;
                                let alpha = (p.alpha() as u32 * m_val / 255) as u8;
                                *p = PremultipliedColorU8::from_rgba(red.min(alpha), green.min(alpha), blue.min(alpha), alpha)
                                    .unwrap_or(PremultipliedColorU8::TRANSPARENT);
                            }
                        }
                    }
                }

                self.clipboard_image_pixmap = Some(pix);
            }
        }
    }

    /// Recomputes matches and updates chip layout.
    pub fn update_matches(&mut self, fonts: &FontContext) {
        if self.input_text.trim().is_empty() {
            self.matched_actions = self.supported_actions.iter().take(MAX_CHIPS).copied().collect();
        } else {
            self.matched_actions = match_actions(&self.input_text, &self.supported_actions);
        }
        if self.selected_chip_idx >= self.matched_actions.len() {
            self.selected_chip_idx = 0;
        }
        self.update_layout(fonts);
    }

    pub fn update_layout(&mut self, fonts: &FontContext) {
        // Base coordinate reference centered at (0, 0)
        let bar_w = SEARCH_BAR_WIDTH_DIP * self.dpi_scale;
        let bar_h = SEARCH_BAR_HEIGHT_DIP * self.dpi_scale;
        let bar_left = -bar_w / 2.0;
        let bar_top = -bar_h / 2.0;

        self.chip_rects = compute_chip_layout(
            &self.matched_actions,
            self.is_clipboard_link,
            bar_left,
            bar_top,
            bar_w,
            bar_h,
            self.dpi_scale,
            self.flip_above,
            fonts,
        );
    }

    pub fn push_char(&mut self, c: char, fonts: &FontContext) {
        if !c.is_control() && self.input_text.len() < 64 {
            self.input_text.push(c);
            self.update_matches(fonts);
        }
    }

    pub fn pop_char(&mut self, fonts: &FontContext) {
        if self.input_text.pop().is_some() {
            self.update_matches(fonts);
        }
    }

    pub fn paste_text(&mut self, s: &str, fonts: &FontContext) {
        for c in s.chars() {
            if self.input_text.len() >= 64 {
                break;
            }
            if !c.is_control() {
                self.input_text.push(c);
            }
        }
        self.update_matches(fonts);
    }

    pub fn clear(&mut self, fonts: &FontContext) {
        self.input_text.clear();
        self.update_matches(fonts);
    }

    pub fn select_prev_chip(&mut self) {
        if !self.matched_actions.is_empty() {
            if self.selected_chip_idx == 0 {
                self.selected_chip_idx = self.matched_actions.len() - 1;
            } else {
                self.selected_chip_idx -= 1;
            }
        }
    }

    pub fn select_next_chip(&mut self) {
        if !self.matched_actions.is_empty() {
            self.selected_chip_idx = (self.selected_chip_idx + 1) % self.matched_actions.len();
        }
    }

    pub fn best_match_action(&self) -> Option<ConversionAction> {
        let act = if self.selected_chip_idx < self.matched_actions.len() {
            self.matched_actions[self.selected_chip_idx]
        } else {
            *self.matched_actions.first()?
        };
        if self.supported_actions.contains(&act) {
            Some(act)
        } else {
            None
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let animating = self.pop_in.tick(dt);

        let cur_visible = is_cursor_visible();
        let blink_changed = cur_visible != self.last_cursor_visible;
        if blink_changed {
            self.last_cursor_visible = cur_visible;
        }

        // Rotate animated placeholder only when query is empty
        let mut placeholder_dirty = false;
        if self.input_text.is_empty() && self.placeholders.len() > 1 {
            self.placeholder_timer += dt;
            if self.placeholder_timer >= PLACEHOLDER_CYCLE_SECS {
                self.placeholder_timer = 0.0;
                self.placeholder_idx = (self.placeholder_idx + 1) % self.placeholders.len();
                placeholder_dirty = true;
            } else if self.placeholder_timer < PLACEHOLDER_TRANSITION_SECS {
                placeholder_dirty = true;
            }
        }

        animating || blink_changed || placeholder_dirty
    }

    /// Returns the bounding box of the search bar centered at (cx, cy).
    pub fn search_bar_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let w = SEARCH_BAR_WIDTH_DIP * self.dpi_scale;
        let h = SEARCH_BAR_HEIGHT_DIP * self.dpi_scale;
        (cx - w / 2.0, cy - h / 2.0, w, h)
    }

    /// Overall bounding box encompassing both search bar and all chips (used for mouse capture).
    pub fn overall_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let (bx, by, bw, bh) = self.search_bar_bounds(cx, cy);
        let mut min_x = bx;
        let mut max_x = bx + bw;
        let mut min_y = by;
        let mut max_y = by + bh;

        for &(rx, ry, rw, rh) in &self.chip_rects {
            let px = cx + rx;
            let py = cy + ry;
            min_x = min_x.min(px);
            max_x = max_x.max(px + rw);
            min_y = min_y.min(py);
            max_y = max_y.max(py + rh);
        }

        if self.clipboard_title.is_some() {
            let is_image_preview = matches!(self.clipboard_preview, Some(ClipboardPreview::Image(_)));
            let has_preview = self.clipboard_preview.is_some();
            let badge_h = if is_image_preview {
                if self.clipboard_subtitle.is_some() {
                    138.0 * self.dpi_scale
                } else {
                    114.0 * self.dpi_scale
                }
            } else if has_preview {
                if self.clipboard_subtitle.is_some() {
                    108.0 * self.dpi_scale
                } else {
                    84.0 * self.dpi_scale
                }
            } else if self.clipboard_subtitle.is_some() {
                60.0 * self.dpi_scale
            } else {
                42.0 * self.dpi_scale
            };
            let badge_w = SEARCH_BAR_WIDTH_DIP * self.dpi_scale;
            min_x = min_x.min(cx - badge_w / 2.0);
            max_x = max_x.max(cx + badge_w / 2.0);

            let gap = 12.0 * self.dpi_scale;
            if self.flip_above {
                max_y = max_y.max(by + bh + gap + badge_h);
            } else {
                min_y = min_y.min(by - gap - badge_h);
            }
        }

        let pad = 12.0 * self.dpi_scale;
        (min_x - pad, min_y - pad, (max_x - min_x) + pad * 2.0, (max_y - min_y) + pad * 2.0)
    }

    pub fn hit_test_chip(&self, rel_x: f32, rel_y: f32) -> Option<usize> {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        for (i, &(rx, ry, rw, rh)) in self.chip_rects.iter().enumerate() {
            let s_rx = rx * scale;
            let s_ry = ry * scale;
            let s_rw = rw * scale;
            let s_rh = rh * scale;
            if rel_x >= s_rx && rel_x <= s_rx + s_rw && rel_y >= s_ry && rel_y <= s_ry + s_rh {
                return Some(i);
            }
        }
        None
    }

    pub fn hit_test_search_bar(&self, rel_x: f32, rel_y: f32) -> bool {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let w = SEARCH_BAR_WIDTH_DIP * self.dpi_scale * scale;
        let h = SEARCH_BAR_HEIGHT_DIP * self.dpi_scale * scale;
        let left = -w / 2.0;
        let top = -h / 2.0;
        rel_x >= left && rel_x <= left + w && rel_y >= top && rel_y <= top + h
    }

    pub fn hit_test_any(&self, rel_x: f32, rel_y: f32) -> bool {
        self.hit_test_search_bar(rel_x, rel_y) || self.hit_test_chip(rel_x, rel_y).is_some()
    }
}

/// Renders the floating Search Box HUD into `pixmap`.
pub fn render_search_hud(
    state: &SearchHudState,
    theme: &Theme,
    fonts: &FontContext,
    pixmap: &mut Pixmap,
) {
    pixmap.fill(Color::TRANSPARENT);

    let scale = state.pop_in.value();
    if scale <= 0.01 {
        return;
    }

    let cx = pixmap.width() as f32 / 2.0;
    let cy = pixmap.height() as f32 / 2.0;
    let dpi = state.dpi_scale;

    let bar_w = SEARCH_BAR_WIDTH_DIP * dpi * scale;
    let bar_h = SEARCH_BAR_HEIGHT_DIP * dpi * scale;
    let bar_left = cx - bar_w / 2.0;
    let bar_top = cy - bar_h / 2.0;
    let bar_r = (SEARCH_BAR_HEIGHT_DIP / 2.0) * dpi * scale;

    // 1. Draw floating search bar ambient drop shadow
    draw_card_shadow(pixmap, bar_left, bar_top, bar_w, bar_h, bar_r, scale, dpi);

    // 2. Draw floating search bar pill body
    let mut bar_pb = PathBuilder::new();
    push_rounded_rect(&mut bar_pb, bar_left, bar_top, bar_w, bar_h, bar_r);
    if let Some(bar_path) = bar_pb.finish() {
        let mut bar_paint = Paint::default();
        if theme.is_dark {
            bar_paint.set_color_rgba8(28, 28, 30, 255);
        } else {
            bar_paint.set_color_rgba8(255, 255, 255, 255);
        }
        bar_paint.anti_alias = true;
        pixmap.fill_path(&bar_path, &bar_paint, FillRule::Winding, Transform::identity(), None);

        let mut border_paint = Paint::default();
        border_paint.set_color(theme.border_subtle);
        border_paint.anti_alias = true;
        let border_stroke = Stroke {
            width: 1.0 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&bar_path, &border_paint, &border_stroke, Transform::identity(), None);
    }

    // 3. Draw circular search icon image from authentic ICO
    let icon_diam = (bar_h - 10.0 * dpi * scale).max(24.0 * dpi * scale);
    let icon_r = icon_diam / 2.0;
    let icon_cx = bar_left + bar_r; // Centered inside the left circular cap of the bar
    let icon_cy = bar_top + bar_r;
    let icon_x = icon_cx - icon_r;
    let icon_y = icon_cy - icon_r;

    if let Some(icon_pixmap) = get_search_icon_pixmap() {
        let scale_x = icon_diam / (icon_pixmap.width() as f32);
        let scale_y = icon_diam / (icon_pixmap.height() as f32);

        let mut icon_paint = Paint::default();
        icon_paint.shader = Pattern::new(
            icon_pixmap.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Bilinear,
            1.0,
            Transform::from_scale(scale_x, scale_y).post_translate(icon_x, icon_y),
        );
        icon_paint.anti_alias = true;

        let mut icon_pb = PathBuilder::new();
        icon_pb.push_circle(icon_cx, icon_cy, icon_r);
        if let Some(icon_path) = icon_pb.finish() {
            pixmap.fill_path(&icon_path, &icon_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 4. Render input text or sliding dynamic placeholder
    let text_x = icon_cx + icon_r + 12.0 * dpi * scale;
    let text_y = bar_top + bar_h / 2.0;
    let font_size = 17.5 * dpi * scale;

    if !state.input_text.is_empty() {
        // Draw typed input text
        fonts.draw_text_left(
            pixmap,
            &state.input_text,
            text_x,
            text_y,
            font_size,
            theme.text_primary,
            false,
        );

        // Blinking caret right after typed text
        let measured_w = fonts.measure_text(&state.input_text, font_size, false);
        let caret_x = text_x + measured_w + 2.0 * dpi * scale;
        draw_text_cursor(
            pixmap,
            caret_x,
            text_y,
            21.0 * dpi * scale,
            scale,
            dpi,
            theme.text_primary,
        );
    } else {
        // Empty query: render animated sliding placeholder
        let current_text = state
            .placeholders
            .get(state.placeholder_idx)
            .copied()
            .unwrap_or("Search action...");

        if state.placeholders.len() > 1 && state.placeholder_timer < PLACEHOLDER_TRANSITION_SECS {
            // Sliding transition animation
            let progress = (state.placeholder_timer / PLACEHOLDER_TRANSITION_SECS).clamp(0.0, 1.0);
            let prev_idx = if state.placeholder_idx == 0 {
                state.placeholders.len() - 1
            } else {
                state.placeholder_idx - 1
            };
            let prev_text = state.placeholders[prev_idx];

            let slide_dist = 14.0 * dpi * scale;

            // Previous placeholder sliding up and fading out
            let prev_alpha = ((1.0 - progress) * 200.0) as u8;
            let mut prev_color = theme.text_secondary;
            prev_color = Color::from_rgba8(
                (prev_color.red() * 255.0) as u8,
                (prev_color.green() * 255.0) as u8,
                (prev_color.blue() * 255.0) as u8,
                prev_alpha,
            );
            fonts.draw_text_left(
                pixmap,
                prev_text,
                text_x,
                text_y - progress * slide_dist,
                font_size,
                prev_color,
                false,
            );

            // Current placeholder sliding in from bottom and fading in
            let cur_alpha = (progress * 200.0) as u8;
            let mut cur_color = theme.text_secondary;
            cur_color = Color::from_rgba8(
                (cur_color.red() * 255.0) as u8,
                (cur_color.green() * 255.0) as u8,
                (cur_color.blue() * 255.0) as u8,
                cur_alpha,
            );
            fonts.draw_text_left(
                pixmap,
                current_text,
                text_x,
                text_y + (1.0 - progress) * slide_dist,
                font_size,
                cur_color,
                false,
            );
        } else {
            // Steady placeholder
            let mut ph_color = theme.text_secondary;
            ph_color = Color::from_rgba8(
                (ph_color.red() * 255.0) as u8,
                (ph_color.green() * 255.0) as u8,
                (ph_color.blue() * 255.0) as u8,
                200,
            );
            fonts.draw_text_left(
                pixmap,
                current_text,
                text_x,
                text_y,
                font_size,
                ph_color,
                false,
            );
        }

        // Caret at initial position
        draw_text_cursor(
            pixmap,
            text_x,
            text_y,
            21.0 * dpi * scale,
            scale,
            dpi,
            theme.text_secondary,
        );
    }

    // 4.5. Render Floating Adaptive Clipboard Badge if triggered from clipboard
    if let Some(ref title) = state.clipboard_title {
        let has_subtitle = state.clipboard_subtitle.is_some();
        let subtitle_raw = state.clipboard_subtitle.as_deref().unwrap_or("");
        let subtitle = if let Some(path_part) = subtitle_raw.strip_prefix("Saves to: ") {
            format!("Saves to: {}", truncate_path_middle(path_part, 32))
        } else {
            truncate_path_middle(subtitle_raw, 36)
        };

        let is_image_preview = matches!(state.clipboard_preview, Some(ClipboardPreview::Image(_)));
        let has_preview = state.clipboard_preview.is_some();
        let badge_h = if is_image_preview {
            if has_subtitle { 138.0 * dpi * scale } else { 114.0 * dpi * scale }
        } else if has_preview {
            if has_subtitle { 108.0 * dpi * scale } else { 84.0 * dpi * scale }
        } else if has_subtitle {
            58.0 * dpi * scale
        } else {
            40.0 * dpi * scale
        };
        // Rounded card matching the search bar's pill corner radius
        let badge_r = 24.0 * dpi * scale;

        let title_font_size = 17.5 * dpi * scale;
        let sub_font_size = 13.5 * dpi * scale;
        let preview_font_size = 15.0 * dpi * scale;

        // Card width matches the search bar width exactly and is centered at bar_left
        let badge_w = bar_w;
        let badge_x = bar_left;
        let badge_gap = 12.0 * dpi * scale;
        let badge_y = if state.flip_above {
            bar_top + bar_h + badge_gap
        } else {
            bar_top - badge_h - badge_gap
        };

        // Ambient drop shadow
        draw_card_shadow(pixmap, badge_x, badge_y, badge_w, badge_h, badge_r, scale, dpi);

        // Badge pill body
        let mut badge_pb = PathBuilder::new();
        push_rounded_rect(&mut badge_pb, badge_x, badge_y, badge_w, badge_h, badge_r);
        if let Some(badge_path) = badge_pb.finish() {
            let mut badge_paint = Paint::default();
            if theme.is_dark {
                badge_paint.set_color_rgba8(36, 36, 38, 255);
            } else {
                badge_paint.set_color_rgba8(255, 255, 255, 255);
            }
            badge_paint.anti_alias = true;
            pixmap.fill_path(&badge_path, &badge_paint, FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            border_paint.set_color(theme.border_subtle);
            border_paint.anti_alias = true;
            let stroke = Stroke {
                width: 1.0 * dpi * scale,
                ..Default::default()
            };
            pixmap.stroke_path(&badge_path, &border_paint, &stroke, Transform::identity(), None);
        }

        // Render preview content & typography
        if is_image_preview {
            let mut curr_y = badge_y + 13.0 * dpi * scale;
            let thumb_box_size = 56.0 * dpi * scale;

            if let Some(ref thumb_pixmap) = state.clipboard_image_pixmap {
                let pw = thumb_pixmap.width() as f32;
                let ph = thumb_pixmap.height() as f32;
                let thumb_x = badge_x + (badge_w - pw) / 2.0;
                let thumb_y = curr_y + (thumb_box_size - ph) / 2.0;
                let thumb_r = 8.0 * dpi * scale;

                pixmap.draw_pixmap(
                    thumb_x as i32,
                    thumb_y as i32,
                    thumb_pixmap.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );

                let mut border_pb = PathBuilder::new();
                push_rounded_rect(&mut border_pb, thumb_x, thumb_y, pw, ph, thumb_r);
                if let Some(bp) = border_pb.finish() {
                    let mut p = Paint::default();
                    p.set_color(theme.border_subtle);
                    p.anti_alias = true;
                    let stroke = Stroke {
                        width: 1.0 * dpi * scale,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&bp, &p, &stroke, Transform::identity(), None);
                }
            }
            curr_y += thumb_box_size + 11.0 * dpi * scale;

            fonts.draw_text_centered(
                pixmap,
                title,
                badge_x + badge_w / 2.0,
                curr_y + 8.0 * dpi * scale,
                title_font_size,
                theme.text_primary,
                true,
            );
            curr_y += 24.0 * dpi * scale;

            if has_subtitle {
                fonts.draw_text_centered(
                    pixmap,
                    &subtitle,
                    badge_x + badge_w / 2.0,
                    curr_y + 6.0 * dpi * scale,
                    sub_font_size,
                    theme.text_secondary,
                    false,
                );
            }
        } else if has_preview {
            let mut curr_y = badge_y + 12.0 * dpi * scale;
            let capsule_h = 28.0 * dpi * scale;
            let capsule_r = 14.0 * dpi * scale;

            let (preview_str, is_link) = match &state.clipboard_preview {
                Some(ClipboardPreview::Link(l)) => {
                    let clean = l.trim();
                    let disp = if clean.chars().count() > 34 {
                        let s: String = clean.chars().take(34).collect();
                        format!("{}...", s)
                    } else {
                        clean.to_string()
                    };
                    (disp, true)
                }
                Some(ClipboardPreview::Text(t)) => {
                    let clean = t.trim().replace('\n', " ").replace('\r', "");
                    let disp = if clean.chars().count() > 30 {
                        let s: String = clean.chars().take(30).collect();
                        format!("\"{}...\"", s)
                    } else {
                        format!("\"{}\"", clean)
                    };
                    (disp, false)
                }
                Some(ClipboardPreview::Files(n, name)) => {
                    let disp = if *n > 1 {
                        format!("{} (+{} more)", name, n - 1)
                    } else {
                        name.to_string()
                    };
                    (disp, false)
                }
                _ => (String::new(), false),
            };

            let cap_txt_w = fonts.measure_text(&preview_str, preview_font_size, false);
            let cap_w = (cap_txt_w + 24.0 * dpi * scale).min(badge_w - 32.0 * dpi * scale);
            let cap_x = badge_x + (badge_w - cap_w) / 2.0;
            let cap_y = curr_y;

            let mut cap_pb = PathBuilder::new();
            push_rounded_rect(&mut cap_pb, cap_x, cap_y, cap_w, capsule_h, capsule_r);
            if let Some(cp) = cap_pb.finish() {
                let mut cap_paint = Paint::default();
                if is_link {
                    if theme.is_dark {
                        cap_paint.set_color_rgba8(112, 102, 224, 40);
                    } else {
                        cap_paint.set_color_rgba8(108, 99, 216, 30);
                    }
                } else {
                    if theme.is_dark {
                        cap_paint.set_color_rgba8(255, 255, 255, 16);
                    } else {
                        cap_paint.set_color_rgba8(0, 0, 0, 14);
                    }
                }
                cap_paint.anti_alias = true;
                pixmap.fill_path(&cp, &cap_paint, FillRule::Winding, Transform::identity(), None);

                let mut border_paint = Paint::default();
                if is_link {
                    border_paint.set_color(theme.slice_hover_border);
                } else {
                    border_paint.set_color(theme.border_subtle);
                }
                border_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 1.0 * dpi * scale,
                    ..Default::default()
                };
                pixmap.stroke_path(&cp, &border_paint, &stroke, Transform::identity(), None);
            }

            let cap_txt_color = if is_link {
                if theme.is_dark {
                    Color::from_rgba8(180, 175, 255, 255)
                } else {
                    theme.slice_hover_border
                }
            } else {
                theme.text_secondary
            };
            fonts.draw_text_centered(
                pixmap,
                &preview_str,
                cap_x + cap_w / 2.0,
                cap_y + capsule_h / 2.0 + 1.0 * dpi * scale,
                preview_font_size,
                cap_txt_color,
                false,
            );

            curr_y += capsule_h + 10.0 * dpi * scale;

            fonts.draw_text_centered(
                pixmap,
                title,
                badge_x + badge_w / 2.0,
                curr_y + 8.0 * dpi * scale,
                title_font_size,
                theme.text_primary,
                true,
            );
            curr_y += 24.0 * dpi * scale;

            if has_subtitle {
                fonts.draw_text_centered(
                    pixmap,
                    &subtitle,
                    badge_x + badge_w / 2.0,
                    curr_y + 6.0 * dpi * scale,
                    sub_font_size,
                    theme.text_secondary,
                    false,
                );
            }
        } else {
            let title_y = if has_subtitle {
                badge_y + 20.0 * dpi * scale
            } else {
                badge_y + badge_h / 2.0
            };
            fonts.draw_text_centered(
                pixmap,
                title,
                badge_x + badge_w / 2.0,
                title_y,
                title_font_size,
                theme.text_primary,
                true,
            );

            if has_subtitle {
                fonts.draw_text_centered(
                    pixmap,
                    &subtitle,
                    badge_x + badge_w / 2.0,
                    badge_y + 42.0 * dpi * scale,
                    sub_font_size,
                    theme.text_secondary,
                    false,
                );
            }
        }
    }

    // 5. Render floating chips
    let chip_r = CHIP_CORNER_RADIUS_DIP * dpi * scale;
    let chip_font_size = 16.5 * dpi * scale;

    for (idx, &action) in state.matched_actions.iter().enumerate() {
        if idx >= state.chip_rects.len() {
            break;
        }

        let (rel_x, rel_y, chip_w_unscaled, chip_h_unscaled) = state.chip_rects[idx];
        let chip_w = chip_w_unscaled * scale;
        let chip_h = chip_h_unscaled * scale;
        let chip_x = cx + rel_x * scale;
        let chip_y = cy + rel_y * scale;

        let is_selected = idx == state.selected_chip_idx;
        let is_hovered = state.hovered_chip_idx == Some(idx);
        let is_active = is_selected || is_hovered;

        // Chip ambient drop shadow
        draw_card_shadow(pixmap, chip_x, chip_y, chip_w, chip_h, chip_r, scale, dpi);

        // Chip pill body
        let mut chip_pb = PathBuilder::new();
        push_rounded_rect(&mut chip_pb, chip_x, chip_y, chip_w, chip_h, chip_r);
        if let Some(chip_path) = chip_pb.finish() {
            let mut chip_paint = Paint::default();
            if is_active {
                chip_paint.set_color(theme.slice_hover_fill);
            } else if theme.is_dark {
                chip_paint.set_color_rgba8(36, 36, 38, 255);
            } else {
                chip_paint.set_color_rgba8(255, 255, 255, 255);
            }
            chip_paint.anti_alias = true;
            pixmap.fill_path(&chip_path, &chip_paint, FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            if is_active {
                border_paint.set_color(theme.slice_hover_border);
            } else {
                border_paint.set_color(theme.border_subtle);
            }
            border_paint.anti_alias = true;
            let stroke = Stroke {
                width: 1.0 * dpi * scale,
                ..Default::default()
            };
            pixmap.stroke_path(&chip_path, &border_paint, &stroke, Transform::identity(), None);
        }

        // Chip text label
        let label = if state.is_clipboard_link && action == ConversionAction::Share {
            "QR Code"
        } else {
            action.label()
        };
        let txt_color = if is_active {
            Color::WHITE
        } else {
            theme.text_primary
        };

        fonts.draw_text_centered(
            pixmap,
            label,
            chip_x + chip_w / 2.0,
            chip_y + chip_h / 2.0,
            chip_font_size,
            txt_color,
            true,
        );
    }
}
