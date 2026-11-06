use fileuc_convert::output::resolve_output_path;

#[test]
fn resolve_basic_extension_change() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("photo.png");
    std::fs::write(&src, b"fake").unwrap();
    let out = resolve_output_path(&src, "jpg");
    assert_eq!(out, tmp.path().join("photo.jpg"));
}

#[test]
fn resolve_avoids_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("photo.png");
    std::fs::write(&src, b"fake").unwrap();
    std::fs::write(tmp.path().join("photo.jpg"), b"exists").unwrap();
    let out = resolve_output_path(&src, "jpg");
    assert_eq!(out, tmp.path().join("photo (2).jpg"));
}

#[test]
fn resolve_never_overwrites_source() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("photo.png");
    std::fs::write(&src, b"fake").unwrap();
    let out = resolve_output_path(&src, "png");
    assert_eq!(out, tmp.path().join("photo (2).png"));
}
