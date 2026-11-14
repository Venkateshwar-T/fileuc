use fileuc_types::ConvertError;
use std::path::Path;

pub fn pdf_to_searchable_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    if let Some(python_exe) = crate::doc::find_pdf2docx() {
        run_ocr_searchable_converter(&python_exe, src, output)?;
        if output.exists() {
            if let Ok(meta) = std::fs::metadata(output) {
                if meta.len() > 0 {
                    guard.disarm();
                    return Ok(());
                }
            }
        }
        return Err(ConvertError::ConverterError(
            "Searchable PDF OCR generation failed.".to_string(),
        ));
    }

    Err(ConvertError::EngineMissing(
        "Office & Document Processing Engine is not installed. Please open FileUC Settings -> Plugins to install it.".to_string(),
    ))
}

fn run_ocr_searchable_converter(python_exe: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    let mut cmd = std::process::Command::new(python_exe);
    let script = r#"
import sys
import numpy as np
import pymupdf
from rapidocr_onnxruntime import RapidOCR

src = sys.argv[1]
dst = sys.argv[2]

try:
    engine = RapidOCR()
    doc = pymupdf.open(src)

    for page_idx in range(len(doc)):
        page = doc[page_idx]
        
        # Check if page already has abundant text
        existing_text = page.get_text().strip()
        if len(existing_text) > 80:
            # Page already has text layer, skip OCR injection
            continue

        # Render page to high-res pixmap (150 DPI)
        pix = page.get_pixmap(dpi=150)
        img_np = np.frombuffer(pix.samples, dtype=np.uint8).reshape((pix.height, pix.width, pix.n))
        
        if pix.n == 4:
            import cv2
            img_np = cv2.cvtColor(img_np, cv2.COLOR_RGBA2BGR)
        elif pix.n == 3:
            import cv2
            img_np = cv2.cvtColor(img_np, cv2.COLOR_RGB2BGR)

        ocr_results, _ = engine(img_np)
        if not ocr_results:
            continue

        scale_x = page.rect.width / pix.width
        scale_y = page.rect.height / pix.height

        for item in ocr_results:
            box, text, score = item
            clean_text = str(text).strip()
            if not clean_text:
                continue

            x0 = min(p[0] for p in box) * scale_x
            y0 = min(p[1] for p in box) * scale_y
            x1 = max(p[0] for p in box) * scale_x
            y1 = max(p[1] for p in box) * scale_y
            
            font_size = max(5.0, (y1 - y0) * 0.8)
            # render_mode=3 means invisible text (searchable / selectable only)
            try:
                page.insert_text(
                    pymupdf.Point(x0, y1),
                    clean_text,
                    fontsize=font_size,
                    render_mode=3
                )
            except Exception:
                pass

    doc.save(dst, garbage=4, deflate=True)
    doc.close()
except Exception as e:
    sys.stderr.write(f"ocr_searchable error: {e}\n")
    sys.exit(1)
"#;

    let abs_src = crate::pptx::clean_windows_path(src);
    let abs_dst = crate::pptx::clean_windows_path(output);

    cmd.args(["-c", script, &abs_src, &abs_dst]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd.status().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    if !status.success() {
        return Err(ConvertError::ConverterError("OCR searchable PDF failed".to_string()));
    }
    Ok(())
}
