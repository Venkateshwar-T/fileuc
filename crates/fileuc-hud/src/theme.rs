use tiny_skia::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub is_dark: bool,
    pub slice_fill: Color,
    pub slice_hover_fill: Color,
    pub text_color: Color,
    pub center_fill: Color,
    pub accent_color: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            is_dark: true,
            slice_fill: Color::from_rgba8(35, 38, 48, 220),
            slice_hover_fill: Color::from_rgba8(124, 92, 252, 240), // Accent purple
            text_color: Color::from_rgba8(240, 242, 245, 255),
            center_fill: Color::from_rgba8(20, 22, 28, 230),
            accent_color: Color::from_rgba8(124, 92, 252, 255),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            slice_fill: Color::from_rgba8(245, 247, 250, 220),
            slice_hover_fill: Color::from_rgba8(100, 70, 240, 240),
            text_color: Color::from_rgba8(30, 32, 38, 255),
            center_fill: Color::from_rgba8(255, 255, 255, 240),
            accent_color: Color::from_rgba8(100, 70, 240, 255),
        }
    }
}
