use crate::drop_target::{DropTargetEvent, FileDropTarget};
use crate::surface::BlitSurface;
use std::sync::mpsc::Sender;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize, RegisterDragDrop, RevokeDragDrop};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE,
    SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};

const WINDOW_CLASS_NAME: PCWSTR = w!("FileUC_HudWindow");

pub const DEFAULT_OUTER_RADIUS_DIP: f32 = 150.0;
pub const HOLE_RATIO: f32 = 0.42;
pub const POP_OUT_DIP: f32 = 7.0;
pub const SHADOW_MARGIN_DIP: f32 = 14.0;
pub const BASE_SIDE_DIP: f32 = 384.0; // Ample canvas space for 150px outer radius donut, 340px modal cards, and 20px ambient blur margins

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
    center_x: i32,
    center_y: i32,
    size: u32,
    dpi_scale: f32,
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

            let initial_dpi = GetDpiForSystem();
            let initial_dpi = if initial_dpi == 0 { 96 } else { initial_dpi };
            let dpi_scale = initial_dpi as f32 / 96.0;
            let initial_size = (BASE_SIDE_DIP * dpi_scale).ceil() as u32;

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                WINDOW_CLASS_NAME,
                w!("FileUC_HUD"),
                WS_POPUP,
                0,
                0,
                initial_size as i32,
                initial_size as i32,
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
                surface: BlitSurface::new(initial_size, initial_size),
                screen_x: 0,
                screen_y: 0,
                center_x: initial_size as i32 / 2,
                center_y: initial_size as i32 / 2,
                size: initial_size,
                dpi_scale,
            })
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    pub fn hud_center(&self) -> (i32, i32) {
        (self.center_x, self.center_y)
    }

    pub fn position_at_center(&mut self, cursor_x: i32, cursor_y: i32) -> (i32, i32) {
        let pt = POINT { x: cursor_x, y: cursor_y };
        let hmon = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
        let mut minfo = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let (work_left, work_top, work_right, work_bottom) = unsafe {
            if GetMonitorInfoW(hmon, &mut minfo).as_bool() {
                (
                    minfo.rcWork.left,
                    minfo.rcWork.top,
                    minfo.rcWork.right,
                    minfo.rcWork.bottom,
                )
            } else {
                (0, 0, 1920, 1080)
            }
        };

        // Query DPI directly for the monitor under the cursor (multi-monitor mixed-DPI aware)
        let dpi = unsafe {
            let mut dpi_x = 0u32;
            let mut dpi_y = 0u32;
            if windows::Win32::UI::HiDpi::GetDpiForMonitor(
                hmon,
                windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
                &mut dpi_x,
                &mut dpi_y,
            )
            .is_ok()
                && dpi_x > 0
            {
                dpi_x
            } else {
                let d = GetDpiForWindow(self.hwnd);
                if d == 0 {
                    GetDpiForSystem()
                } else {
                    d
                }
            }
        };
        let dpi = if dpi == 0 { 96 } else { dpi };
        let dpi_scale = dpi as f32 / 96.0;
        self.dpi_scale = dpi_scale;

        let side_px = (BASE_SIDE_DIP * dpi_scale).ceil() as u32;
        if side_px != self.size {
            self.size = side_px;
            self.surface = BlitSurface::new(side_px, side_px);
        }

        let ideal_x = cursor_x - (self.size as i32 / 2);
        let ideal_y = cursor_y - (self.size as i32 / 2);
        let max_x = (work_right - self.size as i32).max(work_left);
        let max_y = (work_bottom - self.size as i32).max(work_top);

        self.screen_x = ideal_x.clamp(work_left, max_x);
        self.screen_y = ideal_y.clamp(work_top, max_y);
        self.center_x = self.screen_x + (self.size as i32 / 2);
        self.center_y = self.screen_y + (self.size as i32 / 2);

        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                self.screen_x,
                self.screen_y,
                self.size as i32,
                self.size as i32,
                SWP_NOACTIVATE,
            );
        }

        (self.center_x, self.center_y)
    }

    pub fn show(&self) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub fn warmup_surface(&mut self) {
        // Pre-allocate DWM layered window composition resources while staying hidden
        if let Some(pixmap) = tiny_skia::Pixmap::new(self.size, self.size) {
            self.repaint(&pixmap);
        }
        self.hide();
    }

    pub fn prepare_for_drag(&mut self) {
        // Pre-fill with alpha = 1 (completely transparent to the eye, but hit-testable by Windows OLE)
        // This ensures:
        // 1. The layered window has an initialized alpha backing surface before ShowWindow is called (no black box).
        // 2. Windows OLE immediately delivers DragEnter because the pixel under the cursor has non-zero alpha.
        if let Some(mut pixmap) = tiny_skia::Pixmap::new(self.size, self.size) {
            pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 1));
            self.repaint(&pixmap);
        }
        self.show();
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn repaint(&mut self, pixmap: &tiny_skia::Pixmap) {
        self.surface.update_from_pixmap(pixmap);
        self.surface
            .blit_to_layered_window(self.hwnd, self.screen_x, self.screen_y);
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
