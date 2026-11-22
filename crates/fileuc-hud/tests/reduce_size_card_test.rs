use fileuc_hud::reduce_size_card::{ReduceSizeCardState, SizeUnit};

#[test]
fn test_kb_file_defaults_to_kb_and_disables_mb() {
    // 400 KB file (409,600 bytes)
    let card = ReduceSizeCardState::new(1, 400 * 1024, 1.0);
    assert_eq!(card.unit, SizeUnit::Kb);
    assert!(card.is_mb_disabled());
}

#[test]
fn test_mb_file_defaults_to_mb_and_allows_both() {
    // 5 MB file
    let card = ReduceSizeCardState::new(1, 5 * 1024 * 1024, 1.0);
    assert_eq!(card.unit, SizeUnit::Mb);
    assert!(!card.is_mb_disabled());
}

#[test]
fn test_cannot_enter_number_greater_than_current_size() {
    // 400 KB file
    let mut card = ReduceSizeCardState::new(1, 400 * 1024, 1.0);
    assert_eq!(card.unit, SizeUnit::Kb);

    // Type '3' (3 KB < 400 KB) -> allowed
    card.push_char('3');
    assert_eq!(card.input_text, "3");

    // Type '9' (39 KB < 400 KB) -> allowed
    card.push_char('9');
    assert_eq!(card.input_text, "39");

    // Type '9' (399 KB < 400 KB) -> allowed
    card.push_char('9');
    assert_eq!(card.input_text, "399");

    // Type '0' (3990 KB >= 400 KB) -> REJECTED!
    card.push_char('0');
    assert_eq!(card.input_text, "399");

    // Clear and test typing 400 directly
    card.clear();
    card.push_char('4'); // 4 KB -> allowed
    assert_eq!(card.input_text, "4");
    card.push_char('0'); // 40 KB -> allowed
    assert_eq!(card.input_text, "40");
    card.push_char('0'); // 400 KB >= 400 KB -> REJECTED!
    assert_eq!(card.input_text, "40");
}

#[test]
fn test_cannot_switch_to_mb_if_disabled() {
    let mut card = ReduceSizeCardState::new(1, 400 * 1024, 1.0);
    assert!(card.is_mb_disabled());

    card.set_unit(SizeUnit::Mb);
    // Should remain in KB
    assert_eq!(card.unit, SizeUnit::Kb);
}

#[test]
fn test_switching_to_mb_clears_invalid_input() {
    // 5 MB file (5,242,880 bytes)
    let mut card = ReduceSizeCardState::new(1, 5 * 1024 * 1024, 1.0);
    assert!(!card.is_mb_disabled());

    card.set_unit(SizeUnit::Kb);
    // Type 2000 KB (2000 KB < 5 MB) -> allowed
    card.push_char('2');
    card.push_char('0');
    card.push_char('0');
    card.push_char('0');
    assert_eq!(card.input_text, "2000");

    // Switch to MB: 2000 MB > 5 MB, so input must be cleared!
    card.set_unit(SizeUnit::Mb);
    assert_eq!(card.unit, SizeUnit::Mb);
    assert_eq!(card.input_text, "");
}

#[test]
fn test_valid_submission_checks() {
    let mut card = ReduceSizeCardState::new(1, 400 * 1024, 1.0);
    // Empty input is invalid
    assert!(!card.is_valid_submission());

    card.push_char('2');
    card.push_char('0');
    card.push_char('0');
    // 200 KB < 400 KB -> valid
    assert!(card.is_valid_submission());
    assert_eq!(card.target_bytes(), Some(200 * 1024));
}

#[test]
fn test_reduce_size_hit_testing() {
    let card = ReduceSizeCardState::new(1, 400 * 1024, 1.0);
    let cx = 200.0;
    let cy = 200.0;

    // Center hits card
    assert!(card.hit_test_card(0.0, 0.0, cx, cy));
    assert!(!card.hit_test_card(300.0, 300.0, cx, cy));

    // Submit button is at top + 168.0..208.0
    // top = 200 - 115 = 85.0
    // btn_top = 85 + 168 = 253.0
    // rel_y = 60.0 -> py = 260.0 (inside 253..293)
    assert!(card.hit_test_submit(0.0, 60.0, cx, cy));
}
