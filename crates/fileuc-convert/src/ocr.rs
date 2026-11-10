use fileuc_types::ConvertError;
use std::path::Path;
use windows::core::HSTRING;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

pub fn image_to_text(src: &Path, output: &Path) -> Result<String, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let img = image::open(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open image for OCR: {}", e)))?;
    let (width, height) = (img.width(), img.height());
    let rgba = img.to_rgba8();

    let text = ocr_rgba_bytes(width, height, &rgba)?;
    std::fs::write(output, &text).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(text)
}

pub fn run_ocr_on_image(img: &image::DynamicImage) -> Result<String, ConvertError> {
    let rgba = img.to_rgba8();
    ocr_rgba_bytes(img.width(), img.height(), &rgba)
}

pub fn ocr_rgba_bytes(width: u32, height: u32, rgba_data: &[u8]) -> Result<String, ConvertError> {
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| ConvertError::EngineMissing(format!("Windows OCR engine unavailable: {}", e)))?;

    let writer = DataWriter::new()
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    writer.WriteBytes(rgba_data)
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    let buffer = writer.DetachBuffer()
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

    let software_bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Rgba8,
        width as i32,
        height as i32,
    ).map_err(|e| ConvertError::ConverterError(e.to_string()))?;

    let ocr_result = engine
        .RecognizeAsync(&software_bitmap)
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?
        .get()
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

    let text_hstring: HSTRING = ocr_result
        .Text()
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

    Ok(text_hstring.to_string())
}
