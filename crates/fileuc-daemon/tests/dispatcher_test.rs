use fileuc_daemon::dispatcher::{dispatch, ConversionRequest};
use fileuc_settings::store::{AppSettings, OutputMode};
use fileuc_types::ConversionAction;
use std::sync::{Arc, Mutex};

#[test]
fn dispatch_png_to_jpg() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("input.png");
    image::RgbaImage::new(10, 10).save(&src).unwrap();

    let settings = Arc::new(AppSettings {
        output_mode: OutputMode::NextToOriginal,
        ..AppSettings::default()
    });

    let progress_log = Arc::new(Mutex::new(Vec::new()));
    let log_clone = progress_log.clone();

    let handle = dispatch(
        ConversionRequest {
            files: vec![src],
            action: ConversionAction::ToJpg,
            settings,
        },
        move |p| {
            log_clone.lock().unwrap().push(p);
        },
    );

    let result = handle.join().unwrap();
    assert!(result.errors.is_empty());
    assert_eq!(result.outputs.len(), 1);
    assert!(result.outputs[0].exists());
    assert_eq!(result.outputs[0].extension().unwrap(), "jpg");

    let log = progress_log.lock().unwrap();
    assert!(!log.is_empty());
    assert!(log.last().map_or(false, |&p| (p - 1.0).abs() < 0.01));
}

#[test]
fn dispatch_docx_to_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("input.docx");

    let file = std::fs::File::create(&src).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("word/document.xml", options).unwrap();
    std::io::Write::write_all(
        &mut zip,
        br#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Document Content</w:t></w:r></w:p></w:body></w:document>"#,
    ).unwrap();
    zip.finish().unwrap();

    let settings = Arc::new(AppSettings {
        output_mode: OutputMode::NextToOriginal,
        ..AppSettings::default()
    });

    let handle = dispatch(
        ConversionRequest {
            files: vec![src],
            action: ConversionAction::DocToPdf,
            settings,
        },
        |_| {},
    );

    let result = handle.join().unwrap();
    assert!(result.errors.is_empty());
    assert_eq!(result.outputs.len(), 1);
    assert!(result.outputs[0].exists());
    assert_eq!(result.outputs[0].extension().unwrap(), "pdf");
}

#[test]
fn dispatch_pdf_to_docx() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("page.png");
    image::RgbaImage::new(10, 10).save(&img).unwrap();
    let src = tmp.path().join("input.pdf");
    fileuc_convert::pdf::images_to_pdf(&[img], &src).unwrap();

    let settings = Arc::new(AppSettings {
        output_mode: OutputMode::NextToOriginal,
        ..AppSettings::default()
    });

    let handle = dispatch(
        ConversionRequest {
            files: vec![src],
            action: ConversionAction::PdfToDocx,
            settings,
        },
        |_| {},
    );

    let result = handle.join().unwrap();
    if fileuc_convert::doc::find_pdf2docx().is_some() {
        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert_eq!(result.outputs.len(), 1);
        assert!(result.outputs[0].exists());
        assert_eq!(result.outputs[0].extension().unwrap(), "docx");
    }
}

#[test]
fn test_apply_output_mode_custom_folder_and_collision() {
    use fileuc_daemon::dispatcher::apply_output_mode;
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("photo.jpg");
    std::fs::write(&src, b"fake jpg").unwrap();

    let custom_dir = tmp.path().join("exports");
    std::fs::create_dir_all(&custom_dir).unwrap();

    // 1. Move to custom folder
    let out = apply_output_mode(src.clone(), &OutputMode::CustomFolder(custom_dir.clone()));
    assert_eq!(out, custom_dir.join("photo.jpg"));
    assert!(out.exists());
    assert!(!src.exists());

    // 2. Collision avoidance: create another photo.jpg at src, move to same custom folder
    std::fs::write(&src, b"new photo").unwrap();
    let out2 = apply_output_mode(src.clone(), &OutputMode::CustomFolder(custom_dir.clone()));
    assert_eq!(out2, custom_dir.join("photo (2).jpg"));
    assert!(out2.exists());
    assert!(custom_dir.join("photo.jpg").exists());
}

#[test]
fn test_apply_output_mode_directory() {
    use fileuc_daemon::dispatcher::apply_output_mode;
    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("my_pages");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("page_1.txt"), b"hello").unwrap();

    let custom_dir = tmp.path().join("custom_out");
    let out = apply_output_mode(src_dir.clone(), &OutputMode::CustomFolder(custom_dir.clone()));
    assert_eq!(out, custom_dir.join("my_pages"));
    assert!(out.exists());
    assert!(out.join("page_1.txt").exists());
    assert!(!src_dir.exists());
}

#[test]
fn test_dispatch_empty_files_no_panic() {
    let settings = Arc::new(AppSettings::default());
    let progress_log = Arc::new(Mutex::new(Vec::new()));
    let log_clone = progress_log.clone();

    let handle = dispatch(
        ConversionRequest {
            files: vec![],
            action: ConversionAction::ToPdf,
            settings,
        },
        move |p| {
            log_clone.lock().unwrap().push(p);
        },
    );

    let result = handle.join().unwrap();
    assert!(result.outputs.is_empty());
    assert!(result.errors.is_empty());
    let log = progress_log.lock().unwrap();
    assert_eq!(*log, vec![1.0]);
}

#[test]
fn test_dispatch_multi_file_monotonic_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let src1 = tmp.path().join("f1.png");
    let src2 = tmp.path().join("f2.png");
    image::RgbaImage::new(4, 4).save(&src1).unwrap();
    image::RgbaImage::new(4, 4).save(&src2).unwrap();

    let settings = Arc::new(AppSettings::default());
    let progress_log = Arc::new(Mutex::new(Vec::new()));
    let log_clone = progress_log.clone();

    let handle = dispatch(
        ConversionRequest {
            files: vec![src1, src2],
            action: ConversionAction::ToJpg,
            settings,
        },
        move |p| {
            log_clone.lock().unwrap().push(p);
        },
    );

    let result = handle.join().unwrap();
    assert_eq!(result.outputs.len(), 2);

    let log = progress_log.lock().unwrap();
    assert!(log.len() >= 4);
    // Monotonicity check
    for w in log.windows(2) {
        assert!(w[1] >= w[0], "Progress must be monotonic: {} then {}", w[0], w[1]);
    }
    assert!((log.last().unwrap() - 1.0).abs() < 0.001);
}

#[test]
fn test_dispatch_create_7z_with_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("document.txt");
    std::fs::write(&src, b"FileUC lightning fast 7z compression test").unwrap();

    let settings = Arc::new(AppSettings {
        output_mode: OutputMode::NextToOriginal,
        ..AppSettings::default()
    });

    let progress_log = Arc::new(Mutex::new(Vec::new()));
    let log_clone = progress_log.clone();

    let handle = dispatch(
        ConversionRequest {
            files: vec![src],
            action: ConversionAction::Create7z,
            settings,
        },
        move |p| {
            log_clone.lock().unwrap().push(p);
        },
    );

    let result = handle.join().unwrap();
    assert!(result.errors.is_empty());
    assert_eq!(result.outputs.len(), 1);
    assert!(result.outputs[0].exists());
    assert_eq!(result.outputs[0].extension().unwrap(), "7z");

    let log = progress_log.lock().unwrap();
    assert!(!log.is_empty());
    assert!((log.last().unwrap() - 1.0).abs() < 0.001);
}

#[test]
fn test_dispatch_create_zip_with_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("test.txt");
    std::fs::write(&src, b"FileUC streaming zip compression test").unwrap();

    let settings = Arc::new(AppSettings {
        output_mode: OutputMode::NextToOriginal,
        ..AppSettings::default()
    });

    let progress_log = Arc::new(Mutex::new(Vec::new()));
    let log_clone = progress_log.clone();

    let handle = dispatch(
        ConversionRequest {
            files: vec![src],
            action: ConversionAction::CreateZip,
            settings,
        },
        move |p| {
            log_clone.lock().unwrap().push(p);
        },
    );

    let result = handle.join().unwrap();
    assert!(result.errors.is_empty());
    assert_eq!(result.outputs.len(), 1);
    assert!(result.outputs[0].exists());
    assert_eq!(result.outputs[0].extension().unwrap(), "zip");

    let log = progress_log.lock().unwrap();
    assert!(!log.is_empty());
    assert!((log.last().unwrap() - 1.0).abs() < 0.001);
}
