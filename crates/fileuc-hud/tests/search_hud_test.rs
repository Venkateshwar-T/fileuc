use fileuc_hud::font::FontContext;
use fileuc_hud::search_hud::{
    compute_chip_layout, expand_to_leaf_actions, get_placeholders_for_actions, match_actions,
    SearchHudState, SEARCH_BAR_HEIGHT_DIP, SEARCH_BAR_WIDTH_DIP,
};
use fileuc_types::ConversionAction;

#[test]
fn test_expand_to_leaf_actions() {
    let input = vec![
        ConversionAction::ToPng,
        ConversionAction::IconGroup,
        ConversionAction::Share,
    ];
    let expanded = expand_to_leaf_actions(&input);

    assert_eq!(expanded.len(), 4);
    assert_eq!(expanded[0], ConversionAction::ToPng);
    assert_eq!(expanded[1], ConversionAction::ToIco);
    assert_eq!(expanded[2], ConversionAction::CreateIconPack);
    assert_eq!(expanded[3], ConversionAction::Share);
}

#[test]
fn test_expand_office_group() {
    let input = vec![
        ConversionAction::OfficeGroup,
        ConversionAction::SplitPdf,
        ConversionAction::LockPdf,
    ];
    let expanded = expand_to_leaf_actions(&input);

    assert!(expanded.contains(&ConversionAction::PdfToDocx));
    assert!(expanded.contains(&ConversionAction::PdfToExcel));
    assert!(expanded.contains(&ConversionAction::SplitPdf));
    assert!(expanded.contains(&ConversionAction::LockPdf));
    assert!(!expanded.contains(&ConversionAction::OfficeGroup));
}

#[test]
fn test_get_placeholders() {
    let actions = vec![
        ConversionAction::ToPng,
        ConversionAction::ToSvg,
        ConversionAction::Share,
    ];
    let placeholders = get_placeholders_for_actions(&actions);

    assert_eq!(placeholders.len(), 3);
    assert_eq!(placeholders[0], "Convert to PNG");
    assert_eq!(placeholders[1], "Convert to SVG");
    assert_eq!(placeholders[2], "Share to Mobile");
}

#[test]
fn test_keyword_matching_exact_and_prefix() {
    let supported = vec![
        ConversionAction::ToPng,
        ConversionAction::ToJpg,
        ConversionAction::ToWebp,
        ConversionAction::ToSvg,
        ConversionAction::ToIco,
        ConversionAction::CreateIconPack,
        ConversionAction::ReduceSize,
        ConversionAction::Share,
    ];

    // 1. "to png" should prioritize ToPng
    let matches = match_actions("to png", &supported);
    assert!(!matches.is_empty());
    assert_eq!(matches[0], ConversionAction::ToPng);

    // 2. "png" exact
    let matches = match_actions("png", &supported);
    assert_eq!(matches[0], ConversionAction::ToPng);

    // 3. "svg"
    let matches = match_actions("svg", &supported);
    assert_eq!(matches[0], ConversionAction::ToSvg);

    // 4. "ico" should match ToIco and CreateIconPack
    let matches = match_actions("ico", &supported);
    assert!(matches.contains(&ConversionAction::ToIco));
    assert!(matches.contains(&ConversionAction::CreateIconPack));

    // 5. "pack" should prioritize CreateIconPack
    let matches = match_actions("pack", &supported);
    assert_eq!(matches[0], ConversionAction::CreateIconPack);

    // 6. "share" matches Share
    let matches = match_actions("share", &supported);
    assert_eq!(matches[0], ConversionAction::Share);

    // 7. "reduce" matches ReduceSize
    let matches = match_actions("reduce", &supported);
    assert_eq!(matches[0], ConversionAction::ReduceSize);

    // 8. empty query returns first up to 6
    let matches = match_actions("", &supported);
    assert_eq!(matches.len(), 6);
    assert_eq!(matches[0], ConversionAction::ToPng);
}

#[test]
fn test_pdf_keyword_matching() {
    let supported = vec![
        ConversionAction::LockPdf,
        ConversionAction::PdfToDocx,
        ConversionAction::PdfToExcel,
        ConversionAction::PdfOcr,
        ConversionAction::PdfToImages,
        ConversionAction::ReduceSize,
        ConversionAction::SplitPdf,
        ConversionAction::Share,
    ];

    // "split"
    let matches = match_actions("split", &supported);
    assert_eq!(matches[0], ConversionAction::SplitPdf);

    // "word" or "docx"
    let matches = match_actions("word", &supported);
    assert_eq!(matches[0], ConversionAction::PdfToDocx);

    let matches = match_actions("docx", &supported);
    assert_eq!(matches[0], ConversionAction::PdfToDocx);

    // "ocr"
    let matches = match_actions("ocr", &supported);
    assert_eq!(matches[0], ConversionAction::PdfOcr);

    // "excel"
    let matches = match_actions("excel", &supported);
    assert_eq!(matches[0], ConversionAction::PdfToExcel);

    // "lock"
    let matches = match_actions("lock", &supported);
    assert_eq!(matches[0], ConversionAction::LockPdf);
}

#[test]
fn test_chip_layout_adaptive_no_overflow() {
    let fonts = FontContext::new();
    let actions = vec![
        ConversionAction::ToPng,
        ConversionAction::ToJpg,
        ConversionAction::ToWebp,
        ConversionAction::ToSvg,
        ConversionAction::ToIco,
        ConversionAction::CreateIconPack,
    ];

    let bar_w = SEARCH_BAR_WIDTH_DIP;
    let bar_h = SEARCH_BAR_HEIGHT_DIP;
    let bar_left = -bar_w / 2.0;
    let bar_top = -bar_h / 2.0;

    let rects = compute_chip_layout(
        &actions,
        false,
        bar_left,
        bar_top,
        bar_w,
        bar_h,
        1.0,
        false, // below
        &fonts,
    );

    assert_eq!(rects.len(), actions.len());

    for (x, y, w, h) in rects {
        // Every chip must fit within the search bar's horizontal span
        assert!(
            x >= bar_left - 0.1,
            "Chip left ({}) out of bounds ({})",
            x,
            bar_left
        );
        assert!(
            x + w <= bar_left + bar_w + 0.1,
            "Chip right ({}) exceeded bar right ({})",
            x + w,
            bar_left + bar_w
        );
        // Chip must be positioned below the search bar
        assert!(y >= bar_top + bar_h, "Chip y ({}) must be below bar", y);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }
}

#[test]
fn test_chip_layout_flip_above() {
    let fonts = FontContext::new();
    let actions = vec![
        ConversionAction::SplitPdf,
        ConversionAction::MergePdf,
        ConversionAction::ReduceSize,
    ];

    let bar_w = SEARCH_BAR_WIDTH_DIP;
    let bar_h = SEARCH_BAR_HEIGHT_DIP;
    let bar_left = -bar_w / 2.0;
    let bar_top = -bar_h / 2.0;

    let rects = compute_chip_layout(
        &actions,
        false,
        bar_left,
        bar_top,
        bar_w,
        bar_h,
        1.0,
        true, // flip_above
        &fonts,
    );

    assert_eq!(rects.len(), actions.len());
    for (_x, y, _w, h) in rects {
        // Every chip must be strictly above the search bar
        assert!(y + h <= bar_top, "Chip top ({}) must be above bar top ({})", y + h, bar_top);
    }
}

#[test]
fn test_search_hud_state_text_mutation() {
    let fonts = FontContext::new();
    let supported = vec![
        ConversionAction::ToPng,
        ConversionAction::ToJpg,
        ConversionAction::ToWebp,
        ConversionAction::ToSvg,
    ];

    let mut state = SearchHudState::new(&supported, 1.0, false, &fonts);
    assert_eq!(state.matched_actions.len(), 4);

    state.push_char('s', &fonts);
    state.push_char('v', &fonts);
    state.push_char('g', &fonts);
    assert_eq!(state.input_text, "svg");
    assert_eq!(state.best_match_action(), Some(ConversionAction::ToSvg));

    state.pop_char(&fonts);
    assert_eq!(state.input_text, "sv");

    state.clear(&fonts);
    assert_eq!(state.input_text, "");
    assert_eq!(state.matched_actions.len(), 4);
}

#[test]
fn test_fuzzy_typo_matching() {
    let supported = vec![
        ConversionAction::ToPng,
        ConversionAction::ToJpg,
        ConversionAction::ToWebp,
        ConversionAction::Share,
        ConversionAction::ReduceSize,
    ];

    // Typo: "pnf" -> ToPng
    let matches = match_actions("pnf", &supported);
    assert!(!matches.is_empty(), "pnf should fuzzy match ToPng");
    assert_eq!(matches[0], ConversionAction::ToPng);

    // Typo: "shre" -> Share
    let matches = match_actions("shre", &supported);
    assert!(!matches.is_empty(), "shre should fuzzy match Share");
    assert_eq!(matches[0], ConversionAction::Share);

    // Typo: "reduc" -> ReduceSize
    let matches = match_actions("reduc", &supported);
    assert!(!matches.is_empty(), "reduc should fuzzy match ReduceSize");
    assert_eq!(matches[0], ConversionAction::ReduceSize);
}

#[test]
fn test_token_keyword_detection_with_noise() {
    let supported = vec![
        ConversionAction::ToPng,
        ConversionAction::ToJpg,
        ConversionAction::Share,
        ConversionAction::CreateZip,
    ];

    // User types unrecognized characters followed by keyword "share"
    let matches = match_actions("dakhfdk share", &supported);
    assert!(!matches.is_empty(), "'dakhfdk share' must match Share");
    assert_eq!(matches[0], ConversionAction::Share);

    // User types "random into png"
    let matches = match_actions("random into png", &supported);
    assert!(!matches.is_empty(), "'random into png' must match ToPng");
    assert_eq!(matches[0], ConversionAction::ToPng);
}

#[test]
fn test_rich_synonyms_matching() {
    let supported = vec![
        ConversionAction::ReduceSize,
        ConversionAction::Share,
        ConversionAction::MergePdf,
        ConversionAction::SplitPdf,
        ConversionAction::LockPdf,
        ConversionAction::UnlockPdf,
    ];

    // Synonyms for ReduceSize: "reduction", "shrink", "downsize"
    let matches = match_actions("reduction", &supported);
    assert_eq!(matches[0], ConversionAction::ReduceSize);
    let matches = match_actions("shrink", &supported);
    assert_eq!(matches[0], ConversionAction::ReduceSize);

    // Synonyms for Share: "send", "transfer", "airdrop", "beam"
    let matches = match_actions("send", &supported);
    assert_eq!(matches[0], ConversionAction::Share);
    let matches = match_actions("transfer", &supported);
    assert_eq!(matches[0], ConversionAction::Share);

    // Synonyms for MergePdf: "join", "attach", "combine"
    let matches = match_actions("join", &supported);
    assert_eq!(matches[0], ConversionAction::MergePdf);
    let matches = match_actions("attach", &supported);
    assert_eq!(matches[0], ConversionAction::MergePdf);

    // Synonyms for SplitPdf: "pages", "divide", "separate"
    let matches = match_actions("pages", &supported);
    assert_eq!(matches[0], ConversionAction::SplitPdf);

    // Synonyms for LockPdf: "password", "protect", "encrypt"
    let matches = match_actions("password", &supported);
    assert_eq!(matches[0], ConversionAction::LockPdf);
}

#[test]
fn test_unsupported_action_rejection() {
    let fonts = FontContext::new();
    // Suppose user has selected a folder where only CreateZip, Create7z, Share are supported
    let supported = vec![
        ConversionAction::CreateZip,
        ConversionAction::Create7z,
        ConversionAction::Share,
    ];
    let mut state = SearchHudState::new(&supported, 1.0, false, &fonts);

    // User types "compress" which matches archive creation (CreateZip/Create7z) in supported
    state.clear(&fonts);
    state.paste_text("compress", &fonts);
    let matched = state.best_match_action();
    assert!(matched.is_some());
    assert!(state.supported_actions.contains(&matched.unwrap()));

    // Now user types "lock" or "password" which is NOT in supported_actions for folder
    state.clear(&fonts);
    state.paste_text("lock", &fonts);
    assert_eq!(state.best_match_action(), None);

    state.clear(&fonts);
    state.paste_text("password", &fonts);
    assert_eq!(state.best_match_action(), None);
}

#[test]
fn test_path_truncation() {
    use fileuc_hud::search_hud::truncate_path_middle;

    let short = "C:\\Users\\Desktop";
    assert_eq!(truncate_path_middle(short, 40), short);

    let long = "C:\\Users\\Sharad Chand\\VeryLongDirectoryPathThatExceedsLimit\\Subfolder\\file.txt";
    let truncated = truncate_path_middle(long, 30);
    assert!(truncated.contains("..."));
    assert!(truncated.chars().count() <= 30);
}

#[test]
fn test_clipboard_preview_overall_bounds() {
    use fileuc_hud::search_hud::ClipboardPreview;
    use std::path::PathBuf;

    let fonts = FontContext::new();
    let supported = vec![ConversionAction::ToPng, ConversionAction::Share];
    let mut state = SearchHudState::new(&supported, 1.0, false, &fonts);
    state.clipboard_title = Some("Image from Clipboard".to_string());
    state.clipboard_subtitle = Some("Saves to: C:\\Users\\Sharad Chand\\Desktop".to_string());

    let (_x1, _y1, w1, h1) = state.overall_bounds(200.0, 200.0);

    // Now set an image preview
    state.set_clipboard_preview(ClipboardPreview::Image(PathBuf::from("dummy.png")), &fonts);
    let (_x2, _y2, w2, h2) = state.overall_bounds(200.0, 200.0);

    // Height with image preview should be larger than without preview
    assert!(h2 > h1);
    assert!(w2 >= w1);
}

#[test]
fn test_empty_state_receive_send_quick_qr_matching() {
    let empty_state_actions = vec![
        ConversionAction::ReceiveFiles,
        ConversionAction::SendFiles,
        ConversionAction::QuickQr,
    ];

    assert_eq!(ConversionAction::ReceiveFiles.label(), "Receive");
    assert_eq!(ConversionAction::SendFiles.label(), "Send");
    assert_eq!(ConversionAction::QuickQr.label(), "QR Generator");

    let defaults = match_actions("", &empty_state_actions);
    assert_eq!(defaults, empty_state_actions);
    assert!(!defaults.contains(&ConversionAction::Share));

    let recv = match_actions("receive", &empty_state_actions);
    assert_eq!(recv[0], ConversionAction::ReceiveFiles);

    let upload = match_actions("upload", &empty_state_actions);
    assert_eq!(upload[0], ConversionAction::ReceiveFiles);

    let send = match_actions("send", &empty_state_actions);
    assert_eq!(send[0], ConversionAction::SendFiles);

    let qr = match_actions("qr", &empty_state_actions);
    assert_eq!(qr[0], ConversionAction::QuickQr);

    // Searchable Receive chip even when file conversion & Share actions exist
    let with_files_actions = vec![
        ConversionAction::ToPng,
        ConversionAction::ToWebp,
        ConversionAction::ReduceSize,
        ConversionAction::Share,
        ConversionAction::ReceiveFiles,
    ];
    let recv_filtered = match_actions("receive", &with_files_actions);
    assert_eq!(recv_filtered, vec![ConversionAction::ReceiveFiles]);
}
