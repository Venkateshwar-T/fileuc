use fileuc_convert::ocr::ocr_rgba_bytes;

#[test]
fn ocr_engine_initializes_and_runs() {
    // 100x40 blank image with black background
    let width = 100u32;
    let height = 40u32;
    let rgba = vec![255u8; (width * height * 4) as usize];

    let result = ocr_rgba_bytes(width, height, &rgba);
    // On Windows with OCR installed, this should return Ok("")
    assert!(result.is_ok(), "Windows OCR must run cleanly on RGBA bitmap");
}
