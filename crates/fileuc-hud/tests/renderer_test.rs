use fileuc_hud::renderer::{render_donut, DonutState};
use fileuc_hud::theme::Theme;
use fileuc_types::ConversionAction;

#[test]
fn render_produces_non_transparent_pixels_for_donut() {
    let mut state = DonutState::new(vec![ConversionAction::ToPng, ConversionAction::ToJpg], 2);
    // Settle the pop-in animation
    for _ in 0..60 {
        state.tick(1.0 / 60.0);
    }
    let theme = Theme::dark();
    let mut pixmap = tiny_skia::Pixmap::new(300, 300).unwrap();
    render_donut(&state, &theme, &mut pixmap);

    // At least some pixels in the ring area should be non-transparent
    let center = 150usize;
    let ring_r = 80usize; // roughly in the ring
    let pixel = pixmap.pixel(center as u32, (center - ring_r) as u32).unwrap();
    assert!(pixel.alpha() > 0, "ring area should have opaque pixels");
}
