use fileuc_hud::surface::BlitSurface;

#[test]
fn test_surface_swizzle_rgba_to_bgra() {
    let mut pixmap = tiny_skia::Pixmap::new(2, 2).unwrap();
    for pixel in pixmap.pixels_mut() {
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(200, 100, 50, 255).unwrap();
    }
    let mut surface = BlitSurface::new(2, 2);
    surface.update_from_pixmap(&pixmap);
    let bgra = surface.as_bgra_bytes();
    // First pixel: B=50, G=100, R=200, A=255
    assert_eq!(bgra[0], 50);  // B
    assert_eq!(bgra[1], 100); // G
    assert_eq!(bgra[2], 200); // R
    assert_eq!(bgra[3], 255); // A
}
