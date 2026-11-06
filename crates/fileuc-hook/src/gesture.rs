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
    DragStarted { origin_x: i32, origin_y: i32 },
    DragEnded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Armed { origin_x: i32, origin_y: i32, button: MouseButton },
    Dragging { origin_x: i32, origin_y: i32, button: MouseButton },
}

pub struct GestureDetector {
    threshold_px: i32,
    required_modifier: Modifiers,
    state: State,
}

impl GestureDetector {
    pub fn new(threshold_px: i32, required_modifier: Modifiers) -> Self {
        Self {
            threshold_px,
            required_modifier,
            state: State::Idle,
        }
    }

    pub fn mouse_down(
        &mut self,
        button: MouseButton,
        x: i32,
        y: i32,
        mods: Modifiers,
    ) -> Option<GestureEvent> {
        if mods.contains(self.required_modifier) && button == MouseButton::Left {
            self.state = State::Armed {
                origin_x: x,
                origin_y: y,
                button,
            };
        } else {
            self.state = State::Idle;
        }
        None
    }

    pub fn mouse_move(&mut self, x: i32, y: i32, mods: Modifiers) -> Option<GestureEvent> {
        match self.state {
            State::Idle => None,
            State::Armed { origin_x, origin_y, button } => {
                if !mods.contains(self.required_modifier) {
                    self.state = State::Idle;
                    return None;
                }
                let dx = (x - origin_x) as i64;
                let dy = (y - origin_y) as i64;
                let dist_sq = dx * dx + dy * dy;
                let thresh_sq = (self.threshold_px as i64) * (self.threshold_px as i64);

                if dist_sq >= thresh_sq {
                    self.state = State::Dragging { origin_x, origin_y, button };
                    Some(GestureEvent::DragStarted { origin_x, origin_y })
                } else {
                    None
                }
            }
            State::Dragging { .. } => {
                if !mods.contains(self.required_modifier) {
                    self.state = State::Idle;
                    Some(GestureEvent::DragEnded)
                } else {
                    None
                }
            }
        }
    }

    pub fn mouse_up(
        &mut self,
        button: MouseButton,
        _x: i32,
        _y: i32,
        _mods: Modifiers,
    ) -> Option<GestureEvent> {
        match self.state {
            State::Dragging { button: active_btn, .. } if active_btn == button => {
                self.state = State::Idle;
                Some(GestureEvent::DragEnded)
            }
            _ => {
                self.state = State::Idle;
                None
            }
        }
    }
}
