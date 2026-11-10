pub mod docx_emitter;
pub mod layout_analyzer;
pub mod pdfium_extractor;
pub mod pptx_emitter;
pub mod table_detector;
pub mod types;
pub mod xlsx_emitter;

use fileuc_types::ConvertError;
use std::path::Path;
pub use types::*;

pub fn extract_document_ir(src: &Path) -> Result<DocumentIR, ConvertError> {
    let pdfium = crate::pdfium::init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(src, None)
        .map_err(|e| ConvertError::ConverterError(format!("Pdfium failed to load PDF: {}", e)))?;

    let mut doc_ir = DocumentIR::new(src.file_stem().map(|s| s.to_string_lossy().to_string()));

    for (page_idx, page) in document.pages().iter().enumerate() {
        let raw = pdfium_extractor::extract_raw_page_elements(&page, page_idx + 1)?;
        let mut page_ir = layout_analyzer::analyze_page_layout(raw);

        // Fallback for image-only / scanned pages:
        if page_ir.blocks.is_empty() && page_ir.tables.is_empty() && page_ir.images.is_empty() {
            let target_width = (page_ir.width_pt * 2.5).clamp(1600.0, 2400.0) as i32;
            if let Ok(img) = crate::pdfium::render_page_to_image_from_doc(&document, page_idx, target_width) {
                let mut png_bytes = Vec::new();
                if img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).is_ok() {
                    page_ir.images.push(ImageIR {
                        bounds: Rect::new(0.0, 0.0, page_ir.width_pt, page_ir.height_pt),
                        data: png_bytes,
                        format: ImageFormat::Png,
                    });
                }
            }
        }

        doc_ir.pages.push(page_ir);
    }

    Ok(doc_ir)
}

pub fn reconstruct_pdf_to_pptx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let doc_ir = extract_document_ir(src)?;
    pptx_emitter::emit_pptx(&doc_ir, output)
}

pub fn reconstruct_pdf_to_docx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let doc_ir = extract_document_ir(src)?;
    docx_emitter::emit_docx(&doc_ir, output)
}

pub fn reconstruct_pdf_to_xlsx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let doc_ir = extract_document_ir(src)?;
    xlsx_emitter::emit_xlsx(&doc_ir, output)
}
