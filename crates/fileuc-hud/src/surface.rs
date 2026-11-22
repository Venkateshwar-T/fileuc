use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

const AC_SRC_OVER: u8 = 0;
const AC_SRC_ALPHA: u8 = 1;

pub struct BlitSurface {
    width: u32,
    height: u32,
    gdi_context: Option<GdiContext>,
}

struct GdiContext {
    mem_dc: windows::Win32::Graphics::Gdi::HDC,
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    old_bitmap: windows::Win32::Graphics::Gdi::HGDIOBJ,
    bits: *mut u8,
}

unsafe impl Send for GdiContext {}

impl Drop for GdiContext {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.mem_dc, self.old_bitmap);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.mem_dc);
        }
    }
}

impl BlitSurface {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            gdi_context: None,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn as_bgra_bytes(&self) -> &[u8] {
        if let Some(ref gdi) = self.gdi_context {
            unsafe {
                std::slice::from_raw_parts(gdi.bits, (self.width * self.height * 4) as usize)
            }
        } else {
            &[]
        }
    }

    pub fn ensure_gdi_context(&mut self) {
        if self.gdi_context.is_some() {
            return;
        }
        unsafe {
            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                return;
            }

            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_invalid() {
                let _ = ReleaseDC(None, screen_dc);
                return;
            }

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.width as i32,
                    biHeight: -(self.height as i32), // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap: HBITMAP = match CreateDIBSection(
                mem_dc,
                &mut bmi,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            ) {
                Ok(bm) => bm,
                Err(_) => {
                    let _ = DeleteDC(mem_dc);
                    let _ = ReleaseDC(None, screen_dc);
                    return;
                }
            };

            if bitmap.is_invalid() || bits.is_null() {
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(None, screen_dc);
                return;
            }

            let old_bitmap = SelectObject(mem_dc, bitmap);
            let _ = ReleaseDC(None, screen_dc);
            self.gdi_context = Some(GdiContext {
                mem_dc,
                bitmap,
                old_bitmap,
                bits: bits as *mut u8,
            });
        }
    }

    pub fn update_from_pixmap(&mut self, pixmap: &tiny_skia::Pixmap) {
        self.ensure_gdi_context();

        let p_w = pixmap.width();
        let p_h = pixmap.height();
        let s_w = self.width;
        let s_h = self.height;

        if p_w == 0 || p_h == 0 || s_w == 0 || s_h == 0 {
            return;
        }

        let src = pixmap.data();

        if let Some(ref gdi) = self.gdi_context {
            unsafe {
                let dest = std::slice::from_raw_parts_mut(gdi.bits, (s_w * s_h * 4) as usize);

                if p_w == s_w && p_h == s_h {
                    let copy_len = (s_w * s_h * 4) as usize;
                    for (s, d) in src[..copy_len].chunks_exact(4).zip(dest.chunks_exact_mut(4)) {
                        d[0] = s[2]; // B
                        d[1] = s[1]; // G
                        d[2] = s[0]; // R
                        d[3] = s[3]; // A
                    }
                } else {
                    // Stride-aware row-by-row copy for mismatched dimensions to prevent shearing
                    let copy_w = p_w.min(s_w) as usize;
                    let copy_h = p_h.min(s_h) as usize;
                    let src_stride = (p_w * 4) as usize;
                    let dst_stride = (s_w * 4) as usize;

                    dest.fill(0);

                    for row in 0..copy_h {
                        let src_row = &src[row * src_stride..row * src_stride + copy_w * 4];
                        let dst_row = &mut dest[row * dst_stride..row * dst_stride + copy_w * 4];
                        for (s, d) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                            d[0] = s[2]; // B
                            d[1] = s[1]; // G
                            d[2] = s[0]; // R
                            d[3] = s[3]; // A
                        }
                    }
                }
            }
        }
    }

    pub fn blit_to_layered_window(&mut self, hwnd: HWND, screen_x: i32, screen_y: i32) {
        self.ensure_gdi_context();

        unsafe {
            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                return;
            }

            if let Some(ref gdi) = self.gdi_context {
                let pt_src = POINT { x: 0, y: 0 };
                let pt_dst = POINT {
                    x: screen_x,
                    y: screen_y,
                };
                let size = SIZE {
                    cx: self.width as i32,
                    cy: self.height as i32,
                };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA,
                };

                let _ = UpdateLayeredWindow(
                    hwnd,
                    screen_dc,
                    Some(&pt_dst),
                    Some(&size),
                    gdi.mem_dc,
                    Some(&pt_src),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );
            }

            let _ = ReleaseDC(None, screen_dc);
        }
    }
}
