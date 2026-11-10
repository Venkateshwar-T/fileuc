use fileuc_types::ConvertError;
use pdfium_render::prelude::*;

#[derive(Debug, Clone)]
pub struct RawTextItem {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_family: String,
    pub font_size: f32,
    pub is_bold: bool,
    pub is_italic: bool,
    pub color_rgb: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct RawPath {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke_width: f32,
    pub stroke_color: Option<[u8; 3]>,
    pub fill_color: Option<[u8; 3]>,
}

#[derive(Debug, Clone)]
pub struct RawImage {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RawPageContent {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub text_items: Vec<RawTextItem>,
    pub paths: Vec<RawPath>,
    pub images: Vec<RawImage>,
}

pub fn extract_raw_page_elements(page: &PdfPage, page_number: usize) -> Result<RawPageContent, ConvertError> {
    let page_w = page.width().value;
    let page_h = page.height().value;

    let mut text_items = Vec::new();
    let mut paths = Vec::new();
    let mut images = Vec::new();

    for obj in page.objects().iter() {
        let bounds = match obj.bounds() {
            Ok(b) => b,
            Err(_) => continue,
        };

        let min_x = bounds.x1().value.min(bounds.x2().value);
        let max_x = bounds.x1().value.max(bounds.x2().value);
        let min_y = bounds.y1().value.min(bounds.y2().value);
        let max_y = bounds.y1().value.max(bounds.y2().value);

        let width = max_x - min_x;
        let height = max_y - min_y;
        let top_y = page_h - max_y;

        match obj.object_type() {
            PdfPageObjectType::Text => {
                if let Some(text_obj) = obj.as_text_object() {
                    let text = text_obj.text();
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let font_name = text_obj.font().name();
                        let clean_text = if crate::indic::is_mojibake_or_legacy_font(trimmed, Some(&font_name)) {
                            crate::indic::convert_legacy_text(trimmed, Some(&font_name))
                        } else {
                            crate::pdf::clean_pdf_text(trimmed)
                        };

                        if !clean_text.trim().is_empty() {
                            let font_name_lower = font_name.to_lowercase();
                            let is_bold = font_name_lower.contains("bold") || font_name_lower.contains("black") || font_name_lower.contains("heavy");
                            let is_italic = font_name_lower.contains("italic") || font_name_lower.contains("oblique");
                            let font_size = text_obj.scaled_font_size().value;

                            // Extract color if available, default to dark charcoal [15, 23, 42]
                            let color_rgb = match obj.fill_color() {
                                Ok(c) => [c.red(), c.green(), c.blue()],
                                Err(_) => [15, 23, 42],
                            };

                            text_items.push(RawTextItem {
                                text: clean_text,
                                x: min_x,
                                y: top_y,
                                width,
                                height,
                                font_family: font_name,
                                font_size,
                                is_bold,
                                is_italic,
                                color_rgb,
                            });
                        }
                    }
                }
            }
            PdfPageObjectType::Path => {
                let stroke_color = obj.stroke_color().ok().map(|c| [c.red(), c.green(), c.blue()]);
                let fill_color = obj.fill_color().ok().map(|c| [c.red(), c.green(), c.blue()]);
                let stroke_width = obj.stroke_width().map(|w| w.value).unwrap_or(1.0);

                paths.push(RawPath {
                    x1: min_x,
                    y1: top_y,
                    x2: max_x,
                    y2: top_y + height,
                    stroke_width,
                    stroke_color,
                    fill_color,
                });
            }
            PdfPageObjectType::Image => {
                if let Some(img_obj) = obj.as_image_object() {
                    if let Ok(raw_img) = img_obj.get_raw_image() {
                        let mut png_bytes = Vec::new();
                        if raw_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).is_ok() {
                            images.push(RawImage {
                                x: min_x,
                                y: top_y,
                                width,
                                height,
                                data: png_bytes,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(RawPageContent {
        page_number,
        width_pt: page_w,
        height_pt: page_h,
        text_items,
        paths,
        images,
    })
}
