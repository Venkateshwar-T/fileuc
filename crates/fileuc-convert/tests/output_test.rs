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

#[test]
fn resolve_suffixed_locking_and_unlocking() {
    use fileuc_convert::output::resolve_suffixed_output_path;
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("document.pdf");
    std::fs::write(&src, b"fake").unwrap();

    // 1. Lock should create document_locked.pdf
    let locked = resolve_suffixed_output_path(&src, "_locked", "pdf", None);
    assert_eq!(locked, tmp.path().join("document_locked.pdf"));
    std::fs::write(&locked, b"fake locked").unwrap();

    // 2. Unlock from document_locked.pdf should create document_unlocked.pdf (strips _locked!)
    let unlocked = resolve_suffixed_output_path(&locked, "_unlocked", "pdf", Some("_locked"));
    assert_eq!(unlocked, tmp.path().join("document_unlocked.pdf"));
}
