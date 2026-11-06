use fileuc_hook::gesture::*;

#[test]
fn ctrl_drag_past_threshold_fires_drag_started() {
    let mut det = GestureDetector::new(4, Modifiers::CTRL);

    // Press without Ctrl → no event on move
    assert_eq!(det.mouse_down(MouseButton::Left, 100, 100, Modifiers::NONE), None);
    assert_eq!(det.mouse_move(110, 110, Modifiers::NONE), None);
    assert_eq!(det.mouse_up(MouseButton::Left, 110, 110, Modifiers::NONE), None);

    // Press WITH Ctrl → sub-threshold move → no event
    assert_eq!(det.mouse_down(MouseButton::Left, 100, 100, Modifiers::CTRL), None);
    assert_eq!(det.mouse_move(102, 102, Modifiers::CTRL), None);

    // Past threshold → DragStarted at the original press point
    assert_eq!(
        det.mouse_move(106, 106, Modifiers::CTRL),
        Some(GestureEvent::DragStarted { origin_x: 100, origin_y: 100 })
    );

    // Further moves don't re-fire
    assert_eq!(det.mouse_move(120, 120, Modifiers::CTRL), None);

    // Release → DragEnded
    assert_eq!(
        det.mouse_up(MouseButton::Left, 120, 120, Modifiers::CTRL),
        Some(GestureEvent::DragEnded)
    );
}

#[test]
fn releasing_modifier_before_threshold_cancels() {
    let mut det = GestureDetector::new(4, Modifiers::CTRL);
    det.mouse_down(MouseButton::Left, 100, 100, Modifiers::CTRL);
    // Ctrl released mid-drag (modifiers changed)
    assert_eq!(det.mouse_move(110, 110, Modifiers::NONE), None);
    // Now past threshold but Ctrl is gone → nothing
    assert_eq!(det.mouse_up(MouseButton::Left, 110, 110, Modifiers::NONE), None);
}
