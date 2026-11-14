#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const CTRL: Modifiers = Modifiers(1 << 0);
    pub const ALT: Modifiers = Modifiers(1 << 1);
    pub const SHIFT: Modifiers = Modifiers(1 << 2);

    pub fn contains(&self, other: Modifiers) -> bool {
        (self.0 & other.0) == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureEvent {
    DragStarted { cursor_x: i32, cursor_y: i32 },
    DragEnded,
}

pub struct GestureDetector {
    threshold_px: i32,
    required_modifier: Modifiers,
    pressed: bool,
    drag_active: bool,
    origin_x: i32,
    origin_y: i32,
    current_x: i32,
    current_y: i32,
    active_button: Option<MouseButton>,
}

impl GestureDetector {
    pub fn new(threshold_px: i32, required_modifier: Modifiers) -> Self {
        Self {
            threshold_px,
            required_modifier,
            pressed: false,
            drag_active: false,
            origin_x: 0,
            origin_y: 0,
            current_x: 0,
            current_y: 0,
            active_button: None,
        }
    }

    pub fn set_required_modifier(&mut self, m: Modifiers) {
        self.required_modifier = m;
    }

    pub fn mouse_down(
        &mut self,
        button: MouseButton,
        x: i32,
        y: i32,
        _mods: Modifiers,
    ) -> Option<GestureEvent> {
        self.current_x = x;
        self.current_y = y;
        if button == MouseButton::Left {
            self.pressed = true;
            self.origin_x = x;
            self.origin_y = y;
            self.active_button = Some(button);
            self.drag_active = false;
        }
        None
    }

    pub fn mouse_move(&mut self, x: i32, y: i32, mods: Modifiers) -> Option<GestureEvent> {
        self.current_x = x;
        self.current_y = y;

        if !self.pressed {
            let lbutton_down = unsafe {
                (windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(
                    windows::Win32::UI::Input::KeyboardAndMouse::VK_LBUTTON.0 as i32,
                ) as u16
                    & 0x8000)
                    != 0
            };
            if lbutton_down {
                self.pressed = true;
                self.origin_x = x;
                self.origin_y = y;
                self.active_button = Some(MouseButton::Left);
            } else {
                return None;
            }
        }

        if self.drag_active {
            // If the required modifier was released while dragging, cancel the gesture.
            if !mods.contains(self.required_modifier) {
                self.drag_active = false;
                self.pressed = false;
                self.active_button = None;
                return Some(GestureEvent::DragEnded);
            }
            None
        } else {
            // Check if user is holding required modifier and moved past threshold.
            // Supports both starting with Ctrl held AND pressing Ctrl late during drag.
            if mods.contains(self.required_modifier) {
                let dx = (x - self.origin_x) as i64;
                let dy = (y - self.origin_y) as i64;
                let dist_sq = dx * dx + dy * dy;
                let thresh_sq = (self.threshold_px as i64) * (self.threshold_px as i64);

                if dist_sq >= thresh_sq {
                    self.drag_active = true;
                    Some(GestureEvent::DragStarted { cursor_x: x, cursor_y: y })
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    pub fn mouse_up(
        &mut self,
        button: MouseButton,
        x: i32,
        y: i32,
        _mods: Modifiers,
    ) -> Option<GestureEvent> {
        self.current_x = x;
        self.current_y = y;

        let was_dragging = self.drag_active && self.active_button == Some(button);
        if self.active_button == Some(button) || button == MouseButton::Left {
            self.pressed = false;
            self.drag_active = false;
            self.active_button = None;
        }

        if was_dragging {
            Some(GestureEvent::DragEnded)
        } else {
            None
        }
    }

    pub fn key_down(&mut self, mods: Modifiers) -> Option<GestureEvent> {
        // Late modifier press while already pressed and past threshold
        if self.pressed && !self.drag_active && mods.contains(self.required_modifier) {
            let dx = (self.current_x - self.origin_x) as i64;
            let dy = (self.current_y - self.origin_y) as i64;
            let dist_sq = dx * dx + dy * dy;
            let thresh_sq = (self.threshold_px as i64) * (self.threshold_px as i64);

            if dist_sq >= thresh_sq {
                self.drag_active = true;
                return Some(GestureEvent::DragStarted {
                    cursor_x: self.current_x,
                    cursor_y: self.current_y,
                });
            }
        }
        None
    }

    pub fn key_up(&mut self, mods: Modifiers) -> Option<GestureEvent> {
        if self.drag_active && !mods.contains(self.required_modifier) {
            self.drag_active = false;
            self.pressed = false;
            self.active_button = None;
            Some(GestureEvent::DragEnded)
        } else {
            None
        }
    }
}
