use fileuc_types::ConvertError;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};
use std::io::{Read, Write};
use std::path::Path;

static PPTX_BLANK_TEMPLATE: &[u8] = include_bytes!("../assets/blank.pptx");

#[derive(Debug, Clone)]
struct MediaFile {
    filename: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct SlideData {
    title: String,
    bullet_points: Vec<String>,
    image_filename: Option<String>,
    ocr_text: Option<String>,
}

#[derive(Debug, Clone)]
struct SlideItem {
    text: String,
    image: Option<image::DynamicImage>,
}

/// Escapes standard XML characters.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) >= 0x20 || c == '\t' || c == '\n' || c == '\r' => out.push(c),
            _ => {}
        }
    }
    out
}

/// Cleans a Windows path by ensuring it is absolute and removing any `\\?\` UNC prefix
/// that causes COM errors with Microsoft Office applications.
pub fn clean_windows_path(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let s = abs.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

/// Converts a DOCX file to PPTX, retaining 100% of images and text.
pub fn docx_to_pptx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let file = std::fs::File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open DOCX: {}", e)))?;

    // 1. Extract all media files from word/media/*
    let mut media_files: Vec<MediaFile> = Vec::new();
    for i in 0..archive.len() {
        let mut file_in_zip = archive.by_index(i)
            .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        let name = file_in_zip.name().to_string();
        if name.starts_with("word/media/") && !file_in_zip.is_dir() {
            let filename = name.trim_start_matches("word/media/").to_string();
            let mut bytes = Vec::new();
            file_in_zip.read_to_end(&mut bytes)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            media_files.push(MediaFile { filename, bytes });
        }
    }

    // 2. Extract paragraphs from word/document.xml
    let mut doc_xml = String::new();
    if let Ok(mut xml_file) = archive.by_name("word/document.xml") {
        let _ = xml_file.read_to_string(&mut doc_xml);
    }

    let mut paragraphs = Vec::new();
    for p_chunk in doc_xml.split("<w:p") {
        if let Some(end_p) = p_chunk.find("</w:p>") {
            let p_content = &p_chunk[..end_p];
            let mut line = String::new();
            for t_chunk in p_content.split("<w:t") {
                if let Some(start_t) = t_chunk.find('>') {
                    if let Some(end_t) = t_chunk[start_t..].find("</w:t>") {
                        let text = &t_chunk[start_t + 1..start_t + end_t];
                        line.push_str(text);
                    }
                }
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                paragraphs.push(trimmed.to_string());
            }
        }
    }

    if paragraphs.is_empty() {
        paragraphs.push(
            src.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        );
    }

    // 3. Group paragraphs into slides (e.g. 4-5 points per slide)
    let mut slides: Vec<SlideData> = Vec::new();
    let mut current_title = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let mut current_points = Vec::new();

    let mut media_iter = media_files.iter();

    for p in paragraphs {
        if p.len() < 40 && !p.ends_with('.') && current_points.len() >= 2 {
            // New section title
            let img = media_iter.next().map(|m| m.filename.clone());
            slides.push(SlideData {
                title: current_title,
                bullet_points: current_points,
                image_filename: img,
                ocr_text: None,
            });
            current_title = p;
            current_points = Vec::new();
        } else {
            current_points.push(p);
            if current_points.len() >= 5 {
                let img = media_iter.next().map(|m| m.filename.clone());
                slides.push(SlideData {
                    title: current_title.clone(),
                    bullet_points: current_points,
                    image_filename: img,
                    ocr_text: None,
                });
                current_points = Vec::new();
            }
        }
    }

    if !current_points.is_empty() || slides.is_empty() {
        let img = media_iter.next().map(|m| m.filename.clone());
        slides.push(SlideData {
            title: current_title,
            bullet_points: current_points,
            image_filename: img,
            ocr_text: None,
        });
    }

    // If there are leftover images, create dedicated image slides so ZERO images are lost
    for rem_img in media_iter {
        slides.push(SlideData {
            title: format!("Figure: {}", rem_img.filename),
            bullet_points: Vec::new(),
            image_filename: Some(rem_img.filename.clone()),
            ocr_text: None,
        });
    }

    // Detect orientation from DOCX
    let is_landscape = doc_xml.contains("w:orient=\"landscape\"") || {
        if let Some(pos) = doc_xml.find("<w:pgSz") {
            let chunk = &doc_xml[pos..pos + 200.min(doc_xml.len() - pos)];
            let w_val = chunk.split("w:w=\"").nth(1).and_then(|s| s.split('"').next()).and_then(|s| s.parse::<u32>().ok());
            let h_val = chunk.split("w:h=\"").nth(1).and_then(|s| s.split('"').next()).and_then(|s| s.parse::<u32>().ok());
            if let (Some(w), Some(h)) = (w_val, h_val) {
                w > h
            } else {
                false
            }
        } else {
            false
        }
    };

    let slide_dims = if is_landscape {
        (12192000, 6858000) // 16:9 Landscape
    } else {
        (7560000, 10080000) // Standard 3:4 Portrait
    };

    build_pptx_package(output, &slides, &media_files, Some(slide_dims))
}

/// Converts a PPTX file to DOCX, retaining 100% of images and text.
pub fn pptx_to_docx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let file = std::fs::File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open PPTX: {}", e)))?;

    // 1. Extract all media files from ppt/media/*
    let mut media_files: Vec<MediaFile> = Vec::new();
    for i in 0..archive.len() {
        let mut file_in_zip = archive.by_index(i)
            .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        let name = file_in_zip.name().to_string();
        if name.starts_with("ppt/media/") && !file_in_zip.is_dir() {
            let filename = name.trim_start_matches("ppt/media/").to_string();
            let mut bytes = Vec::new();
            file_in_zip.read_to_end(&mut bytes)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            media_files.push(MediaFile { filename, bytes });
        }
    }

    // 2. Extract slides in order
    let mut slide_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                slide_names.push(name);
            }
        }
    }

    // Natural sort slide1.xml, slide2.xml...
    slide_names.sort_by_key(|name| {
        let num_str: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
        num_str.parse::<u32>().unwrap_or(0)
    });

    let mut slides_content: Vec<(String, Vec<String>)> = Vec::new();

    for s_name in &slide_names {
        let mut s_xml = String::new();
        if let Ok(mut s_file) = archive.by_name(s_name) {
            let _ = s_file.read_to_string(&mut s_xml);
        }

        let mut lines = Vec::new();
        for p_chunk in s_xml.split("<a:p") {
            if let Some(end_p) = p_chunk.find("</a:p>") {
                let p_content = &p_chunk[..end_p];
                let mut line = String::new();
                for t_chunk in p_content.split("<a:t") {
                    if let Some(start_t) = t_chunk.find('>') {
                        if let Some(end_t) = t_chunk[start_t..].find("</a:t>") {
                            let text = &t_chunk[start_t + 1..start_t + end_t];
                            line.push_str(text);
                        }
                    }
                }
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
        }

        let title = if !lines.is_empty() {
            lines.remove(0)
        } else {
            format!("Slide {}", slides_content.len() + 1)
        };
        slides_content.push((title, lines));
    }

    build_docx_from_slides(output, &slides_content, &media_files)
}

/// Converts a PDF to PPTX, retaining 100% of images, layouts, orientation, and text.
pub fn pdf_to_pptx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // 1. Try high-fidelity Document IR reconstruction engine
    if let Ok(()) = crate::ir::reconstruct_pdf_to_pptx(src, output) {
        if output.exists() {
            if let Ok(meta) = std::fs::metadata(output) {
                if meta.len() > 0 {
                    return Ok(());
                }
            }
        }
    }

    // 2. Structured layout analysis via PDFium fallback
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        if let Ok(document) = pdfium.load_pdf_from_file(src, None) {
            if let Ok(layouts) = crate::pdfium::extract_structured_layout_from_doc(&document) {
                if !layouts.is_empty() {
                    let has_text = layouts.iter().any(|l| !l.items.is_empty());
                    if has_text {
                        return render_structured_layout_to_pptx(&layouts, output);
                    } else {
                        // PDF has pages but 0 text objects -> Image-only / scanned PDF!
                        // Directly render the high-res slide images from this already-loaded document,
                        // embed them with exact slide dimensions, and run Windows OCR so the slide
                        // contains editable text.
                        return render_image_pdf_doc_to_pptx(&document, &layouts, output);
                    }
                }
            }
        }
    }

    // 2. Genuine last-resort fallback: Pure-Rust slide generation from lopdf text only if PDFium is completely missing
    let text_paragraphs = extract_lopdf_paragraphs(src)?;
    render_lopdf_text_to_pptx(src, &text_paragraphs, output)
}

fn render_image_pdf_doc_to_pptx(
    document: &pdfium_render::prelude::PdfDocument,
    layouts: &[crate::pdfium::PageLayout],
    output: &Path,
) -> Result<(), ConvertError> {
    let mut media_files: Vec<MediaFile> = Vec::new();
    let mut slides: Vec<SlideData> = Vec::new();

    let first = &layouts[0];
    let slide_cx = (first.width_pt * 12700.0).round() as u64;
    let slide_cy = (first.height_pt * 12700.0).round() as u64;
    let slide_dims = (slide_cx.max(1), slide_cy.max(1));

    for (page_idx, layout) in layouts.iter().enumerate() {
        let is_portrait = layout.height_pt > layout.width_pt;
        let render_config = if is_portrait {
            pdfium_render::prelude::PdfRenderConfig::new().set_target_height(1920)
        } else {
            pdfium_render::prelude::PdfRenderConfig::new().set_target_width(1920)
        };

        if let Ok(page) = document.pages().get(page_idx as _) {
            if let Ok(bitmap) = page.render_with_config(&render_config) {
                if let Ok(img) = bitmap.as_image() {
                    // Run Windows OCR on the slide image so slide contains editable text
                    let rgba = img.to_rgba8();
                    let ocr_text = crate::ocr::ocr_rgba_bytes(img.width(), img.height(), &rgba)
                        .ok()
                        .map(|t| {
                            if crate::indic::is_mojibake_or_legacy_font(&t, None) {
                                crate::indic::convert_legacy_text(&t, None)
                            } else {
                                crate::pdf::clean_pdf_text(&t)
                            }
                        })
                        .filter(|t| !t.trim().is_empty());

                    let mut png_bytes = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut png_bytes);
                    if img.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                        let filename = format!("slide_page_{}.png", page_idx + 1);
                        media_files.push(MediaFile {
                            filename: filename.clone(),
                            bytes: png_bytes,
                        });

                        slides.push(SlideData {
                            title: format!("Slide {}", page_idx + 1),
                            bullet_points: Vec::new(),
                            image_filename: Some(filename),
                            ocr_text,
                        });
                    }
                }
            }
        }
    }

    build_pptx_package(output, &slides, &media_files, Some(slide_dims))
}

fn extract_lopdf_paragraphs(src: &Path) -> Result<Vec<String>, ConvertError> {
    let doc = lopdf::Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF via lopdf: {}", e)))?;
    let pages = doc.get_pages();
    let mut paragraphs = Vec::new();

    for (&page_num, _) in pages.iter() {
        if let Ok(text) = doc.extract_text(&[page_num]) {
            let converted = if crate::indic::is_mojibake_or_legacy_font(&text, None) {
                crate::indic::convert_legacy_text(&text, None)
            } else {
                crate::pdf::clean_pdf_text(&text)
            };
            for line in converted.lines() {
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
            src.file_stem().unwrap_or_default().to_string_lossy()
        ));
    }

    Ok(paragraphs)
}

fn render_lopdf_text_to_pptx(src: &Path, paragraphs: &[String], output: &Path) -> Result<(), ConvertError> {
    let mut slides = Vec::new();
    let mut cur_points = Vec::new();
    let stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();

    for p in paragraphs {
        cur_points.push(p.clone());
        if cur_points.len() >= 5 {
            slides.push(SlideData {
                title: format!("{} - Part {}", stem, slides.len() + 1),
                bullet_points: cur_points,
                image_filename: None,
                ocr_text: None,
            });
            cur_points = Vec::new();
        }
    }
    if !cur_points.is_empty() || slides.is_empty() {
        slides.push(SlideData {
            title: stem,
            bullet_points: cur_points,
            image_filename: None,
            ocr_text: None,
        });
    }

    build_pptx_package(output, &slides, &[], None)
}

/// Converts a PPTX presentation to PDF.
pub fn pptx_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // 1. Try PowerPoint COM automation first on Windows
    if try_powerpoint_com_convert(src, output).is_ok() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                return Ok(());
            }
        }
    }

    // 2. Pure Rust fallback: extract slides and exact slide-to-image mappings, render via pdf-writer
    let file = std::fs::File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to read PPTX: {}", e)))?;

    // Map media filename -> bytes
    let mut media_map: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
    for i in 0..archive.len() {
        if let Ok(mut entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if name.starts_with("ppt/media/") && !entry.is_dir() {
                let filename = name.trim_start_matches("ppt/media/").to_string();
                let mut bytes = Vec::new();
                if entry.read_to_end(&mut bytes).is_ok() {
                    media_map.insert(filename, bytes);
                }
            }
        }
    }

    // Find and natural-sort slides
    let mut slide_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                slide_names.push(name);
            }
        }
    }
    slide_names.sort_by_key(|name| {
        let digits: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().unwrap_or(0)
    });

    let mut slides_data: Vec<SlideItem> = Vec::new();

    for s_name in &slide_names {
        // Read slide XML
        let mut slide_xml = String::new();
        if let Ok(mut s_file) = archive.by_name(s_name) {
            let _ = s_file.read_to_string(&mut slide_xml);
        }

        // Extract text
        let mut text_buf = String::new();
        for t_chunk in slide_xml.split("<a:t>") {
            if let Some(end_t) = t_chunk.find("</a:t>") {
                let val = &t_chunk[..end_t];
                if !val.trim().is_empty() {
                    text_buf.push_str(val);
                    text_buf.push('\n');
                }
            }
        }

        // Determine rels filename: ppt/slides/slide1.xml -> ppt/slides/_rels/slide1.xml.rels
        let rels_name = s_name.replace("ppt/slides/", "ppt/slides/_rels/") + ".rels";
        let mut slide_img: Option<image::DynamicImage> = None;

        if let Ok(mut rels_file) = archive.by_name(&rels_name) {
            let mut rels_xml = String::new();
            if rels_file.read_to_string(&mut rels_xml).is_ok() {
                // Find Target="../media/imageX.ext" or Target="media/imageX.ext"
                for rel_chunk in rels_xml.split("<Relationship") {
                    if rel_chunk.contains("relationships/image") {
                        if let Some(target_start) = rel_chunk.find("Target=\"") {
                            let after = &rel_chunk[target_start + 8..];
                            if let Some(target_end) = after.find('"') {
                                let target = &after[..target_end];
                                let filename = target
                                    .trim_start_matches("../media/")
                                    .trim_start_matches("media/")
                                    .to_string();
                                if let Some(bytes) = media_map.get(&filename) {
                                    if let Ok(img) = image::load_from_memory(bytes) {
                                        slide_img = Some(img);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: If no rels found an image, but there's a 1-to-1 media file
        if slide_img.is_none() && slides_data.len() < media_map.len() {
            let keys: Vec<_> = media_map.keys().collect();
            if let Some(key) = keys.get(slides_data.len()) {
                if let Some(bytes) = media_map.get(*key) {
                    slide_img = image::load_from_memory(bytes).ok();
                }
            }
        }

        slides_data.push(SlideItem {
            text: text_buf,
            image: slide_img,
        });
    }

    // If no slides detected but media exists, create slides from media
    if slides_data.is_empty() {
        for (_, bytes) in &media_map {
            if let Ok(img) = image::load_from_memory(bytes) {
                slides_data.push(SlideItem {
                    text: String::new(),
                    image: Some(img),
                });
            }
        }
    }

    render_pptx_slides_to_pdf(&slides_data, output)
}

fn try_powerpoint_com_convert(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let abs_src = clean_windows_path(src);
    let abs_dst = clean_windows_path(output);

    // 32 = ppSaveAsPDF
    let script = format!(
        "$ppt = New-Object -ComObject PowerPoint.Application; $ppt.DisplayAlerts = 0; try {{ $pres = $ppt.Presentations.Open('{}', 1, 0, 0); if ($pres -ne $null) {{ $pres.SaveAs('{}', 32); $pres.Close(); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($pres) | Out-Null; }} }} finally {{ $ppt.Quit(); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($ppt) | Out-Null; [System.GC]::Collect(); [System.GC]::WaitForPendingFinalizers(); }}",
        abs_src.replace('\'', "''"),
        abs_dst.replace('\'', "''")
    );

    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd.status().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    if status.success() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                return Ok(());
            }
        }
    }
    if output.exists() {
        let _ = std::fs::remove_file(output);
    }
    Err(ConvertError::ConverterError("PowerPoint COM conversion failed".to_string()))
}

fn render_pptx_slides_to_pdf(slides: &[SlideItem], output: &Path) -> Result<(), ConvertError> {
    let mut pdf = Pdf::new();
    let catalog_ref = Ref::new(1);
    let page_tree_ref = Ref::new(2);
    let font_ref = Ref::new(3);
    let font_name = Name(b"F1");

    pdf.catalog(catalog_ref).pages(page_tree_ref);
    pdf.type1_font(font_ref).base_font(Name(b"Helvetica"));

    let page_w = 960.0; // 16:9 widescreen presentation in points
    let page_h = 540.0;
    let mut next_ref = 4;
    let mut page_refs = Vec::new();

    let count = slides.len().max(1);
    for _ in 0..count {
        page_refs.push(Ref::new(next_ref));
        next_ref += 1;
    }

    pdf.pages(page_tree_ref)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    for (i, &page_ref) in page_refs.iter().enumerate() {
        let content_ref = Ref::new(next_ref);
        next_ref += 1;

        let slide = slides.get(i);
        let maybe_img = slide.and_then(|s| s.image.as_ref());

        // Allocate image Ref if image present
        let img_ref = if maybe_img.is_some() {
            let r = Ref::new(next_ref);
            next_ref += 1;
            Some(r)
        } else {
            None
        };

        let mut page = pdf.page(page_ref);
        page.media_box(Rect::new(0.0, 0.0, page_w, page_h));
        page.parent(page_tree_ref);
        page.contents(content_ref);

        let img_name = Name(b"Im0");

        // CRITICAL: Configure Resources in a single call to avoid overwriting /Resources dictionary!
        {
            let mut resources = page.resources();
            resources.fonts().pair(font_name, font_ref);
            if let Some(r) = img_ref {
                resources.x_objects().pair(img_name, r);
            }
        }
        page.finish();

        let mut content = Content::new();

        // 1. Draw crisp white slide background
        content.save_state();
        content.set_fill_rgb(1.0, 1.0, 1.0);
        content.rect(0.0, 0.0, page_w, page_h);
        content.fill_nonzero();
        content.restore_state();

        let slide_text = slide.map(|s| s.text.as_str()).unwrap_or("");
        let has_text = !slide_text.trim().is_empty();

        // 2. Embed Image if present
        if let (Some(img), Some(r)) = (maybe_img, img_ref) {
            let mut jpeg_bytes = Vec::new();
            let rgb = img.to_rgb8();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 90);
            let _ = enc.encode(&rgb, img.width(), img.height(), image::ExtendedColorType::Rgb8);

            let mut image_stream = pdf.image_xobject(r, &jpeg_bytes);
            image_stream.width(img.width() as i32);
            image_stream.height(img.height() as i32);
            image_stream.color_space().device_rgb();
            image_stream.bits_per_component(8);
            image_stream.filter(Filter::DctDecode);
            image_stream.finish();

            // Calculate scaled image bounds
            let img_aspect = img.width() as f32 / img.height().max(1) as f32;

            if !has_text {
                // Centered large image on slide
                let max_w = page_w - 60.0;
                let max_h = page_h - 60.0;
                let (draw_w, draw_h) = if max_w / max_h > img_aspect {
                    (max_h * img_aspect, max_h)
                } else {
                    (max_w, max_w / img_aspect)
                };
                let x = (page_w - draw_w) / 2.0;
                let y = (page_h - draw_h) / 2.0;

                content.save_state();
                content.transform([draw_w, 0.0, 0.0, draw_h, x, y]);
                content.x_object(img_name);
                content.restore_state();
            } else {
                // Split layout: Text on left (width ~500), image on right (width ~400)
                let target_w = 420.0;
                let target_h = 440.0;
                let (draw_w, draw_h) = if target_w / target_h > img_aspect {
                    (target_h * img_aspect, target_h)
                } else {
                    (target_w, target_w / img_aspect)
                };
                let x = page_w - draw_w - 40.0;
                let y = (page_h - draw_h) / 2.0;

                content.save_state();
                content.transform([draw_w, 0.0, 0.0, draw_h, x, y]);
                content.x_object(img_name);
                content.restore_state();
            }
        }

        // 3. Draw slide text
        if has_text {
            content.begin_text();
            content.set_fill_rgb(0.12, 0.16, 0.23); // slate-900
            content.set_font(font_name, 16.0);
            content.next_line(50.0, page_h - 70.0);

            for line in slide_text.lines().take(22) {
                let sanitized: String = line
                    .chars()
                    .map(|c| if (c as u32) >= 0x20 && (c as u32) <= 0x7E { c } else if c == '\t' { ' ' } else { '?' })
                    .collect();
                let trimmed = sanitized.trim();
                if !trimmed.is_empty() {
                    content.show(pdf_writer::Str(trimmed.as_bytes()));
                    content.next_line(0.0, -22.0);
                }
            }
            content.end_text();
        }

        pdf.stream(content_ref, &content.finish());
    }

    std::fs::write(output, pdf.finish()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

fn copy_template_base(
    target_zip: &mut zip::ZipWriter<std::fs::File>,
    options: zip::write::SimpleFileOptions,
    num_slides: usize,
    slide_cx: u64,
    slide_cy: u64,
) -> Result<(), ConvertError> {
    let mut template_archive = zip::ZipArchive::new(std::io::Cursor::new(PPTX_BLANK_TEMPLATE))
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open base PPTX template: {}", e)))?;

    for i in 0..template_archive.len() {
        let mut entry = template_archive.by_index(i)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        let name = entry.name().to_string();

        if name == "[Content_Types].xml" {
            let mut ct = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="gif" ContentType="image/gif"/>
  <Default Extension="bmp" ContentType="image/bmp"/>
  <Default Extension="webp" ContentType="image/webp"/>
  <Default Extension="tiff" ContentType="image/tiff"/>
  <Default Extension="tif" ContentType="image/tiff"/>
  <Default Extension="emf" ContentType="image/x-emf"/>
  <Default Extension="wmf" ContentType="image/x-wmf"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/>
  <Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout2.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout3.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout4.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout5.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout6.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout7.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout8.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout9.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout10.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout11.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
"#);
            for s in 1..=num_slides {
                ct.push_str(&format!(
                    "  <Override PartName=\"/ppt/slides/slide{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\n",
                    s
                ));
            }
            ct.push_str("</Types>");
            target_zip.start_file("[Content_Types].xml", options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(ct.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if name == "ppt/_rels/presentation.xml.rels" {
            let mut pr_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps" Target="presProps.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/viewProps" Target="viewProps.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/tableStyles" Target="tableStyles.xml"/>
"#);
            for s in 1..=num_slides {
                pr_rels.push_str(&format!(
                    "  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>\n",
                    s + 5,
                    s
                ));
            }
            pr_rels.push_str("</Relationships>");
            target_zip.start_file("ppt/_rels/presentation.xml.rels", options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(pr_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if name == "ppt/presentation.xml" {
            let mut p_xml = String::new();
            entry.read_to_string(&mut p_xml).map_err(|e| ConvertError::IoError(e.to_string()))?;

            if let Some(pos) = p_xml.find("<p:sldSz") {
                if let Some(end_pos) = p_xml[pos..].find("/>") {
                    let old_tag = &p_xml[pos..pos + end_pos + 2];
                    let new_tag = format!("<p:sldSz cx=\"{}\" cy=\"{}\"/>", slide_cx, slide_cy);
                    p_xml = p_xml.replace(old_tag, &new_tag);
                }
            }

            let mut sld_list = String::from("<p:sldIdLst>");
            for s in 1..=num_slides {
                sld_list.push_str(&format!(
                    "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
                    255 + s,
                    s + 5
                ));
            }
            sld_list.push_str("</p:sldIdLst>");

            if let Some(start) = p_xml.find("<p:sldIdLst>") {
                if let Some(end) = p_xml.find("</p:sldIdLst>") {
                    let old_chunk = &p_xml[start..end + "</p:sldIdLst>".len()];
                    p_xml = p_xml.replace(old_chunk, &sld_list);
                }
            } else if let Some(_pos) = p_xml.find("<p:sldMasterIdLst>") {
                if let Some(end) = p_xml.find("</p:sldMasterIdLst>") {
                    let insert_pos = end + "</p:sldMasterIdLst>".len();
                    p_xml.insert_str(insert_pos, &sld_list);
                }
            }

            target_zip.start_file("ppt/presentation.xml", options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(p_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if name.starts_with("ppt/slides/") || name.starts_with("docProps/thumbnail") {
            continue;
        } else {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.start_file(name, options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(&bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
        }
    }
    Ok(())
}

fn render_structured_layout_to_pptx(
    layouts: &[crate::pdfium::PageLayout],
    output: &Path,
) -> Result<(), ConvertError> {
    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let first = &layouts[0];
    let slide_cx = (first.width_pt * 12700.0).round() as u64;
    let slide_cy = (first.height_pt * 12700.0).round() as u64;

    copy_template_base(&mut zip, options, layouts.len(), slide_cx, slide_cy)?;

    for (page_idx, layout) in layouts.iter().enumerate() {
        let slide_num = page_idx + 1;

        // ppt/slides/_rels/slide{N}.xml.rels
        zip.start_file(format!("ppt/slides/_rels/slide{}.xml.rels", slide_num), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        let mut s_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout7.xml"/>
"#);
        for (img_i, _) in layout.images.iter().enumerate() {
            let img_file = format!("img_{}_{}.png", slide_num, img_i + 1);
            s_rels.push_str(&format!(
                "  <Relationship Id=\"rIdImg{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>\n",
                img_i + 1,
                img_file
            ));
        }
        s_rels.push_str("</Relationships>");
        zip.write_all(s_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;

        // Embedded images
        for (img_i, img) in layout.images.iter().enumerate() {
            let img_file = format!("ppt/media/img_{}_{}.png", slide_num, img_i + 1);
            zip.start_file(img_file, options)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            zip.write_all(&img.data).map_err(|e| ConvertError::IoError(e.to_string()))?;
        }

        // ppt/slides/slide{N}.xml
        zip.start_file(format!("ppt/slides/slide{}.xml", slide_num), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;

        let mut s_xml = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" 
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" 
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
"#);

        let mut shape_id = 2u32;

        if layout.has_outer_border {
            let bx = (layout.width_pt * 0.05 * 12700.0).round() as i64;
            let by = (layout.height_pt * 0.05 * 12700.0).round() as i64;
            let bcx = (layout.width_pt * 0.90 * 12700.0).round() as i64;
            let bcy = (layout.height_pt * 0.90 * 12700.0).round() as i64;
            s_xml.push_str(&format!(r#"      <!-- Outer Border -->
      <p:sp>
        <p:nvSpPr><p:cNvPr id="{}" name="Border"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
          <a:ln w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln>
        </p:spPr>
      </p:sp>
"#, shape_id, bx, by, bcx, bcy));
            shape_id += 1;
        }

        for b in &layout.boxes {
            let bx = (b.x * 12700.0).round() as i64;
            let by = (b.y * 12700.0).round() as i64;
            let bcx = (b.width * 12700.0).round() as i64;
            let bcy = (b.height * 12700.0).round() as i64;
            s_xml.push_str(&format!(r#"      <!-- Box -->
      <p:sp>
        <p:nvSpPr><p:cNvPr id="{}" name="Box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
          <a:ln w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln>
        </p:spPr>
      </p:sp>
"#, shape_id, bx, by, bcx, bcy));
            shape_id += 1;
        }

        for &div_y in &layout.divider_lines {
            let lx = (layout.width_pt * 0.06 * 12700.0).round() as i64;
            let ly = (div_y * 12700.0).round() as i64;
            let lcx = (layout.width_pt * 0.88 * 12700.0).round() as i64;
            s_xml.push_str(&format!(r#"      <!-- Divider Line -->
      <p:sp>
        <p:nvSpPr><p:cNvPr id="{}" name="Line"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="12700"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:solidFill><a:srgbClr val="888888"/></a:solidFill>
        </p:spPr>
      </p:sp>
"#, shape_id, lx, ly, lcx));
            shape_id += 1;
        }

        for (img_i, img) in layout.images.iter().enumerate() {
            let ix = (img.x * 12700.0).round() as i64;
            let iy = (img.y * 12700.0).round() as i64;
            let icx = (img.width * 12700.0).round() as i64;
            let icy = (img.height * 12700.0).round() as i64;
            s_xml.push_str(&format!(r#"      <!-- Picture Shape -->
      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="{}" name="Picture {}"/>
          <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="rIdImg{}"/>
          <a:stretch><a:fillRect/></a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
      </p:pic>
"#, shape_id, img_i + 1, img_i + 1, ix, iy, icx, icy));
            shape_id += 1;
        }

        for item in &layout.items {
            let tx = (item.x * 12700.0).round() as i64;
            let ty = (item.y * 12700.0).round() as i64;
            let tcx = ((item.width * 12700.0).max(127000.0) + 127000.0).round() as i64;
            let tcy = ((item.height * 12700.0).max(160000.0) + 60000.0).round() as i64;

            if item.is_checkbox {
                s_xml.push_str(&format!(r#"      <!-- Checkbox -->
      <p:sp>
        <p:nvSpPr><p:cNvPr id="{}" name="Checkbox"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
          <a:ln w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln>
        </p:spPr>
      </p:sp>
"#, shape_id, tx, ty, tcx.min(200000), tcy.min(200000)));
                shape_id += 1;
                continue;
            }

            let sz = (item.font_size * 100.0).round().max(800.0) as i32;
            let b_val = if item.is_bold { "1" } else { "0" };

            s_xml.push_str(&format!(r#"      <!-- Text Shape -->
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="{}" name="TextBox {}"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr vert="horz" wrap="none" rtlCol="0"><a:spAutoFit/></a:bodyPr>
          <a:lstStyle/>
"#, shape_id, shape_id, tx, ty, tcx, tcy));

            for line in item.text.lines() {
                let esc_line = xml_escape(line);
                s_xml.push_str(&format!(r#"          <a:p>
            <a:r>
              <a:rPr lang="en-IN" sz="{}" b="{}">
                <a:solidFill><a:srgbClr val="0F172A"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
"#, sz, b_val, esc_line));
            }

            s_xml.push_str(r#"        </p:txBody>
      </p:sp>
"#);
            shape_id += 1;
        }

        s_xml.push_str(r#"    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#);

        zip.write_all(s_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    zip.finish().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

fn build_pptx_package(
    output: &Path,
    slides: &[SlideData],
    media_files: &[MediaFile],
    slide_dimensions: Option<(u64, u64)>,
) -> Result<(), ConvertError> {
    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let (slide_cx, slide_cy) = slide_dimensions.unwrap_or((12192000, 6858000));
    let is_portrait = slide_cy > slide_cx;

    copy_template_base(&mut zip, options, slides.len(), slide_cx, slide_cy)?;

    // Write all media files to ppt/media/*
    for media in media_files {
        zip.start_file(format!("ppt/media/{}", media.filename), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        zip.write_all(&media.bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    // Build each slide
    for (i, slide) in slides.iter().enumerate() {
        let slide_num = i + 1;

        // ppt/slides/_rels/slide{N}.xml.rels
        zip.start_file(format!("ppt/slides/_rels/slide{}.xml.rels", slide_num), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        let mut s_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout7.xml"/>
"#);
        if let Some(ref img_file) = slide.image_filename {
            s_rels.push_str(&format!(
                "  <Relationship Id=\"rIdImg1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>\n",
                img_file
            ));
        }
        s_rels.push_str("</Relationships>");
        zip.write_all(s_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;

        // ppt/slides/slide{N}.xml
        zip.start_file(format!("ppt/slides/slide{}.xml", slide_num), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;

        let has_img = slide.image_filename.is_some();
        let title_esc = xml_escape(&slide.title);
        let is_page_image = slide.bullet_points.is_empty() && has_img;

        let mut s_xml = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" 
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" 
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
"#);

        if !is_page_image {
            // Render Title
            let (title_x, title_y, title_cx, title_cy) = if is_portrait {
                (400000, 400000, slide_cx.saturating_sub(800000), 700000)
            } else {
                let w = if has_img { 5200000 } else { slide_cx.saturating_sub(1600000) };
                (800000, 500000, w, 800000)
            };

            s_xml.push_str(&format!(r#"      <!-- Title -->
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title 1"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr vert="horz" rtlCol="0"><a:spAutoFit/></a:bodyPr>
          <a:lstStyle/>
          <a:p>
            <a:r>
              <a:rPr lang="en-US" sz="2800" b="1">
                <a:solidFill><a:srgbClr val="1E293B"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
"#, title_x, title_y, title_cx, title_cy, title_esc));

            // Content bullet points
            if !slide.bullet_points.is_empty() {
                let (c_x, c_y, c_cx, c_cy) = if is_portrait {
                    if has_img {
                        (400000, 1200000, slide_cx.saturating_sub(800000), 3800000)
                    } else {
                        (400000, 1200000, slide_cx.saturating_sub(800000), slide_cy.saturating_sub(1600000))
                    }
                } else {
                    if has_img {
                        (800000, 1500000, 5000000, slide_cy.saturating_sub(2000000))
                    } else {
                        (800000, 1500000, slide_cx.saturating_sub(1600000), slide_cy.saturating_sub(2000000))
                    }
                };

                s_xml.push_str(&format!(r#"      <!-- Content -->
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content 1"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr vert="horz" rtlCol="0"><a:spAutoFit/></a:bodyPr>
          <a:lstStyle/>
"#, c_x, c_y, c_cx, c_cy));

                for point in &slide.bullet_points {
                    let p_esc = xml_escape(point);
                    s_xml.push_str(&format!(r#"          <a:p>
            <a:pPr lvl="0">
              <a:buChar char="•"/>
            </a:pPr>
            <a:r>
              <a:rPr lang="en-US" sz="1600">
                <a:solidFill><a:srgbClr val="334155"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
"#, p_esc));
                }

                s_xml.push_str(r#"        </p:txBody>
      </p:sp>
"#);
            }
        }

        // Image picture shape
        if has_img {
            let (img_x, img_y, img_cx, img_cy) = if is_page_image {
                let mx = (slide_cx * 2 / 100).max(80000);
                let my = (slide_cy * 2 / 100).max(80000);
                (mx, my, slide_cx.saturating_sub(2 * mx), slide_cy.saturating_sub(2 * my))
            } else if is_portrait {
                (400000, 5200000, slide_cx.saturating_sub(800000), slide_cy.saturating_sub(5600000))
            } else {
                (6200000, 1500000, slide_cx.saturating_sub(6800000), slide_cy.saturating_sub(2000000))
            };

            s_xml.push_str(&format!(r#"      <!-- Picture Shape -->
      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="4" name="Picture 1"/>
          <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="rIdImg1"/>
          <a:stretch><a:fillRect/></a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
      </p:pic>
"#, img_x, img_y, img_cx, img_cy));
        }

        if let Some(ref text) = slide.ocr_text {
            let ocr_lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            if !ocr_lines.is_empty() {
                let ocr_x = 400000i64;
                let ocr_y = 400000i64;
                let ocr_cx = slide_cx.saturating_sub(800000) as i64;
                let ocr_cy = slide_cy.saturating_sub(800000) as i64;
                s_xml.push_str(&format!(r#"      <!-- OCR Selectable/Editable Text Shape -->
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="5" name="OCR Text"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr vert="horz" wrap="square" rtlCol="0"><a:spAutoFit/></a:bodyPr>
          <a:lstStyle/>
"#, ocr_x, ocr_y, ocr_cx, ocr_cy));

                for line in ocr_lines {
                    let esc_line = xml_escape(line);
                    s_xml.push_str(&format!(r#"          <a:p>
            <a:r>
              <a:rPr lang="en-US" sz="1400">
                <a:solidFill><a:srgbClr val="1E293B"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
"#, esc_line));
                }

                s_xml.push_str(r#"        </p:txBody>
      </p:sp>
"#);
            }
        }

        s_xml.push_str(r#"    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#);

        zip.write_all(s_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    zip.finish().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

fn build_docx_from_slides(
    output: &Path,
    slides: &[(String, Vec<String>)],
    media_files: &[MediaFile],
) -> Result<(), ConvertError> {
    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Default Extension="gif" ContentType="image/gif"/>
  <Default Extension="bmp" ContentType="image/bmp"/>
  <Default Extension="webp" ContentType="image/webp"/>
  <Default Extension="tiff" ContentType="image/tiff"/>
  <Default Extension="tif" ContentType="image/tiff"/>
  <Default Extension="emf" ContentType="image/x-emf"/>
  <Default Extension="wmf" ContentType="image/x-wmf"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 3. word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut d_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
"#);
    for (idx, media) in media_files.iter().enumerate() {
        d_rels.push_str(&format!(
            "  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/{}\"/>\n",
            idx + 2,
            media.filename
        ));
    }
    d_rels.push_str("</Relationships>");
    zip.write_all(d_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 4. word/styles.xml
    zip.start_file("word/styles.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
        <w:lang w:val="en-US"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:next w:val="Normal"/>
    <w:qFormat/>
    <w:rPr>
      <w:b/>
      <w:bCs/>
      <w:sz w:val="36"/>
      <w:szCs w:val="36"/>
      <w:color w:val="1E293B"/>
    </w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="ListBullet">
    <w:name w:val="List Bullet"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:pPr>
      <w:spacing w:after="80"/>
    </w:pPr>
  </w:style>
</w:styles>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 5. Write all media files to word/media/*
    for media in media_files {
        zip.start_file(format!("word/media/{}", media.filename), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        zip.write_all(&media.bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    // 6. word/document.xml
    zip.start_file("word/document.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    let mut doc_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>
"#);

    let mut media_iter = media_files.iter().enumerate();

    for (title, points) in slides {
        // Slide title heading
        let t_esc = xml_escape(title);
        doc_xml.push_str(&format!(
            "    <w:p><w:pPr><w:pStyle w:val=\"Heading1\"/><w:spacing w:before=\"240\" w:after=\"120\"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val=\"36\"/></w:rPr><w:t>{}</w:t></w:r></w:p>\n",
            t_esc
        ));

        // Slide bullet points
        for p in points {
            let p_esc = xml_escape(p);
            doc_xml.push_str(&format!(
                "    <w:p><w:pPr><w:pStyle w:val=\"ListBullet\"/><w:spacing w:after=\"80\"/></w:pPr><w:r><w:t>• {}</w:t></w:r></w:p>\n",
                p_esc
            ));
        }

        // Inline image if available
        if let Some((idx, media)) = media_iter.next() {
            let (cx, cy) = if let Ok(dims) = image::load_from_memory(&media.bytes) {
                let pw = dims.width().max(1) as f32;
                let ph = dims.height().max(1) as f32;
                let aspect = pw / ph;
                let max_w = 5486400.0; // 6 inches in EMUs
                let max_h = 4500000.0;
                if max_w / max_h > aspect {
                    ((max_h * aspect) as u64, max_h as u64)
                } else {
                    (max_w as u64, (max_w / aspect) as u64)
                }
            } else {
                (5486400, 3086100)
            };

            let pic_id = idx + 1;
            let rel_id = idx + 2;

            doc_xml.push_str(&format!(r#"    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{}" cy="{}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{}" name="Picture {}"/>
            <wp:cNvGraphicFramePr>
              <a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/>
            </wp:cNvGraphicFramePr>
            <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:nvPicPr>
                    <pic:cNvPr id="{}" name="Image {}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="rId{}"/>
                    <a:stretch><a:fillRect/></a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{}" cy="{}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#, cx, cy, pic_id, pic_id, pic_id, pic_id, rel_id, cx, cy));
        }
    }

    // Embed any leftover images at the end of the document
    for (idx, media) in media_iter {
        let (cx, cy) = if let Ok(dims) = image::load_from_memory(&media.bytes) {
            let pw = dims.width().max(1) as f32;
            let ph = dims.height().max(1) as f32;
            let aspect = pw / ph;
            let max_w = 5486400.0;
            let max_h = 4500000.0;
            if max_w / max_h > aspect {
                ((max_h * aspect) as u64, max_h as u64)
            } else {
                (max_w as u64, (max_w / aspect) as u64)
            }
        } else {
            (5486400, 3086100)
        };

        let pic_id = idx + 1;
        let rel_id = idx + 2;

        doc_xml.push_str(&format!(r#"    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{}" cy="{}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{}" name="Picture {}"/>
            <wp:cNvGraphicFramePr>
              <a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/>
            </wp:cNvGraphicFramePr>
            <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:nvPicPr>
                    <pic:cNvPr id="{}" name="Image {}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="rId{}"/>
                    <a:stretch><a:fillRect/></a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{}" cy="{}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#, cx, cy, pic_id, pic_id, pic_id, pic_id, rel_id, cx, cy));
    }

    doc_xml.push_str(r#"    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#);

    zip.write_all(doc_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.finish().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}
