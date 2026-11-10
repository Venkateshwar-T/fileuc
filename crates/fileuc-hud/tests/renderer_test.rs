use fileuc_hud::font::FontContext;
use fileuc_hud::renderer::{render_donut, DonutState};
use fileuc_hud::theme::Theme;
use fileuc_types::ConversionAction;

#[test]
fn render_produces_non_transparent_pixels_for_donut() {
    let mut state = DonutState::new(
        vec![ConversionAction::ToPng, ConversionAction::ToJpg],
        2,
        1.0,
    );
    // Settle the pop-in animation
    for _ in 0..60 {
        state.tick(1.0 / 60.0);
    }
    let theme = Theme::dark();
    let fonts = FontContext::new();
    let mut pixmap = tiny_skia::Pixmap::new(342, 342).unwrap();
    render_donut(&state, &theme, &fonts, &mut pixmap);

    // At least some pixels in the ring area should be non-transparent
    let center = 171usize;
    let ring_r = 100usize; // in the 63..150 ring
    let pixel = pixmap.pixel(center as u32, (center - ring_r) as u32).unwrap();
    assert!(pixel.alpha() > 0, "ring area should have opaque pixels");
}

#[test]
fn test_hovered_slice_updates_center_prompt() {
    let mut state = DonutState::new(
        vec![ConversionAction::Share, ConversionAction::CompressPdf, ConversionAction::LockPdf],
        1,
        1.0,
    );
    for _ in 0..60 {
        state.tick(1.0 / 60.0);
    }
    state.set_hovered(Some(0)); // Share slice
    let theme = Theme::dark();
    let fonts = FontContext::new();
    let mut pixmap = tiny_skia::Pixmap::new(342, 342).unwrap();
    render_donut(&state, &theme, &fonts, &mut pixmap);

    // Center area should have drawn the text pixels
    let center = 171usize;
    let pixel = pixmap.pixel(center as u32, (center + 11) as u32).unwrap();
    assert!(pixel.alpha() > 0, "center subtitle text area should have non-transparent pixels");
}
