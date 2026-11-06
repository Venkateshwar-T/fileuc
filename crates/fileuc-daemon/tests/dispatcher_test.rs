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
