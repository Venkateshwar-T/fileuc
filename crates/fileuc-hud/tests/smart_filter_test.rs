use fileuc_hud::smart_filter::filter_actions;
use fileuc_types::ConversionAction;
use std::path::PathBuf;

#[test]
fn images_get_image_actions() {
    let files = vec![PathBuf::from("a.png"), PathBuf::from("b.jpg")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::ToPng,
            ConversionAction::ToJpg,
            ConversionAction::ToWebp,
            ConversionAction::ToPdf,
            ConversionAction::ReduceSize,
            ConversionAction::StripMetadata,
            ConversionAction::BulkRename,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn single_unencrypted_pdf_gets_lock_pdf() {
    let files = vec![PathBuf::from("doc.pdf")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::LockPdf,
            ConversionAction::OfficeGroup,
            ConversionAction::PdfOcr,
            ConversionAction::PdfToImages,
            ConversionAction::ReduceSize,
            ConversionAction::SplitPdf,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn multiple_unencrypted_pdfs_get_lock_pdf() {
    let files = vec![PathBuf::from("a.pdf"), PathBuf::from("b.pdf")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::LockPdf,
            ConversionAction::MergePdf,
            ConversionAction::OfficeGroup,
            ConversionAction::PdfOcr,
            ConversionAction::PdfToImages,
            ConversionAction::ReduceSize,
            ConversionAction::BulkRename,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn multiple_encrypted_pdfs_get_unlock_pdf() {
    let enc_path1 = std::env::temp_dir().join("fileuc_test_encrypted1.pdf");
    let enc_path2 = std::env::temp_dir().join("fileuc_test_encrypted2.pdf");
    let mock = b"%PDF-1.4\n1 0 obj\n<< /Encrypt 2 0 R >>\nendobj\ntrailer\n<< /Encrypt 2 0 R >>\n%%EOF";
    std::fs::write(&enc_path1, mock).unwrap();
    std::fs::write(&enc_path2, mock).unwrap();

    let files = vec![enc_path1.clone(), enc_path2.clone()];
    let actions = filter_actions(&files, false);
    let _ = std::fs::remove_file(enc_path1);
    let _ = std::fs::remove_file(enc_path2);

    assert_eq!(
        actions,
        vec![
            ConversionAction::UnlockPdf,
            ConversionAction::MergePdf,
            ConversionAction::OfficeGroup,
            ConversionAction::PdfOcr,
            ConversionAction::PdfToImages,
            ConversionAction::ReduceSize,
            ConversionAction::BulkRename,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn mixed_encrypted_and_unencrypted_pdfs_omit_both_lock_and_unlock() {
    let enc_path = std::env::temp_dir().join("fileuc_test_encrypted_mixed.pdf");
    let unenc_path = std::env::temp_dir().join("fileuc_test_unencrypted_mixed.pdf");
    std::fs::write(&enc_path, b"%PDF-1.4\n1 0 obj\n<< /Encrypt 2 0 R >>\nendobj\ntrailer\n<< /Encrypt 2 0 R >>\n%%EOF").unwrap();
    std::fs::write(&unenc_path, b"%PDF-1.4\n1 0 obj\n<< >>\nendobj\ntrailer\n<< >>\n%%EOF").unwrap();

    let files = vec![enc_path.clone(), unenc_path.clone()];
    let actions = filter_actions(&files, false);
    let _ = std::fs::remove_file(enc_path);
    let _ = std::fs::remove_file(unenc_path);

    assert_eq!(
        actions,
        vec![
            ConversionAction::MergePdf,
            ConversionAction::OfficeGroup,
            ConversionAction::PdfOcr,
            ConversionAction::PdfToImages,
            ConversionAction::ReduceSize,
            ConversionAction::BulkRename,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn single_encrypted_pdf_gets_unlock_pdf() {
    let enc_path = std::env::temp_dir().join("fileuc_test_encrypted.pdf");
    // Minimal mock PDF containing /Encrypt
    std::fs::write(&enc_path, b"%PDF-1.4\n1 0 obj\n<< /Encrypt 2 0 R >>\nendobj\ntrailer\n<< /Encrypt 2 0 R >>\n%%EOF").unwrap();

    let files = vec![enc_path.clone()];
    let actions = filter_actions(&files, false);
    let _ = std::fs::remove_file(enc_path);
    assert_eq!(
        actions,
        vec![
            ConversionAction::UnlockPdf,
            ConversionAction::OfficeGroup,
            ConversionAction::PdfOcr,
            ConversionAction::PdfToImages,
            ConversionAction::ReduceSize,
            ConversionAction::SplitPdf,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn excel_documents_get_zip_share() {
    let files = vec![PathBuf::from("sheet.xlsx")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::ExcelToPdf,
            ConversionAction::XlsxToCsv,
            ConversionAction::XlsxToJson,
            ConversionAction::CreateZip,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn word_documents_get_pdf_and_zip() {
    let files = vec![PathBuf::from("document.docx")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::DocToPdf,
            ConversionAction::CreateZip,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn pptx_documents_get_pdf_zip() {
    let files = vec![PathBuf::from("slides.pptx")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::PptxToPdf,
            ConversionAction::CreateZip,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn archive_gets_archive_actions() {
    let files = vec![PathBuf::from("archive.zip")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::ExtractArchive,
            ConversionAction::CreateZip,
            ConversionAction::Create7z,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn executables_and_other_files_get_archive_actions() {
    let files = vec![PathBuf::from("setup.exe")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::CreateZip,
            ConversionAction::Create7z,
            ConversionAction::Share,
        ]
    );

    let torrent = vec![PathBuf::from("ubuntu.torrent")];
    let actions_torrent = filter_actions(&torrent, false);
    assert_eq!(
        actions_torrent,
        vec![
            ConversionAction::CreateZip,
            ConversionAction::Create7z,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn mixed_files_get_zip_only() {
    let files = vec![PathBuf::from("a.png"), PathBuf::from("b.pdf"), PathBuf::from("c.docx")];
    let actions = filter_actions(&files, false);
    assert!(actions.contains(&ConversionAction::CreateZip));
}

#[test]
fn png_only_omits_to_png() {
    let files = vec![PathBuf::from("photo.png")];
    let actions = filter_actions(&files, false);
    assert!(!actions.contains(&ConversionAction::ToPng));
    assert!(actions.contains(&ConversionAction::ToJpg));
    assert!(actions.contains(&ConversionAction::ToWebp));
    assert!(actions.contains(&ConversionAction::ToPdf));
}

#[test]
fn jpg_only_omits_to_jpg() {
    let files = vec![PathBuf::from("photo.jpg")];
    let actions = filter_actions(&files, false);
    assert!(!actions.contains(&ConversionAction::ToJpg));
    assert!(!actions.contains(&ConversionAction::ToJpeg));
    assert!(actions.contains(&ConversionAction::ToPng));
    assert!(actions.contains(&ConversionAction::ToWebp));
}

#[test]
fn webp_only_omits_to_webp() {
    let files = vec![PathBuf::from("photo.webp")];
    let actions = filter_actions(&files, false);
    assert!(!actions.contains(&ConversionAction::ToWebp));
    assert!(actions.contains(&ConversionAction::ToPng));
    assert!(actions.contains(&ConversionAction::ToJpg));
}

#[test]
fn svg_gets_svg_to_webp() {
    let files = vec![PathBuf::from("vector.svg")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::SvgToPng,
            ConversionAction::SvgToJpg,
            ConversionAction::SvgToWebp,
            ConversionAction::CreateZip,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn markdown_gets_to_html() {
    let files = vec![PathBuf::from("readme.md")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::MdToPdf,
            ConversionAction::MdToTxt,
            ConversionAction::ToHtml,
            ConversionAction::CreateZip,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn text_gets_to_html() {
    let files = vec![PathBuf::from("notes.txt")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::TxtToPdf,
            ConversionAction::TxtToMd,
            ConversionAction::ToHtml,
            ConversionAction::CreateZip,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn html_gets_to_pdf() {
    let files = vec![PathBuf::from("index.html")];
    let actions = filter_actions(&files, false);
    assert_eq!(
        actions,
        vec![
            ConversionAction::HtmlToPdf,
            ConversionAction::CreateZip,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn audio_files_get_audio_actions() {
    let files = vec![PathBuf::from("song.wav")];
    let actions = filter_actions(&files, true);
    assert_eq!(
        actions,
        vec![
            ConversionAction::ToMp3,
            ConversionAction::ToOgg,
            ConversionAction::ToAac,
            ConversionAction::ToFlac,
            ConversionAction::ReduceSize,
            ConversionAction::StripMetadata,
            ConversionAction::Share,
        ]
    );
}

#[test]
fn office_group_has_docx_and_excel_children() {
    use fileuc_hud::smart_filter::get_action_children;
    let children = get_action_children(ConversionAction::OfficeGroup);
    assert_eq!(
        children,
        Some(&[ConversionAction::PdfToDocx, ConversionAction::PdfToExcel][..])
    );
}
