use crate::gesture::{Modifiers, MouseButton};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use windows::Win32::Foundation::{HMODULE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RCONTROL, VK_RMENU,
    VK_RSHIFT, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_RBUTTONDOWN, WM_RBUTTONUP,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    MouseDown {
        btn: MouseButton,
        x: i32,
        y: i32,
        mods: Modifiers,
    },
    MouseUp {
        btn: MouseButton,
        x: i32,
        y: i32,
        mods: Modifiers,
    },
    MouseMove {
        x: i32,
        y: i32,
        mods: Modifiers,
    },
}

#[derive(Debug)]
pub enum HookError {
    SetHookFailed(windows::core::Error),
}

struct SendHhook(HHOOK);
unsafe impl Send for SendHhook {}
unsafe impl Sync for SendHhook {}

static SENDER: Mutex<Option<Sender<HookEvent>>> = Mutex::new(None);
static HOOK_HANDLE: Mutex<Option<SendHhook>> = Mutex::new(None);

fn query_current_modifiers() -> Modifiers {
    unsafe {
        let mut bits = 0u8;
        let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) < 0
            || GetAsyncKeyState(VK_LCONTROL.0 as i32) < 0
            || GetAsyncKeyState(VK_RCONTROL.0 as i32) < 0;
        let alt = GetAsyncKeyState(VK_MENU.0 as i32) < 0
            || GetAsyncKeyState(VK_LMENU.0 as i32) < 0
            || GetAsyncKeyState(VK_RMENU.0 as i32) < 0;
        let shift = GetAsyncKeyState(VK_SHIFT.0 as i32) < 0
            || GetAsyncKeyState(VK_LSHIFT.0 as i32) < 0
            || GetAsyncKeyState(VK_RSHIFT.0 as i32) < 0;

        if ctrl {
            bits |= Modifiers::CTRL.0;
        }
        if alt {
            bits |= Modifiers::ALT.0;
        }
        if shift {
            bits |= Modifiers::SHIFT.0;
        }

        Modifiers(bits)
    }
}

unsafe extern "system" fn mouse_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let ms_struct = *(lparam.0 as *const MSLLHOOKSTRUCT);
        let x = ms_struct.pt.x;
        let y = ms_struct.pt.y;
        let mods = query_current_modifiers();

        let event = match msg {
            WM_MOUSEMOVE => Some(HookEvent::MouseMove { x, y, mods }),
            WM_LBUTTONDOWN => Some(HookEvent::MouseDown {
                btn: MouseButton::Left,
                x,
                y,
                mods,
            }),
            WM_LBUTTONUP => Some(HookEvent::MouseUp {
                btn: MouseButton::Left,
                x,
                y,
                mods,
            }),
            WM_RBUTTONDOWN => Some(HookEvent::MouseDown {
                btn: MouseButton::Right,
                x,
                y,
                mods,
            }),
            WM_RBUTTONUP => Some(HookEvent::MouseUp {
                btn: MouseButton::Right,
                x,
                y,
                mods,
            }),
            WM_MBUTTONDOWN => Some(HookEvent::MouseDown {
                btn: MouseButton::Middle,
                x,
                y,
                mods,
            }),
            WM_MBUTTONUP => Some(HookEvent::MouseUp {
                btn: MouseButton::Middle,
                x,
                y,
                mods,
            }),
            _ => None,
        };

        if let Some(ev) = event {
            if let Ok(guard) = SENDER.lock() {
                if let Some(ref tx) = *guard {
                    let _ = tx.send(ev);
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

pub struct HookGuard {
    _private: (),
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        if let Ok(mut handle_guard) = HOOK_HANDLE.lock() {
            if let Some(hook) = handle_guard.take() {
                unsafe {
                    let _ = UnhookWindowsHookEx(hook.0);
                }
            }
        }
        if let Ok(mut sender_guard) = SENDER.lock() {
            *sender_guard = None;
        }
    }
}

pub fn install(sender: Sender<HookEvent>) -> Result<HookGuard, HookError> {
    if let Ok(mut sender_guard) = SENDER.lock() {
        *sender_guard = Some(sender);
    }

    unsafe {
        let hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_hook_proc),
            HMODULE::default(),
            0,
        )
        .map_err(HookError::SetHookFailed)?;

        if let Ok(mut handle_guard) = HOOK_HANDLE.lock() {
            *handle_guard = Some(SendHhook(hook));
        }
    }

    Ok(HookGuard { _private: () })
}
