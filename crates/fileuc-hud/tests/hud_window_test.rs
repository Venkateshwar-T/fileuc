use fileuc_hud::hud_window::HudWindow;
use std::sync::mpsc;

#[test]
fn test_hud_window_move_to_updates_coordinates() {
    let (tx, _rx) = mpsc::channel();
    // HudWindow requires Win32 GUI environment; initialize and test move_to
    if let Ok(mut hud) = HudWindow::new(tx) {
        let (cx, cy) = hud.move_to(250, 350);
        assert_eq!(hud.screen_x(), 250);
        assert_eq!(hud.screen_y(), 350);
        assert_eq!(cx, 250 + (hud.size() as i32 / 2));
        assert_eq!(cy, 350 + (hud.size() as i32 / 2));

        let (cx2, cy2) = hud.move_to(-100, 50);
        assert_eq!(hud.screen_x(), -100);
        assert_eq!(hud.screen_y(), 50);
        assert_eq!(cx2, -100 + (hud.size() as i32 / 2));
        assert_eq!(cy2, 50 + (hud.size() as i32 / 2));
    }
}
