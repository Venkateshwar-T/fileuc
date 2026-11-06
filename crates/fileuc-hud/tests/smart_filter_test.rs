use fileuc_hud::smart_filter::filter_actions;
use fileuc_types::ConversionAction;
use std::path::PathBuf;

#[test]
fn images_get_image_actions() {
    let files = vec![PathBuf::from("a.png"), PathBuf::from("b.jpg")];
    let actions = filter_actions(&files, false);
    assert!(actions.contains(&ConversionAction::ToPng));
    assert!(actions.contains(&ConversionAction::ToJpg));
    assert!(actions.contains(&ConversionAction::ToWebp));
    assert!(actions.contains(&ConversionAction::ToPdf));
    assert!(!actions.contains(&ConversionAction::VideoToGif));
    assert!(actions.len() <= 8);
}

#[test]
fn videos_without_ffmpeg_get_no_media_actions() {
    let files = vec![PathBuf::from("clip.mp4")];
    let actions = filter_actions(&files, false);
    assert!(!actions.contains(&ConversionAction::VideoToGif));
    assert!(!actions.contains(&ConversionAction::VideoToMp3));
}

#[test]
fn videos_with_ffmpeg_get_media_actions() {
    let files = vec![PathBuf::from("clip.mp4")];
    let actions = filter_actions(&files, true);
    assert!(actions.contains(&ConversionAction::VideoToGif));
    assert!(actions.contains(&ConversionAction::VideoToMp3));
}

#[test]
fn multiple_pdfs_get_merge() {
    let files = vec![PathBuf::from("a.pdf"), PathBuf::from("b.pdf")];
    let actions = filter_actions(&files, false);
    assert!(actions.contains(&ConversionAction::MergePdf));
}

#[test]
fn mixed_files_get_zip_only() {
    let files = vec![PathBuf::from("a.png"), PathBuf::from("b.pdf"), PathBuf::from("c.docx")];
    let actions = filter_actions(&files, false);
    assert!(actions.contains(&ConversionAction::CreateZip));
}
