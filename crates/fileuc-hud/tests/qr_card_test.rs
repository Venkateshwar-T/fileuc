use fileuc_hud::font::FontContext;
use fileuc_hud::qr_card::{render_qr_card, QrCardState, QrShareStatus};
use fileuc_hud::theme::Theme;
use tiny_skia::Pixmap;

#[test]
fn test_qr_card_state_creation_and_matrix() {
    let url = "http://192.168.1.100:9876/s/test_token_1234";
    let state = QrCardState::new(url, "my_document.pdf", 2_500_000, 1.0)
        .expect("Failed to create QrCardState");

    assert_eq!(state.file_name, "my_document.pdf");
    assert_eq!(state.file_size_formatted, "2.4 MB");
    assert_eq!(state.status, QrShareStatus::WaitingForScan);
    assert!(!state.qr_matrix.is_empty());

    let size = state.qr_matrix.len();
    assert!(size >= 21, "QR version 1 is at least 21x21 modules");
    assert_eq!(state.qr_matrix[0].len(), size);

    // Verify dark module exists
    let dark_count = state.qr_matrix.iter().flatten().filter(|&&m| m).count();
    assert!(dark_count > 50, "QR code must have dark modules");
}

#[test]
fn test_qr_finder_patterns() {
    let url = "http://192.168.29.109:54321/s/0123456789abcdef";
    let state = QrCardState::new(url, "photo.jpg", 1_000_000, 1.0).unwrap();
    let n = state.qr_matrix.len();
    assert!(n >= 21);

    // Top-left finder: 7x7 outer border dark, 3x3 center dark, 5x5 ring light
    for r in 0..7 {
        for c in 0..7 {
            let dark = r == 0 || r == 6 || c == 0 || c == 6 || (r >= 2 && r <= 4 && c >= 2 && c <= 4);
            assert_eq!(state.qr_matrix[r][c], dark, "Top-left finder mismatch at ({}, {})", r, c);
        }
    }

    // Top-right finder
    for r in 0..7 {
        for c in 0..7 {
            let dark = r == 0 || r == 6 || c == 0 || c == 6 || (r >= 2 && r <= 4 && c >= 2 && c <= 4);
            assert_eq!(state.qr_matrix[r][n - 7 + c], dark, "Top-right finder mismatch at ({}, {})", r, c);
        }
    }

    // Bottom-left finder
    for r in 0..7 {
        for c in 0..7 {
            let dark = r == 0 || r == 6 || c == 0 || c == 6 || (r >= 2 && r <= 4 && c >= 2 && c <= 4);
            assert_eq!(state.qr_matrix[n - 7 + r][c], dark, "Bottom-left finder mismatch at ({}, {})", r, c);
        }
    }
}

#[test]
fn test_qr_card_status_transitions() {
    let mut state = QrCardState::new("http://192.168.1.5:8080/s/tok", "image.png", 500, 1.0)
        .unwrap();

    assert_eq!(state.status, QrShareStatus::WaitingForScan);
    assert!(!state.is_finished());

    state.set_sending();
    assert_eq!(state.status, QrShareStatus::Sending);
    assert!(!state.is_finished());

    state.set_success();
    assert_eq!(state.status, QrShareStatus::Success);
    assert!(!state.is_finished()); // Should not finish immediately, allows 1.5s display
}

#[test]
fn test_qr_card_hit_testing() {
    let state = QrCardState::new("http://192.168.1.5:8080/s/tok", "file.txt", 1024, 1.0).unwrap();
    let cx = 171.0;
    let cy = 171.0;

    // Center is inside card
    assert!(state.hit_test_card(0.0, 0.0, cx, cy));

    // Far away is outside card
    assert!(!state.hit_test_card(200.0, 200.0, cx, cy));

    // Close button hit testing
    let (left, top, w, _) = state.card_bounds(cx, cy);
    let close_rel_x = (left + w - 22.0) - cx;
    let close_rel_y = (top + 22.0) - cy;
    assert!(state.hit_test_close(close_rel_x, close_rel_y, cx, cy));
    assert!(!state.hit_test_close(0.0, 0.0, cx, cy));
}

#[test]
fn test_render_qr_card_to_pixmap() {
    let mut state = QrCardState::new(
        "http://192.168.1.100:9876/s/test_token_1234",
        "LargeSpreadsheet.xlsx",
        15_000_000,
        1.0,
    )
    .unwrap();

    // Advance spring to full pop-in
    for _ in 0..60 {
        state.tick(1.0 / 60.0);
    }
    assert!(state.pop_in.value() > 0.95);

    let theme = Theme::resolve("dark");
    let fonts = FontContext::new();
    let mut pixmap = Pixmap::new(342, 342).unwrap();

    render_qr_card(&state, &theme, &fonts, &mut pixmap);

    // Verify pixmap has non-transparent pixels rendered
    let non_transparent = pixmap.data().chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(non_transparent > 1000, "Card must be rendered with non-transparent pixels");
}
