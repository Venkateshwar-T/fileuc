use fileuc_types::ConvertError;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone)]
struct SlideItem {
    text: String,
    image: Option<image::DynamicImage>,
}

pub use crate::output::clean_windows_path;

/// Converts a PPTX presentation to PDF.
pub fn pptx_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    // 1. Try PowerPoint COM automation first on Windows
    if try_powerpoint_com_convert(src, output).is_ok() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                guard.disarm();
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
            let lower = name.to_ascii_lowercase();
            let is_raster_img = lower.ends_with(".png")
                || lower.ends_with(".jpg")
                || lower.ends_with(".jpeg")
                || lower.ends_with(".bmp")
                || lower.ends_with(".webp");
            if name.starts_with("ppt/media/") && is_raster_img && !entry.is_dir() {
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

    render_pptx_slides_to_pdf(&slides_data, output)?;
    guard.disarm();
    Ok(())
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
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(45);
    let mut finished = false;
    let mut exit_status = None;

    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => {
                finished = true;
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(ConvertError::ConverterError(format!("PowerPoint COM error: {}", e)));
            }
        }
    }

    if !finished {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ConvertError::ConverterError("PowerPoint COM conversion timed out".to_string()));
    }

    let status = exit_status.ok_or_else(|| {
        ConvertError::ConverterError("PowerPoint COM process exited without status".to_string())
    })?;
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

        content.save_state();
        content.restore_state();

        pdf.stream(content_ref, &content.finish());
    }

    std::fs::write(output, pdf.finish()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}
