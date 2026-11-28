pub mod gesture;
pub mod mouse_hook;
pub mod shortcut;

pub use gesture::{GestureDetector, GestureEvent, Modifiers, MouseButton};
pub use mouse_hook::{
    install, set_key_capture, set_mouse_capture_rect, set_qr_shortcut, set_receive_shortcut,
    set_search_shortcut, HookError, HookEvent, HookGuard,
};
pub use shortcut::{parse_shortcut, ShortcutCombo};
