use tiny_skia::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub is_dark: bool,
    pub slice_fill: Color,
    pub slice_hover_fill: Color,
    pub slice_hover_border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_on_accent: Color,
    pub center_fill: Color,
    pub danger_fill: Color,
    pub danger_border: Color,
    pub danger_text: Color,
    pub border_subtle: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            is_dark: true,
            slice_fill: Color::from_rgba8(36, 36, 38, 255),
            slice_hover_fill: Color::from_rgba8(112, 102, 224, 255),  // Filee signature purple
            slice_hover_border: Color::from_rgba8(163, 156, 244, 255),// Crisp bright violet border
            text_primary: Color::from_rgba8(245, 243, 238, 255),
            text_secondary: Color::from_rgba8(175, 173, 166, 255),
            text_on_accent: Color::from_rgba8(255, 255, 255, 255),
            center_fill: Color::from_rgba8(26, 26, 28, 255),
            danger_fill: Color::from_rgba8(82, 30, 36, 255),          // Slight red tint like Filee
            danger_border: Color::from_rgba8(240, 100, 100, 200),     // Soft red border
            danger_text: Color::from_rgba8(255, 128, 128, 255),
            border_subtle: Color::from_rgba8(255, 255, 255, 22),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            slice_fill: Color::from_rgba8(255, 255, 255, 255),
            slice_hover_fill: Color::from_rgba8(108, 99, 216, 255),
            slice_hover_border: Color::from_rgba8(75, 65, 185, 255),
            text_primary: Color::from_rgba8(30, 29, 28, 255),
            text_secondary: Color::from_rgba8(110, 108, 104, 255),
            text_on_accent: Color::from_rgba8(255, 255, 255, 255),
            center_fill: Color::from_rgba8(252, 252, 252, 255),
            danger_fill: Color::from_rgba8(254, 236, 236, 255),       // Slight rose/red tint
            danger_border: Color::from_rgba8(235, 90, 90, 190),
            danger_text: Color::from_rgba8(215, 45, 45, 255),
            border_subtle: Color::from_rgba8(0, 0, 0, 20),
        }
    }

    pub fn resolve(theme_choice: &str) -> Self {
        match theme_choice.trim().to_ascii_lowercase().as_str() {
            "dark" => Self::dark(),
            "light" => Self::light(),
            _ => {
                if is_windows_dark_mode() {
                    Self::dark()
                } else {
                    Self::light()
                }
            }
        }
    }
}

pub fn is_windows_dark_mode() -> bool {
    #[cfg(windows)]
    {
        use windows::core::w;
        use windows::Win32::System::Registry::{
            RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
        };
        let mut data: u32 = 0;
        let mut data_len = std::mem::size_of::<u32>() as u32;
        let res = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut data as *mut u32 as *mut std::ffi::c_void),
                Some(&mut data_len),
            )
        };
        if res.is_ok() {
            return data == 0;
        }
    }
    false
}
