use crate::drop_target::{DropTargetEvent, FileDropTarget};
use crate::surface::BlitSurface;
use std::sync::mpsc::Sender;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize, RegisterDragDrop, RevokeDragDrop};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE,
    SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};

const WINDOW_CLASS_NAME: PCWSTR = w!("FileUC_HudWindow");
const HUD_SIZE: u32 = 300;

unsafe extern "system" fn hud_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub struct HudWindow {
    hwnd: HWND,
    surface: BlitSurface,
    screen_x: i32,
    screen_y: i32,
    size: u32,
}

impl HudWindow {
    pub fn new(event_sender: Sender<DropTargetEvent>) -> Result<Self, windows::core::Error> {
        unsafe {
            let _ = OleInitialize(None);
            let hinstance: HINSTANCE = GetModuleHandleW(None)?.into();

            let wnd_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(hud_wnd_proc),
                hInstance: hinstance,
                lpszClassName: WINDOW_CLASS_NAME,
                ..Default::default()
            };
            let _ = RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                WINDOW_CLASS_NAME,
                w!("FileUC_HUD"),
                WS_POPUP,
                0,
                0,
                HUD_SIZE as i32,
                HUD_SIZE as i32,
                None,
                None,
                hinstance,
                None,
            )?;

            let drop_target = FileDropTarget::new(hwnd, event_sender);
            let idrop: windows::Win32::System::Ole::IDropTarget = drop_target.into();
            RegisterDragDrop(hwnd, &idrop)?;

            Ok(Self {
                hwnd,
                surface: BlitSurface::new(HUD_SIZE, HUD_SIZE),
                screen_x: 0,
                screen_y: 0,
                size: HUD_SIZE,
            })
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn position_at_center(&mut self, center_x: i32, center_y: i32) {
        self.screen_x = center_x - (self.size as i32 / 2);
        self.screen_y = center_y - (self.size as i32 / 2);

        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                self.screen_x,
                self.screen_y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn repaint(&mut self, pixmap: &tiny_skia::Pixmap) {
        self.surface.update_from_pixmap(pixmap);
        self.surface.blit_to_layered_window(self.hwnd, self.screen_x, self.screen_y);
    }
}

impl Drop for HudWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = RevokeDragDrop(self.hwnd);
            let _ = DestroyWindow(self.hwnd);
            OleUninitialize();
        }
    }
}
