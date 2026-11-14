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

#[test]
fn test_sub_slice_no_dead_gap_between_children() {
    let layout = DonutLayout::new(63.0, 150.0, 6);
    let pop_out = 7.0;
    let group_idx = 0; // 12 o'clock
    let child_count = 2; // DOCX and EXCEL

    let (inner, outer) = layout.sub_radii(pop_out);
    let mid_r = (inner + outer) / 2.0;

    // Point directly on the seam between child 0 and child 1 at 12 o'clock
    let res_seam = layout.hit_test_sub(0.0, -mid_r, group_idx, child_count, pop_out);
    assert!(res_seam.is_some(), "Seam between children must resolve to a valid sub-slice");

    // Point in the radial corridor between parent slice outer radius and sub-slice inner radius
    let corridor_r = layout.outer_radius() + pop_out + 2.0;
    let res_corridor = layout.hit_test_sub(0.0, -corridor_r, group_idx, child_count, pop_out);
    assert!(res_corridor.is_some(), "Radial corridor must bridge without empty hole");
}
