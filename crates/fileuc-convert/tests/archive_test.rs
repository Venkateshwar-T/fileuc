use fileuc_convert::archive::{create_zip, extract_archive};

#[test]
fn zip_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let f1 = tmp.path().join("hello.txt");
    std::fs::write(&f1, b"hello world").unwrap();
    let zip_path = tmp.path().join("out.zip");
    create_zip(&[f1], &zip_path).unwrap();
    assert!(zip_path.exists());

    let extract_dir = tmp.path().join("extracted");
    extract_archive(&zip_path, &extract_dir).unwrap();
    let content = std::fs::read_to_string(extract_dir.join("hello.txt")).unwrap();
    assert_eq!(content, "hello world");
}
