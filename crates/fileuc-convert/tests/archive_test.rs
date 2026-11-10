use fileuc_convert::archive::{
    create_7z, create_7z_with_progress, create_zip, create_zip_with_progress, extract_archive,
    find_7z,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

#[test]
fn zip_with_progress_emits_updates() {
    let tmp = tempfile::tempdir().unwrap();
    let f1 = tmp.path().join("data.txt");
    std::fs::write(&f1, vec![b'A'; 64 * 1024]).unwrap();

    let zip_path = tmp.path().join("out_prog.zip");
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();

    create_zip_with_progress(&[f1], &zip_path, move |_p| {
        c.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();

    assert!(zip_path.exists());
    assert!(count.load(Ordering::Relaxed) > 0);
}

#[test]
fn sevenz_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let f1 = tmp.path().join("data.txt");
    std::fs::write(&f1, b"FileUC Embedded 7z Engine Roundtrip!").unwrap();

    let sz_path = tmp.path().join("archive.7z");
    create_7z(&[f1], &sz_path).unwrap();
    assert!(sz_path.exists());

    let extract_dir = tmp.path().join("extracted_7z");
    extract_archive(&sz_path, &extract_dir).unwrap();
    let content = std::fs::read_to_string(extract_dir.join("data.txt")).unwrap();
    assert_eq!(content, "FileUC Embedded 7z Engine Roundtrip!");
}

#[test]
fn sevenz_with_progress_emits_updates() {
    let tmp = tempfile::tempdir().unwrap();
    let f1 = tmp.path().join("data.bin");
    std::fs::write(&f1, vec![b'X'; 256 * 1024]).unwrap();

    let sz_path = tmp.path().join("out_prog.7z");
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();

    create_7z_with_progress(&[f1], &sz_path, move |_p| {
        c.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();

    assert!(sz_path.exists());
    assert!(count.load(Ordering::Relaxed) > 0);
}

#[test]
fn test_find_7z_detection() {
    let found = find_7z();
    println!("find_7z returned: {:?}", found);
    if let Some(p) = found {
        assert!(p.exists() || p.to_string_lossy() == "7z");
    }
}

#[test]
fn test_compress_user_test_setup_exe() {
    let test_file = std::path::PathBuf::from(r"D:\Coding Files\Major Projects\FileUC\target\test-setup.exe");
    if !test_file.exists() {
        println!("test-setup.exe not found at target/test-setup.exe, skipping");
        return;
    }

    let out_archive = std::path::PathBuf::from(r"D:\Coding Files\Major Projects\FileUC\target\test_setup_compressed.7z");
    let _ = std::fs::remove_file(&out_archive);

    let original_size = test_file.metadata().unwrap().len();
    println!("\n=======================================================");
    println!("Starting 7z compression on test-setup.exe ({} bytes, {:.2} MB)...", original_size, original_size as f64 / (1024.0 * 1024.0));
    println!("=======================================================");
    let start = std::time::Instant::now();
    let progress_snapshots = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let snaps = progress_snapshots.clone();

    create_7z_with_progress(&[test_file.clone()], &out_archive, move |p| {
        let pct = (p * 100.0) as u32;
        snaps.lock().unwrap().push(pct);
        if pct % 10 == 0 || pct == 88 || pct >= 95 {
            println!("  [Progress callback] -> {}%", pct);
        }
    })
    .unwrap();

    let duration = start.elapsed();
    println!("Completed in {:.2}s!", duration.as_secs_f64());
    let list = progress_snapshots.lock().unwrap().clone();
    println!("Total progress callbacks emitted: {}", list.len());
    assert!(out_archive.exists());
    let compressed_len = out_archive.metadata().unwrap().len();
    println!("Compressed size: {} bytes ({:.2} MB)", compressed_len, compressed_len as f64 / (1024.0 * 1024.0));
    println!("Last progress snapshot: {:?}", list.last());

    assert!(out_archive.exists());
    let _ = std::fs::remove_file(&out_archive);
}
