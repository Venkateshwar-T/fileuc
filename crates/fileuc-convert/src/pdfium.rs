use fileuc_types::ConvertError;
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

static GLOBAL_PDFIUM: std::sync::OnceLock<Option<Pdfium>> = std::sync::OnceLock::new();

fn try_create_pdfium() -> Result<Pdfium, ConvertError> {
    let mut candidate_paths: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidate_paths.push(parent.join("pdfium.dll"));
            if let Some(grandparent) = parent.parent() {
                candidate_paths.push(grandparent.join("pdfium.dll"));
            }
        }
    }
    candidate_paths.push(PathBuf::from("pdfium.dll"));
    candidate_paths.push(PathBuf::from(r"C:\Program Files\FileUC\pdfium.dll"));

    if let Ok(cwd) = std::env::current_dir() {
        candidate_paths.push(cwd.join("pdfium.dll"));
        if let Some(p) = cwd.parent() {
            candidate_paths.push(p.join("pdfium.dll"));
            if let Some(gp) = p.parent() {
                candidate_paths.push(gp.join("pdfium.dll"));
            }
        }
    }

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

pub fn extract_text(src: &Path) -> Result<Vec<String>, ConvertError> {
    let pdfium = init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(src, None)
        .or_else(|_| pdfium.load_pdf_from_file(src, Some("")))
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
        .or_else(|_| pdfium.load_pdf_from_file(src, Some("")))
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
        .or_else(|_| pdfium.load_pdf_from_file(src, Some("")))
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
