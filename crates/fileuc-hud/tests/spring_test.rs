use fileuc_hud::spring::Spring;

#[test]
fn spring_converges_to_target() {
    let mut s = Spring::new(0.0, 300.0, 25.0);
    s.set_target(1.0);
    for _ in 0..200 {
        s.tick(1.0 / 60.0);
    }
    assert!((s.value() - 1.0).abs() < 0.001, "spring should settle at target");
}

#[test]
fn spring_reports_not_animating_when_settled() {
    let mut s = Spring::new(1.0, 300.0, 25.0);
    s.set_target(1.0);
    let animating = s.tick(1.0 / 60.0);
    assert!(!animating, "spring at rest should not be animating");
}
