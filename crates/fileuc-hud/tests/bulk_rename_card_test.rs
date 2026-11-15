use fileuc_hud::bulk_rename_card::{render_bulk_rename_card, BulkRenameCardState};
use fileuc_hud::font::FontContext;
use fileuc_hud::theme::Theme;
use tiny_skia::Pixmap;

#[test]
fn test_bulk_rename_state_input_and_sanitize() {
    let mut state = BulkRenameCardState::new(5, "jpg", 1.0);
    assert_eq!(state.file_count, 5);
    assert_eq!(state.sample_ext, ".jpg");

    state.paste_text("My:Vacation*Photos?2026");
    // Invalid Windows filename characters ':', '*', '?' must be stripped
    assert_eq!(state.input_text, "MyVacationPhotos2026");

    state.pop_char();
    assert_eq!(state.input_text, "MyVacationPhotos202");
}

#[test]
fn test_render_bulk_rename_card_with_long_preview_truncation() {
    let mut state = BulkRenameCardState::new(12, ".jpeg", 1.0);
    // Paste an extremely long base name that would overflow normal boundaries
    state.paste_text("Extremely_Long_Custom_Project_Archive_Name_For_Testing_Preview_Bounds");

    for _ in 0..60 {
        state.tick(1.0 / 60.0);
    }
    assert!(state.pop_in.value() > 0.95);

    let theme = Theme::resolve("dark");
    let fonts = FontContext::new();
    let mut pixmap = Pixmap::new(400, 300).unwrap();

    // Verify render runs cleanly without panicking and bounds-checks the text
    render_bulk_rename_card(&state, &theme, &fonts, &mut pixmap);

    let non_transparent = pixmap.data().chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(non_transparent > 1000, "PixMap must contain rendered elements");
}

#[test]
fn test_bulk_rename_hit_testing() {
    let state = BulkRenameCardState::new(3, "png", 1.0);
    let cx = 200.0;
    let cy = 150.0;

    assert!(state.hit_test_card(0.0, 0.0, cx, cy));
    assert!(!state.hit_test_card(300.0, 300.0, cx, cy));
}
