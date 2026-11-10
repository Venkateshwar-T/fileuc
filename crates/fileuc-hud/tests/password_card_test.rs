use fileuc_hud::password_card::{render_password_card, PasswordCardState};
use fileuc_hud::theme::Theme;
use fileuc_hud::FontContext;
use std::path::PathBuf;
use tiny_skia::Pixmap;

#[test]
fn test_password_card_state_input_and_clear() {
    let mut state = PasswordCardState::new(
        PathBuf::from("C:\\test\\document.pdf"),
        1.0,
    );

    assert_eq!(state.file_name, "document.pdf");
    assert_eq!(state.password_input, "");

    state.push_char('S');
    state.push_char('e');
    state.push_char('c');
    state.push_char('r');
    state.push_char('e');
    state.push_char('t');
    assert_eq!(state.password_input, "Secret");

    state.pop_char();
    assert_eq!(state.password_input, "Secre");

    state.clear();
    assert_eq!(state.password_input, "");
}

#[test]
fn test_password_card_hit_testing() {
    let state = PasswordCardState::new(
        PathBuf::from("C:\\test\\document.pdf"),
        1.0,
    );
    let cx = 200.0;
    let cy = 200.0;

    // Center should hit card
    assert!(state.hit_test_card(0.0, 0.0, cx, cy));

    // Outside bounds should not hit card
    assert!(!state.hit_test_card(300.0, 300.0, cx, cy));

    // Submit button area is around cy + 50.0
    assert!(state.hit_test_submit(0.0, 50.0, cx, cy));

    // Close button area is at top right
    let (left, top, w, _) = state.card_bounds(cx, cy);
    let close_rx = (left + w - 22.0) - cx;
    let close_ry = (top + 22.0) - cy;
    assert!(state.hit_test_close(close_rx, close_ry, cx, cy));
}

#[test]
fn test_render_password_card() {
    let mut state = PasswordCardState::new(
        PathBuf::from("C:\\test\\document.pdf"),
        1.0,
    );
    state.push_char('P');
    state.push_char('a');
    state.push_char('s');
    state.push_char('s');
    // Advance spring across 60 frames (16ms per frame)
    for _ in 0..60 {
        state.tick(1.0 / 60.0);
    }
    assert!(state.pop_in.value() > 0.95);

    let mut pixmap = Pixmap::new(400, 400).unwrap();
    let theme = Theme::dark();
    let fonts = FontContext::new();

    render_password_card(&state, &theme, &fonts, &mut pixmap);
    let non_transparent = pixmap.data().chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(non_transparent > 1000, "Pixmap should have non-transparent pixels rendered");
}
