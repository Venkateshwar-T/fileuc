use crate::gesture::{Modifiers, MouseButton};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use windows::Win32::Foundation::{HMODULE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardState, ToUnicode, VK_BACK, VK_CAPITAL, VK_CONTROL,
    VK_ESCAPE, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RCONTROL, VK_RETURN, VK_RMENU,
    VK_RSHIFT, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
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
    KeyDown {
        mods: Modifiers,
    },
    KeyUp {
        mods: Modifiers,
    },
    KeyAction {
        vk: u32,
        ch: Option<char>,
        mods: Modifiers,
    },
    QuickQrTriggered,
    SearchShortcutTriggered,
    ReceiveShortcutTriggered,
}

#[derive(Debug)]
pub enum HookError {
    SetHookFailed(windows::core::Error),
}

struct SendHhook(HHOOK);
unsafe impl Send for SendHhook {}
unsafe impl Sync for SendHhook {}

static SENDER: Mutex<Option<Sender<HookEvent>>> = Mutex::new(None);
static MOUSE_HOOK_HANDLE: Mutex<Option<SendHhook>> = Mutex::new(None);
static KBD_HOOK_HANDLE: Mutex<Option<SendHhook>> = Mutex::new(None);
static KEY_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
static MOUSE_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
static MOUSE_CAPTURE_RECT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);
static QR_SHORTCUT_ACTIVE: AtomicBool = AtomicBool::new(false);
static QR_SHORTCUT_VK: AtomicU32 = AtomicU32::new(0);
static QR_SHORTCUT_MODS: AtomicU8 = AtomicU8::new(0);
static SEARCH_SHORTCUT_ACTIVE: AtomicBool = AtomicBool::new(false);
static SEARCH_SHORTCUT_VK: AtomicU32 = AtomicU32::new(0);
static SEARCH_SHORTCUT_MODS: AtomicU8 = AtomicU8::new(0);
static RECEIVE_SHORTCUT_ACTIVE: AtomicBool = AtomicBool::new(false);
static RECEIVE_SHORTCUT_VK: AtomicU32 = AtomicU32::new(0);
static RECEIVE_SHORTCUT_MODS: AtomicU8 = AtomicU8::new(0);
static TARGET_THREAD_ID: AtomicU32 = AtomicU32::new(0);

// Fast-path state for low-level hook callbacks: avoids 12 Win32 syscalls and channel locks per mouse move
static LBUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static MODIFIERS_STATE: AtomicU8 = AtomicU8::new(0);

pub fn set_key_capture(active: bool) {
    KEY_CAPTURE_ACTIVE.store(active, Ordering::SeqCst);
    if active {
        let m = query_current_modifiers();
        MODIFIERS_STATE.store(m.0, Ordering::SeqCst);
    }
}

pub fn set_target_thread_id(tid: u32) {
    TARGET_THREAD_ID.store(tid, Ordering::Release);
}

#[inline]
unsafe fn notify_target_thread() {
    let tid = TARGET_THREAD_ID.load(Ordering::Relaxed);
    if tid != 0 {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            tid,
            windows::Win32::UI::WindowsAndMessaging::WM_NULL,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

pub fn set_qr_shortcut(combo: Option<crate::shortcut::ShortcutCombo>) {
    if let Some(c) = combo {
        QR_SHORTCUT_VK.store(c.vk, Ordering::Release);
        QR_SHORTCUT_MODS.store(c.mods.0, Ordering::Release);
        QR_SHORTCUT_ACTIVE.store(true, Ordering::Release);
    } else {
        QR_SHORTCUT_ACTIVE.store(false, Ordering::Release);
    }
}

pub fn set_search_shortcut(combo: Option<crate::shortcut::ShortcutCombo>) {
    if let Some(c) = combo {
        SEARCH_SHORTCUT_VK.store(c.vk, Ordering::Release);
        SEARCH_SHORTCUT_MODS.store(c.mods.0, Ordering::Release);
        SEARCH_SHORTCUT_ACTIVE.store(true, Ordering::Release);
    } else {
        SEARCH_SHORTCUT_ACTIVE.store(false, Ordering::Release);
    }
}

pub fn set_receive_shortcut(combo: Option<crate::shortcut::ShortcutCombo>) {
    if let Some(c) = combo {
        RECEIVE_SHORTCUT_VK.store(c.vk, Ordering::Release);
        RECEIVE_SHORTCUT_MODS.store(c.mods.0, Ordering::Release);
        RECEIVE_SHORTCUT_ACTIVE.store(true, Ordering::Release);
    } else {
        RECEIVE_SHORTCUT_ACTIVE.store(false, Ordering::Release);
    }
}

pub fn set_mouse_capture_rect(rect: Option<(i32, i32, i32, i32)>) {
    let has_rect = rect.is_some();
    if let Ok(mut guard) = MOUSE_CAPTURE_RECT.lock() {
        *guard = rect;
    }
    MOUSE_CAPTURE_ACTIVE.store(has_rect, Ordering::SeqCst);
}

fn query_current_modifiers() -> Modifiers {
    unsafe {
        let mut bits = 0u8;
        let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_LCONTROL.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_RCONTROL.0 as i32) as u16 & 0x8000) != 0;
        let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_LMENU.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_RMENU.0 as i32) as u16 & 0x8000) != 0;
        let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_LSHIFT.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_RSHIFT.0 as i32) as u16 & 0x8000) != 0;

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

        if msg == WM_LBUTTONDOWN {
            LBUTTON_DOWN.store(true, Ordering::Relaxed);
        } else if msg == WM_LBUTTONUP {
            LBUTTON_DOWN.store(false, Ordering::Relaxed);
        }

        let is_lbutton = LBUTTON_DOWN.load(Ordering::Relaxed);
        let mods_byte = MODIFIERS_STATE.load(Ordering::Relaxed);
        let is_capturing = MOUSE_CAPTURE_ACTIVE.load(Ordering::Relaxed);

        // Fast-path bypass: If it's an idle mouse move with no modifiers, no LMB, and no modal card,
        // immediately forward to next hook. Eliminates 12 Win32 syscalls, mutex locks, and channel
        // pressure on thousands of mouse events per second!
        if msg == WM_MOUSEMOVE && !is_lbutton && mods_byte == 0 && !is_capturing {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        let ms_struct = *(lparam.0 as *const MSLLHOOKSTRUCT);
        let x = ms_struct.pt.x;
        let y = ms_struct.pt.y;
        let mods = if msg == WM_MOUSEMOVE {
            Modifiers(mods_byte)
        } else {
            let m = query_current_modifiers();
            MODIFIERS_STATE.store(m.0, Ordering::Relaxed);
            m
        };

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
                    notify_target_thread();
                }
            }
        }

        // Suppress mouse clicks inside the active modal card bounds so the window underneath never receives them
        if msg != WM_MOUSEMOVE && is_capturing {
            let is_inside_capture = if let Ok(guard) = MOUSE_CAPTURE_RECT.lock() {
                if let Some((l, t, r, b)) = *guard {
                    x >= l && x <= r && y >= t && y <= b
                } else {
                    false
                }
            } else {
                false
            };
            if is_inside_capture {
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let kb_struct = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb_struct.vkCode;
        let is_capturing = KEY_CAPTURE_ACTIVE.load(Ordering::SeqCst);

        let is_modifier = match vk {
            x if x == VK_CONTROL.0 as u32 || x == VK_LCONTROL.0 as u32 || x == VK_RCONTROL.0 as u32 => {
                Some(Modifiers::CTRL)
            }
            x if x == VK_MENU.0 as u32 || x == VK_LMENU.0 as u32 || x == VK_RMENU.0 as u32 => {
                Some(Modifiers::ALT)
            }
            x if x == VK_SHIFT.0 as u32 || x == VK_LSHIFT.0 as u32 || x == VK_RSHIFT.0 as u32 => {
                Some(Modifiers::SHIFT)
            }
            _ => None,
        };

        let is_shortcut_active = QR_SHORTCUT_ACTIVE.load(Ordering::Relaxed);
        let target_vk = QR_SHORTCUT_VK.load(Ordering::Relaxed);
        let target_mods = QR_SHORTCUT_MODS.load(Ordering::Relaxed);
        let current_mods = MODIFIERS_STATE.load(Ordering::Relaxed);
        let matches_shortcut = is_shortcut_active
            && (vk == target_vk)
            && (current_mods == target_mods);

        let is_search_active = SEARCH_SHORTCUT_ACTIVE.load(Ordering::Relaxed);
        let search_target_vk = SEARCH_SHORTCUT_VK.load(Ordering::Relaxed);
        let search_target_mods = SEARCH_SHORTCUT_MODS.load(Ordering::Relaxed);
        let matches_search_shortcut = is_search_active
            && (vk == search_target_vk)
            && (current_mods == search_target_mods);

        let is_receive_active = RECEIVE_SHORTCUT_ACTIVE.load(Ordering::Relaxed);
        let receive_target_vk = RECEIVE_SHORTCUT_VK.load(Ordering::Relaxed);
        let receive_target_mods = RECEIVE_SHORTCUT_MODS.load(Ordering::Relaxed);
        let matches_receive_shortcut = is_receive_active
            && (vk == receive_target_vk)
            && (current_mods == receive_target_mods);

        if let Some(mod_flag) = is_modifier {
            let mut mods = Modifiers(current_mods);
            if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                mods.0 &= !mod_flag.0;
                MODIFIERS_STATE.store(mods.0, Ordering::Relaxed);
                if let Ok(guard) = SENDER.lock() {
                    if let Some(ref tx) = *guard {
                        let _ = tx.send(HookEvent::KeyUp { mods });
                        notify_target_thread();
                    }
                }
            } else if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                mods.0 |= mod_flag.0;
                MODIFIERS_STATE.store(mods.0, Ordering::Relaxed);
                if let Ok(guard) = SENDER.lock() {
                    if let Some(ref tx) = *guard {
                        let _ = tx.send(HookEvent::KeyDown { mods });
                        notify_target_thread();
                    }
                }
            }
        } else if matches_shortcut && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) {
            // Trigger quick QR even if modal is open (enables instant toggle-dismiss)
            if let Ok(guard) = SENDER.lock() {
                if let Some(ref tx) = *guard {
                    let _ = tx.send(HookEvent::QuickQrTriggered);
                    notify_target_thread();
                }
            }
            return LRESULT(1);
        } else if matches_shortcut && (msg == WM_KEYUP || msg == WM_SYSKEYUP) {
            return LRESULT(1);
        } else if matches_search_shortcut && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) {
            // Trigger search HUD even if modal or capture is active (enables instant toggle-dismiss)
            if let Ok(guard) = SENDER.lock() {
                if let Some(ref tx) = *guard {
                    let _ = tx.send(HookEvent::SearchShortcutTriggered);
                    notify_target_thread();
                }
            }
            return LRESULT(1);
        } else if matches_search_shortcut && (msg == WM_KEYUP || msg == WM_SYSKEYUP) {
            return LRESULT(1);
        } else if matches_receive_shortcut && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) {
            if let Ok(guard) = SENDER.lock() {
                if let Some(ref tx) = *guard {
                    let _ = tx.send(HookEvent::ReceiveShortcutTriggered);
                    notify_target_thread();
                }
            }
            return LRESULT(1);
        } else if matches_receive_shortcut && (msg == WM_KEYUP || msg == WM_SYSKEYUP) {
            return LRESULT(1);
        } else if is_capturing && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) {
            let mods = query_current_modifiers();
            MODIFIERS_STATE.store(mods.0, Ordering::Relaxed);

            let mut ch = None;
            if vk != VK_BACK.0 as u32
                && vk != VK_RETURN.0 as u32
                && vk != VK_ESCAPE.0 as u32
                && vk != VK_TAB.0 as u32
            {
                let mut key_state = [0u8; 256];
                let _ = GetKeyboardState(&mut key_state);
                // Synchronize global Shift, CapsLock, Ctrl, and Alt states for accurate unicode translation:
                if mods.contains(Modifiers::SHIFT) {
                    key_state[VK_SHIFT.0 as usize] = 0x80;
                } else {
                    key_state[VK_SHIFT.0 as usize] = 0;
                }
                if (GetKeyState(VK_CAPITAL.0 as i32) & 0x0001) != 0 {
                    key_state[VK_CAPITAL.0 as usize] = 0x01;
                } else {
                    key_state[VK_CAPITAL.0 as usize] = 0;
                }
                if mods.contains(Modifiers::CTRL) {
                    key_state[VK_CONTROL.0 as usize] = 0x80;
                } else {
                    key_state[VK_CONTROL.0 as usize] = 0;
                }
                if mods.contains(Modifiers::ALT) {
                    key_state[VK_MENU.0 as usize] = 0x80;
                } else {
                    key_state[VK_MENU.0 as usize] = 0;
                }

                let mut buff = [0u16; 4];
                let count = ToUnicode(vk, kb_struct.scanCode, Some(&key_state), &mut buff, 0);
                if count == 1 {
                    if let Some(c) = char::from_u32(buff[0] as u32) {
                        if !c.is_control() {
                            ch = Some(c);
                        }
                    }
                }
            }
            if let Ok(guard) = SENDER.lock() {
                if let Some(ref tx) = *guard {
                    let _ = tx.send(HookEvent::KeyAction { vk, ch, mods });
                    notify_target_thread();
                }
            }
            return LRESULT(1);
        } else if is_capturing && (msg == WM_KEYUP || msg == WM_SYSKEYUP) {
            return LRESULT(1);
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

pub struct HookGuard {
    _private: (),
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        if let Ok(mut handle_guard) = MOUSE_HOOK_HANDLE.lock() {
            if let Some(hook) = handle_guard.take() {
                unsafe {
                    let _ = UnhookWindowsHookEx(hook.0);
                }
            }
        }
        if let Ok(mut kbd_guard) = KBD_HOOK_HANDLE.lock() {
            if let Some(hook) = kbd_guard.take() {
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

use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::StationsAndDesktops::{OpenDesktopW, SetThreadDesktop};

pub fn install(sender: Sender<HookEvent>) -> Result<HookGuard, HookError> {
    if let Ok(mut sender_guard) = SENDER.lock() {
        *sender_guard = Some(sender);
    }

    // Pre-initialize initial modifier and mouse button states
    let initial_mods = query_current_modifiers();
    MODIFIERS_STATE.store(initial_mods.0, Ordering::Relaxed);
    let lbutton = unsafe {
        (windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(
            windows::Win32::UI::Input::KeyboardAndMouse::VK_LBUTTON.0 as i32,
        ) as u16
            & 0x8000)
            != 0
    };
    LBUTTON_DOWN.store(lbutton, Ordering::Relaxed);

    unsafe {
        if let Ok(hdesk) = OpenDesktopW(
            windows::core::w!("Default"),
            windows::Win32::System::StationsAndDesktops::DESKTOP_CONTROL_FLAGS(0),
            false,
            0x10000000,
        ) {
            let _ = SetThreadDesktop(hdesk);
            let _ = windows::Win32::System::StationsAndDesktops::CloseDesktop(hdesk);
        }

        let hinstance: HMODULE = GetModuleHandleW(None).unwrap_or_default();

        let mouse_hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_hook_proc),
            hinstance,
            0,
        )
        .map_err(HookError::SetHookFailed)?;

        let kbd_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            hinstance,
            0,
        )
        .map_err(HookError::SetHookFailed)?;

        if let Ok(mut handle_guard) = MOUSE_HOOK_HANDLE.lock() {
            *handle_guard = Some(SendHhook(mouse_hook));
        }
        if let Ok(mut kbd_guard) = KBD_HOOK_HANDLE.lock() {
            *kbd_guard = Some(SendHhook(kbd_hook));
        }
    }

    Ok(HookGuard { _private: () })
}
