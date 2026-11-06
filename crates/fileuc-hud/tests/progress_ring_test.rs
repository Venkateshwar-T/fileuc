use fileuc_hud::progress_ring::{ProgressState, ProgressTracker};

#[test]
fn tracker_lifecycle() {
    let mut t = ProgressTracker::new();
    assert!(matches!(t.state(), ProgressState::Converting(f) if *f == 0.0));

    t.update(0.5);
    assert!(matches!(t.state(), ProgressState::Converting(f) if (*f - 0.5).abs() < 0.001));

    t.update(1.0);
    t.mark_success();
    assert!(matches!(t.state(), ProgressState::Success));
}

#[test]
fn failure_carries_reason() {
    let mut t = ProgressTracker::new();
    t.mark_failure("File locked".to_string());
    assert!(matches!(t.state(), ProgressState::Failure(s) if s == "File locked"));
}
