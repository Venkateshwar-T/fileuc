use fileuc_hook::gesture::*;

#[test]
fn ctrl_drag_past_threshold_fires_drag_started() {
    let mut det = GestureDetector::new(4, Modifiers::CTRL);

    // Press without Ctrl -> no event on move
    assert_eq!(det.mouse_down(MouseButton::Left, 100, 100, Modifiers::NONE), None);
    assert_eq!(det.mouse_move(110, 110, Modifiers::NONE), None);
    assert_eq!(det.mouse_up(MouseButton::Left, 110, 110, Modifiers::NONE), None);

    // Press WITH Ctrl -> sub-threshold move -> no event
    assert_eq!(det.mouse_down(MouseButton::Left, 100, 100, Modifiers::CTRL), None);
    assert_eq!(det.mouse_move(102, 102, Modifiers::CTRL), None);

    // Past threshold -> DragStarted at current cursor
    assert_eq!(
        det.mouse_move(106, 106, Modifiers::CTRL),
        Some(GestureEvent::DragStarted { cursor_x: 106, cursor_y: 106 })
    );

    // Further moves don't re-fire
    assert_eq!(det.mouse_move(120, 120, Modifiers::CTRL), None);

    // Release -> DragEnded
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
    // Now release -> nothing
    assert_eq!(det.mouse_up(MouseButton::Left, 110, 110, Modifiers::NONE), None);
}

#[test]
fn late_modifier_past_threshold_fires_drag_started() {
    let mut det = GestureDetector::new(4, Modifiers::CTRL);
    // User clicks left mouse button without Ctrl
    assert_eq!(det.mouse_down(MouseButton::Left, 100, 100, Modifiers::NONE), None);
    // User drags a bit without Ctrl
    assert_eq!(det.mouse_move(102, 102, Modifiers::NONE), None);
    // User presses Ctrl while dragging past threshold!
    assert_eq!(
        det.mouse_move(110, 110, Modifiers::CTRL),
        Some(GestureEvent::DragStarted { cursor_x: 110, cursor_y: 110 })
    );
    // Releasing mouse ends the drag
    assert_eq!(
        det.mouse_up(MouseButton::Left, 110, 110, Modifiers::CTRL),
        Some(GestureEvent::DragEnded)
    );
}

#[test]
fn releasing_modifier_while_dragging_ends_drag() {
    let mut det = GestureDetector::new(4, Modifiers::CTRL);
    det.mouse_down(MouseButton::Left, 100, 100, Modifiers::CTRL);
    assert_eq!(
        det.mouse_move(110, 110, Modifiers::CTRL),
        Some(GestureEvent::DragStarted { cursor_x: 110, cursor_y: 110 })
    );
    // Releasing Ctrl during move ends drag
    assert_eq!(
        det.mouse_move(115, 115, Modifiers::NONE),
        Some(GestureEvent::DragEnded)
    );
}

#[test]
fn key_up_while_dragging_ends_drag() {
    let mut det = GestureDetector::new(4, Modifiers::CTRL);
    det.mouse_down(MouseButton::Left, 100, 100, Modifiers::CTRL);
    assert_eq!(
        det.mouse_move(110, 110, Modifiers::CTRL),
        Some(GestureEvent::DragStarted { cursor_x: 110, cursor_y: 110 })
    );
    // KeyUp event for Ctrl ends drag immediately even without mouse move
    assert_eq!(
        det.key_up(Modifiers::NONE),
        Some(GestureEvent::DragEnded)
    );
}
