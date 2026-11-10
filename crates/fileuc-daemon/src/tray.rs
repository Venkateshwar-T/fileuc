use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICONDATAW_0,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyMenu, DestroyWindow, GetCursorPos, LoadIconW, PostMessageW, RegisterClassW,
    SetForegroundWindow, SetMenuDefaultItem, TrackPopupMenu, IDI_APPLICATION, MF_SEPARATOR,
    MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, WINDOW_EX_STYLE, WM_CONTEXTMENU,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NULL, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_USER, WNDCLASSW, WS_OVERLAPPED,
};

pub const WM_TRAY_CALLBACK: u32 = WM_USER + 101;
pub const TRAY_CMD_SETTINGS: u32 = 1;
pub const TRAY_CMD_TOGGLE_SERVICE: u32 = 2;
pub const TRAY_CMD_EXIT: u32 = 3;

pub const NIN_SELECT: u32 = WM_USER + 0;
pub const NIN_KEYSELECT: u32 = WM_USER + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    OpenSettings,
    ToggleService,
    Exit,
}

static TRAY_EVENT_TX: Mutex<Option<Sender<TrayEvent>>> = Mutex::new(None);
static LAST_OPEN: Mutex<Option<Instant>> = Mutex::new(None);
static SERVICE_ENABLED: AtomicBool = AtomicBool::new(true);

const TRAY_WINDOW_CLASS: PCWSTR = w!("FileUC_TrayWindow");

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TRAY_CALLBACK {
        let event = (lparam.0 & 0xFFFF) as u32;

        // Handle left clicks, double clicks, and keyboard activation
        if event == WM_LBUTTONDOWN
            || event == WM_LBUTTONUP
            || event == WM_LBUTTONDBLCLK
            || event == NIN_SELECT
            || event == NIN_KEYSELECT
        {
            let now = Instant::now();
            let should_fire = if let Ok(mut guard) = LAST_OPEN.lock() {
                if let Some(last) = *guard {
                    if now.duration_since(last) < Duration::from_millis(350) {
                        false
                    } else {
                        *guard = Some(now);
                        true
                    }
                } else {
                    *guard = Some(now);
                    true
                }
            } else {
                true
            };

            if should_fire {
                let _ = AllowSetForegroundWindow(0xFFFFFFFF);
                if let Ok(guard) = TRAY_EVENT_TX.lock() {
                    if let Some(ref tx) = *guard {
                        let _ = tx.send(TrayEvent::OpenSettings);
                    }
                }
            }
            return LRESULT(0);
        } else if event == WM_RBUTTONUP || event == WM_RBUTTONDOWN || event == WM_CONTEXTMENU {
            if let Some(cmd) = show_tray_menu(hwnd) {
                match cmd {
                    TRAY_CMD_SETTINGS => {
                        let _ = AllowSetForegroundWindow(0xFFFFFFFF);
                        if let Ok(guard) = TRAY_EVENT_TX.lock() {
                            if let Some(ref tx) = *guard {
                                let _ = tx.send(TrayEvent::OpenSettings);
                            }
                        }
                    }
                    TRAY_CMD_TOGGLE_SERVICE => {
                        if let Ok(guard) = TRAY_EVENT_TX.lock() {
                            if let Some(ref tx) = *guard {
                                let _ = tx.send(TrayEvent::ToggleService);
                            }
                        }
                    }
                    TRAY_CMD_EXIT => {
                        if let Ok(guard) = TRAY_EVENT_TX.lock() {
                            if let Some(ref tx) = *guard {
                                let _ = tx.send(TrayEvent::Exit);
                            }
                        }
                    }
                    _ => {}
                }
            }
            return LRESULT(0);
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub unsafe fn show_tray_menu(hwnd: HWND) -> Option<u32> {
    let hmenu = CreatePopupMenu().ok()?;
    let _ = AppendMenuW(hmenu, MF_STRING, TRAY_CMD_SETTINGS as usize, w!("Preferences..."));
    let _ = SetMenuDefaultItem(hmenu, TRAY_CMD_SETTINGS, 0);
    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());

    let is_enabled = SERVICE_ENABLED.load(Ordering::SeqCst);
    let service_label = if is_enabled {
        w!("Pause Service")
    } else {
        w!("Resume Service")
    };
    let _ = AppendMenuW(hmenu, MF_STRING, TRAY_CMD_TOGGLE_SERVICE as usize, service_label);

    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(hmenu, MF_STRING, TRAY_CMD_EXIT as usize, w!("Exit FileUC"));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);

    let _ = SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        hmenu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(hmenu);
    if cmd.0 > 0 {
        Some(cmd.0 as u32)
    } else {
        None
    }
}

pub struct TrayIcon {
    hwnd: HWND,
    nid: NOTIFYICONDATAW,
}

impl TrayIcon {
    pub fn new(event_tx: Sender<TrayEvent>, initial_service_enabled: bool) -> Self {
        SERVICE_ENABLED.store(initial_service_enabled, Ordering::SeqCst);
        unsafe {
            if let Ok(mut guard) = TRAY_EVENT_TX.lock() {
                *guard = Some(event_tx);
            }

            let h_instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(tray_wnd_proc),
                hInstance: h_instance,
                lpszClassName: TRAY_WINDOW_CLASS,
                ..Default::default()
            };
            let _ = RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                TRAY_WINDOW_CLASS,
                w!("FileUC_TrayWindow"),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                h_instance,
                None,
            )
            .unwrap_or_default();

            let h_icon = LoadIconW(h_instance, PCWSTR(1 as *const u16))
                .or_else(|_| LoadIconW(None, IDI_APPLICATION))
                .unwrap_or_default();

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_TIP | NIF_MESSAGE,
                uCallbackMessage: WM_TRAY_CALLBACK,
                hIcon: h_icon,
                ..Default::default()
            };

            let tip = if initial_service_enabled {
                "FileUC - Active"
            } else {
                "FileUC - Paused"
            };
            for (i, c) in tip.encode_utf16().enumerate().take(nid.szTip.len() - 1) {
                nid.szTip[i] = c;
            }

            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            // Enable NOTIFYICON_VERSION_4 so Windows routes NIN_SELECT reliably
            let nid_v = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                Anonymous: NOTIFYICONDATAW_0 { uVersion: 4 },
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid_v);

            Self { hwnd, nid }
        }
    }

    pub fn set_service_enabled(&mut self, enabled: bool) {
        SERVICE_ENABLED.store(enabled, Ordering::SeqCst);
        unsafe {
            let tip = if enabled {
                "FileUC - Active"
            } else {
                "FileUC - Paused"
            };
            self.nid.szTip = [0; 128];
            for (i, c) in tip.encode_utf16().enumerate().take(self.nid.szTip.len() - 1) {
                self.nid.szTip[i] = c;
            }
            self.nid.uFlags = NIF_TIP;
            let _ = Shell_NotifyIconW(NIM_MODIFY, &self.nid);
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_icon_creation() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut tray = TrayIcon::new(tx, true);
        println!("Tray HWND: {:?}", tray.hwnd);
        assert!(!tray.hwnd.is_invalid(), "HWND must not be invalid!");
        assert!(tray.hwnd.0 != 0 as _, "HWND must not be null!");
        tray.set_service_enabled(false);
        tray.set_service_enabled(true);
    }
}

