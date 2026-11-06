use fileuc_hud::drop_target::{create_mock_hdrop, parse_hdrop_buffer};
use std::path::PathBuf;

#[test]
fn test_parse_hdrop_extracts_paths() {
    let paths = vec![
        PathBuf::from(r"C:\Users\test\photo.png"),
        PathBuf::from(r"D:\docs\report.pdf"),
    ];
    let buf = create_mock_hdrop(&paths);
    let extracted = parse_hdrop_buffer(&buf).unwrap();
    assert_eq!(extracted, paths);
}

#[test]
fn test_parse_hdrop_empty_returns_empty_vec() {
    let buf = create_mock_hdrop(&[]);
    let extracted = parse_hdrop_buffer(&buf).unwrap();
    assert!(extracted.is_empty());
}
