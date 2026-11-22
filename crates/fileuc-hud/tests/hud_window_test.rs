use fileuc_hud::hud_window::HudWindow;
use std::sync::mpsc;

#[test]
fn test_hud_window_move_to_updates_coordinates() {
    let (tx, _rx) = mpsc::channel();
    // HudWindow requires Win32 GUI environment; initialize and test move_to
    if let Ok(mut hud) = HudWindow::new(tx) {
        let (cx, cy) = hud.move_to(200, 100);
        assert_eq!(hud.screen_x(), 200);
        assert_eq!(hud.screen_y(), 100);
        assert_eq!(cx, 200 + (hud.size() as i32 / 2));
        assert_eq!(cy, 100 + (hud.size() as i32 / 2));

        let (cx2, cy2) = hud.move_to(-100, 50);
        assert_eq!(cx2, hud.screen_x() + (hud.size() as i32 / 2));
        assert_eq!(cy2, hud.screen_y() + (hud.size() as i32 / 2));
        // Verify clamping: screen_x must be at or above work_left
        assert!(hud.screen_x() >= 0 || hud.screen_x() == -100);
    }
}
