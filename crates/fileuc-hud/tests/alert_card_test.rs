use fileuc_hud::alert_card::{render_alert_card, AlertCardState};
use fileuc_hud::theme::Theme;
use fileuc_hud::FontContext;
use tiny_skia::Pixmap;

#[test]
fn test_alert_card_creation_and_bounds() {
    let state = AlertCardState::new("Cannot merge a locked PDF. Unlock first.", 1.0);
    assert_eq!(state.title, "Cannot Merge PDF");
    assert_eq!(state.message, "Cannot merge a locked PDF. Unlock first.");

    let cx = 200.0;
    let cy = 200.0;
    let (l, t, w, h) = state.card_bounds(cx, cy);
    assert_eq!(w, 340.0);
    assert_eq!(h, 168.0);
    assert_eq!(l, 200.0 - 170.0);
    assert_eq!(t, 200.0 - 84.0);

    // Inside card hit test
    assert!(state.hit_test_card(0.0, 0.0, cx, cy));
    // Outside card hit test
    assert!(!state.hit_test_card(250.0, 250.0, cx, cy));
}

#[test]
fn test_alert_card_hit_test_close_and_ok() {
    let state = AlertCardState::new("Cannot merge a locked PDF. Unlock first.", 1.0);
    let cx = 200.0;
    let cy = 200.0;

    let (left, top, w, _) = state.card_bounds(cx, cy);
    let close_x = left + w - 22.0;
    let close_y = top + 22.0;
    let rel_close_x = close_x - cx;
    let rel_close_y = close_y - cy;
    assert!(state.hit_test_close(rel_close_x, rel_close_y, cx, cy));

    // OK button is at top + 104.0, height 40.0, centered horizontally
    let ok_rel_x = 0.0;
    let ok_rel_y = (top + 120.0) - cy;
    assert!(state.hit_test_ok(ok_rel_x, ok_rel_y, cx, cy));
}

#[test]
fn test_alert_card_render() {
    let mut state = AlertCardState::new("Cannot merge a locked PDF. Unlock first.", 1.0);
    for _ in 0..60 {
        state.tick(1.0 / 60.0);
    }
    assert!(state.pop_in.value() > 0.95);

    let mut pixmap = Pixmap::new(400, 400).unwrap();
    let theme = Theme::dark();
    let fonts = FontContext::new();

    render_alert_card(&state, &theme, &fonts, &mut pixmap);
    let non_transparent = pixmap.data().chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(non_transparent > 1000, "Pixmap should render alert card content");
}
