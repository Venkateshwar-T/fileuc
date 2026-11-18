use fileuc_hud::progress_ring::{ProgressState, ProgressTracker};

#[test]
fn tracker_lifecycle() {
    let mut t = ProgressTracker::new();
    assert!(matches!(t.state(), ProgressState::Converting(f) if *f >= 0.0));

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

#[test]
fn test_real_progress_does_not_get_stuck_at_88() {
    let mut t = ProgressTracker::new();
    assert_eq!(t.shown_progress(), 0.0);
    assert_eq!(*t.state(), ProgressState::Converting(0.0));

    t.update(0.2);
    t.tick(2.0); // 2 seconds pass
    assert!(matches!(t.state(), ProgressState::Converting(f) if (*f - 0.2).abs() < 0.001));

    t.update(0.5);
    t.tick(2.0); // 2 more seconds pass
    assert!(matches!(t.state(), ProgressState::Converting(f) if (*f - 0.5).abs() < 0.001));
}

#[test]
fn test_no_artificial_progress_creep() {
    let mut t = ProgressTracker::new();
    t.update(0.10);
    // Simulate 60 seconds passing at 60fps
    for _ in 0..3600 {
        t.tick(1.0 / 60.0);
    }
    // shown_progress must strictly track target_progress and NEVER creep ahead to 0.95
    assert!(
        t.shown_progress() <= 0.1001,
        "shown_progress was {} but should never exceed target_progress 0.10",
        t.shown_progress()
    );
}

#[test]
fn test_startup_crawl_capped_at_05() {
    let mut t = ProgressTracker::new();
    // No target progress reported yet (0.0)
    for _ in 0..3600 {
        t.tick(1.0 / 60.0);
    }
    // Startup crawl must be capped at 0.05 max
    assert!(
        t.shown_progress() <= 0.0501,
        "startup crawl without progress was {} but must be capped at 0.05",
        t.shown_progress()
    );
}
