use fileuc_types::ConvertError;
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

static GLOBAL_PDFIUM: std::sync::OnceLock<Option<Pdfium>> = std::sync::OnceLock::new();

fn try_create_pdfium() -> Result<Pdfium, ConvertError> {
    let mut candidate_paths: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidate_paths.push(parent.join("pdfium.dll"));
        }
    }
    candidate_paths.push(PathBuf::from("pdfium.dll"));
    candidate_paths.push(PathBuf::from(r"C:\Program Files\Filee\pdfium.dll"));

    for path in candidate_paths {
        if path.exists() {
            if let Ok(bindings) = Pdfium::bind_to_library(path.to_string_lossy().as_ref()) {
                return Ok(Pdfium::new(bindings));
            }
        }
    }

    if let Ok(bindings) = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./")) {
        return Ok(Pdfium::new(bindings));
    }

    if let Ok(bindings) = Pdfium::bind_to_system_library() {
        return Ok(Pdfium::new(bindings));
    }

    Err(ConvertError::EngineMissing(
        "pdfium.dll could not be loaded. Please ensure pdfium.dll is present next to fileuc.exe."
            .to_string(),
    ))
}

pub fn init_pdfium() -> Result<&'static Pdfium, ConvertError> {
    let instance = GLOBAL_PDFIUM.get_or_init(|| try_create_pdfium().ok());
    match instance {
        Some(p) => Ok(p),
        None => Err(ConvertError::EngineMissing(
            "pdfium.dll could not be loaded. Please ensure pdfium.dll is present next to fileuc.exe."
                .to_string(),
        )),
    }
}

#[derive(Debug, Clone)]
pub struct LayoutItem {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: String,
    pub is_bold: bool,
    pub font_size: f32,
    pub is_checkbox: bool,
    pub is_in_box: bool,
}

#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct LayoutImage {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PageLayout {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub items: Vec<LayoutItem>,
    pub divider_lines: Vec<f32>,
    pub has_outer_border: bool,
    pub boxes: Vec<LayoutBox>,
    pub images: Vec<LayoutImage>,
}

pub fn extract_structured_layout(src: &Path) -> Result<Vec<PageLayout>, ConvertError> {
    let pdfium = init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(src, None)
        .map_err(|e| ConvertError::ConverterError(format!("Pdfium failed to open PDF: {}", e)))?;
    extract_structured_layout_from_doc(&document)
}

pub fn extract_structured_layout_from_doc(document: &PdfDocument) -> Result<Vec<PageLayout>, ConvertError> {
    let mut layouts = Vec::new();

    for (page_idx, page) in document.pages().iter().enumerate() {
        let page_w = page.width().value;
        let page_h = page.height().value;
        let mut items = Vec::new();
        let mut divider_lines = Vec::new();
        let mut images: Vec<LayoutImage> = Vec::new();

        let mut top_border_found = false;
        let mut bottom_border_found = false;
        let mut box_regions: Vec<(f32, f32, f32, f32)> = Vec::new(); // (min_x, max_x, min_y, max_y)

        // First pass: scan paths for borders, boxes, and divider lines
        let mut h_lines: Vec<(f32, f32, f32)> = Vec::new(); // (y, min_x, max_x)

        for obj in page.objects().iter() {
            if obj.object_type() == PdfPageObjectType::Path {
                if let Ok(b) = obj.bounds() {
                    let min_x = b.x1().value.min(b.x2().value);
                    let max_x = b.x1().value.max(b.x2().value);
                    let min_y = b.y1().value.min(b.y2().value);
                    let max_y = b.y1().value.max(b.y2().value);
                    let w = max_x - min_x;
                    let h = max_y - min_y;

                    // Large width line
                    if w > page_w * 0.7 && h <= 3.0 {
                        if min_y > page_h * 0.75 {
                            top_border_found = true;
                        } else if min_y < page_h * 0.25 {
                            bottom_border_found = true;
                        } else {
                            divider_lines.push(page_h - max_y);
                        }
                    } else if w >= 40.0 && w <= 200.0 && h <= 3.0 {
                        h_lines.push((min_y, min_x, max_x));
                    }
                }
            }
        }

        // Pair horizontal lines that form rectangular boxes (like Photo box)
        for i in 0..h_lines.len() {
            for j in (i + 1)..h_lines.len() {
                let (y1, x1_a, x1_b) = h_lines[i];
                let (y2, x2_a, x2_b) = h_lines[j];
                let dy = (y1 - y2).abs();
                let dx_a = (x1_a - x2_a).abs();
                let dx_b = (x1_b - x2_b).abs();
                if dy >= 50.0 && dy <= 200.0 && dx_a <= 10.0 && dx_b <= 10.0 {
                    let b_min_x = x1_a.min(x2_a) - 5.0;
                    let b_max_x = x1_b.max(x2_b) + 5.0;
                    let b_min_y = y1.min(y2) - 5.0;
                    let b_max_y = y1.max(y2) + 5.0;
                    box_regions.push((b_min_x, b_max_x, b_min_y, b_max_y));
                }
            }
        }

        let has_outer_border = top_border_found && bottom_border_found;

        // Second pass: scan text and checkboxes
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
                            let processed = if crate::indic::is_mojibake_or_legacy_font(trimmed, Some(&font_name)) {
                                crate::indic::convert_legacy_text(trimmed, Some(&font_name))
                            } else {
                                crate::pdf::clean_pdf_text(trimmed)
                            };
                            let clean_trimmed = processed.trim();
                            if !clean_trimmed.is_empty() {
                                let font_size = text_obj.scaled_font_size().value;
                                let is_bold = font_name.to_lowercase().contains("bold");

                                // Check if this text is inside a designated box region (like Photo box)
                                let is_in_box = box_regions.iter().any(|&(bx1, bx2, by1, by2)| {
                                    min_x >= bx1 && max_x <= bx2 && min_y >= by1 && max_y <= by2
                                });

                                items.push(LayoutItem {
                                    x: min_x,
                                    y: top_y,
                                    width,
                                    height,
                                    text: clean_trimmed.to_string(),
                                    is_bold,
                                    font_size,
                                    is_checkbox: false,
                                    is_in_box,
                                });
                            }
                        }
                    }
                }
                PdfPageObjectType::Path => {
                    // Small square checkbox
                    if width >= 6.0 && width <= 25.0 && height >= 6.0 && height <= 25.0 {
                        let ratio = width / height.max(0.1);
                        if ratio >= 0.7 && ratio <= 1.4 {
                            items.push(LayoutItem {
                                x: min_x,
                                y: top_y,
                                width,
                                height,
                                text: "☐".to_string(),
                                is_bold: false,
                                font_size: 11.0,
                                is_checkbox: true,
                                is_in_box: false,
                            });
                        }
                    }
                }
                PdfPageObjectType::Image => {
                    if let Some(img_obj) = obj.as_image_object() {
                        if let Ok(raw_img) = img_obj.get_raw_image() {
                            let mut png_bytes = Vec::new();
                            if raw_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).is_ok() {
                                images.push(LayoutImage {
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

        // Fallback if no text objects were found
        if items.is_empty() {
            if let Ok(text_page) = page.text() {
                let full_page_text = text_page.all();
                let mut cur_y = 50.0;
                for line in full_page_text.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        let processed = if crate::indic::is_mojibake_or_legacy_font(trimmed, None) {
                            crate::indic::convert_legacy_text(trimmed, None)
                        } else {
                            crate::pdf::clean_pdf_text(trimmed)
                        };
                        items.push(LayoutItem {
                            x: 50.0,
                            y: cur_y,
                            width: page_w - 100.0,
                            height: 14.0,
                            text: processed,
                            is_bold: false,
                            font_size: 11.0,
                            is_checkbox: false,
                            is_in_box: false,
                        });
                        cur_y += 18.0;
                    }
                }
            }
        }

        let boxes: Vec<LayoutBox> = box_regions
            .into_iter()
            .map(|(bx1, bx2, by1, by2)| LayoutBox {
                x: bx1,
                y: page_h - by2,
                width: bx2 - bx1,
                height: by2 - by1,
            })
            .collect();

        layouts.push(PageLayout {
            page_number: page_idx + 1,
            width_pt: page_w,
            height_pt: page_h,
            items,
            divider_lines,
            has_outer_border,
            boxes,
            images,
        });
    }

    Ok(layouts)
}

pub fn extract_text(src: &Path) -> Result<Vec<String>, ConvertError> {
    let pdfium = init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(src, None)
        .map_err(|e| ConvertError::ConverterError(format!("Pdfium failed to open PDF: {}", e)))?;

    let mut paragraphs = Vec::new();

    for page in document.pages().iter() {
        if let Ok(text_page) = page.text() {
            let full_page_text = text_page.all();
            for line in full_page_text.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    paragraphs.push(trimmed.to_string());
                }
            }
        }
    }

    if paragraphs.is_empty() {
        paragraphs.push(format!(
            "Document: {}",
            src.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    Ok(paragraphs)
}

pub fn render_page_to_image_from_doc(
    document: &PdfDocument,
    page_index: usize,
    target_width: i32,
) -> Result<image::DynamicImage, ConvertError> {
    let page = document
        .pages()
        .get(page_index as _)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to get page {}: {}", page_index, e)))?;

    let render_config = PdfRenderConfig::new().set_target_width(target_width);
    let bitmap = page
        .render_with_config(&render_config)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to render page: {}", e)))?;

    bitmap
        .as_image()
        .map_err(|e| ConvertError::ConverterError(format!("Failed to convert bitmap to image: {}", e)))
}

pub fn render_page_to_image(
    src: &Path,
    page_index: usize,
    target_width: i32,
) -> Result<image::DynamicImage, ConvertError> {
    let pdfium = init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(src, None)
        .map_err(|e| ConvertError::ConverterError(format!("Pdfium failed to open PDF: {}", e)))?;
    render_page_to_image_from_doc(&document, page_index, target_width)
}

pub fn render_pdf_to_images(
    src: &Path,
    output_dir: &Path,
    format: &str,
) -> Result<Vec<PathBuf>, ConvertError> {
    let stem = src.file_stem().unwrap_or_default().to_string_lossy();
    render_pdf_to_images_with_stem(src, output_dir, &stem, format)
}

pub fn render_pdf_to_images_with_stem(
    src: &Path,
    output_dir: &Path,
    stem: &str,
    format: &str,
) -> Result<Vec<PathBuf>, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    std::fs::create_dir_all(output_dir).map_err(|e| ConvertError::IoError(e.to_string()))?;

    let pdfium = init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(src, None)
        .map_err(|e| ConvertError::ConverterError(format!("Pdfium failed to open PDF: {}", e)))?;

    let mut outputs = Vec::new();
    let render_config = PdfRenderConfig::new().set_target_width(1920);

    for (index, page) in document.pages().iter().enumerate() {
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to render page {}: {}", index + 1, e)))?;

        let img = bitmap
            .as_image()
            .map_err(|e| ConvertError::ConverterError(format!("Failed to convert page {} to image: {}", index + 1, e)))?;

        let out_path = output_dir.join(format!("{}_image_{}.{}", stem, index + 1, format));
        img.save(&out_path)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to save image {}: {}", out_path.display(), e)))?;
        outputs.push(out_path);
    }

    Ok(outputs)
}
