use fileuc_hud::donut_geometry::{DonutLayout, HitResult};

#[test]
fn hit_test_center_and_outside() {
    let layout = DonutLayout::new(45.0, 120.0, 6);
    assert_eq!(layout.hit_test(0.0, 0.0), HitResult::CenterHole);
    assert_eq!(layout.hit_test(0.0, -20.0), HitResult::CenterHole); // inside inner radius
    assert_eq!(layout.hit_test(0.0, -200.0), HitResult::Outside);
}

#[test]
fn hit_test_identifies_correct_slice() {
    let layout = DonutLayout::new(45.0, 120.0, 6);
    // Slice 0 centered at 12 o'clock → point straight up at radius 80
    assert_eq!(layout.hit_test(0.0, -80.0), HitResult::Slice(0));
    // Slice 3 centered at 6 o'clock → point straight down at radius 80
    assert_eq!(layout.hit_test(0.0, 80.0), HitResult::Slice(3));
    // Slice 1 at ~60° clockwise → point to the right and slightly up
    let angle = std::f32::consts::TAU / 6.0; // 60°
    let x = 80.0 * angle.sin();
    let y = -80.0 * angle.cos();
    assert_eq!(layout.hit_test(x, y), HitResult::Slice(1));
}

#[test]
fn drop_in_center_is_not_a_slice() {
    // Review Focus #3: dropping in center must not trigger conversion
    let layout = DonutLayout::new(45.0, 120.0, 4);
    assert!(matches!(layout.hit_test(0.0, 0.0), HitResult::CenterHole));
    assert!(matches!(layout.hit_test(10.0, 10.0), HitResult::CenterHole));
}
