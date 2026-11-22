//! High-DPI Password Modal UI Card for PDF Encryption and Decryption.
//! Renders an animated modal dialog with spring pop-in physics, brand styling,
//! masked password input, and interactive submission controls.
//! Powered by shared `card_common` primitives for a unified, maintainable Win32 HUD experience.

use crate::card_common::{
    draw_action_button, draw_card_body, draw_card_shadow, draw_close_button, draw_input_box,
    draw_text_cursor, is_cursor_visible,
};
use crate::font::FontContext;
use crate::spring::Spring;
use crate::theme::Theme;
use std::path::PathBuf;
use tiny_skia::{Color, Pixmap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordCardMode {
    Lock,
    Unlock,
}

#[derive(Debug, Clone)]
pub struct PasswordCardState {
    pub file_path: PathBuf,
    pub file_name: String,
    pub password_input: String,
    pub pop_in: Spring,
    pub error_msg: Option<String>,
    pub is_submitting: bool,
    pub is_close_hovered: bool,
    pub is_submit_hovered: bool,
    pub last_cursor_visible: bool,
    pub dpi_scale: f32,
    pub mode: PasswordCardMode,
    pub show_password: bool,
}

impl PasswordCardState {
    pub fn new(file_path: PathBuf, dpi_scale: f32) -> Self {
        Self::new_with_mode(file_path, dpi_scale, PasswordCardMode::Lock)
    }

    pub fn new_with_mode(file_path: PathBuf, dpi_scale: f32, mode: PasswordCardMode) -> Self {
        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        Self {
            file_path,
            file_name,
            password_input: String::new(),
            pop_in,
            error_msg: None,
            is_submitting: false,
            is_close_hovered: false,
            is_submit_hovered: false,
            last_cursor_visible: true,
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
            mode,
            show_password: false,
        }
    }

    pub fn toggle_show_password(&mut self) {
        self.show_password = !self.show_password;
    }

    pub fn push_char(&mut self, c: char) {
        if self.password_input.len() < 128 && !c.is_control() {
            self.password_input.push(c);
            self.error_msg = None;
        }
    }

    pub fn pop_char(&mut self) {
        self.password_input.pop();
        self.error_msg = None;
    }

    pub fn clear(&mut self) {
        self.password_input.clear();
        self.error_msg = None;
    }

    pub fn set_error(&mut self, err: &str) {
        self.error_msg = Some(err.to_string());
        self.is_submitting = false;
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let animating = self.pop_in.tick(dt);
        let cur_visible = is_cursor_visible();
        let blink_changed = cur_visible != self.last_cursor_visible;
        if blink_changed {
            self.last_cursor_visible = cur_visible;
        }
        animating || blink_changed || self.is_submitting
    }

    pub fn card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let card_w = 340.0 * self.dpi_scale;
        let card_h = 204.0 * self.dpi_scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn scaled_card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let card_w = 340.0 * self.dpi_scale * scale;
        let card_h = 204.0 * self.dpi_scale * scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn hit_test_close(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let (left, top, w, _) = self.scaled_card_bounds(cx, cy);
        let btn_cx = left + w - 22.0 * self.dpi_scale * scale;
        let btn_cy = top + 22.0 * self.dpi_scale * scale;
        let hit_radius = 18.0 * self.dpi_scale * scale;
        let dx = (cx + rel_x) - btn_cx;
        let dy = (cy + rel_y) - btn_cy;
        (dx * dx + dy * dy) <= (hit_radius * hit_radius)
    }

    pub fn hit_test_submit(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let px = cx + rel_x;
        let py = cy + rel_y;
        let (_, top, _, _) = self.scaled_card_bounds(cx, cy);
        let btn_w = 296.0 * self.dpi_scale * scale;
        let btn_h = 40.0 * self.dpi_scale * scale;
        let btn_left = cx - btn_w / 2.0;
        let btn_top = top + 144.0 * self.dpi_scale * scale;

        px >= btn_left && px <= (btn_left + btn_w) && py >= btn_top && py <= (btn_top + btn_h)
    }

    pub fn hit_test_card(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (left, top, w, h) = self.scaled_card_bounds(cx, cy);
        let px = cx + rel_x;
        let py = cy + rel_y;
        px >= left && px <= (left + w) && py >= top && py <= (top + h)
    }
}

pub fn render_password_card(
    state: &PasswordCardState,
    theme: &Theme,
    fonts: &FontContext,
    pixmap: &mut Pixmap,
) {
    pixmap.fill(Color::TRANSPARENT);

    let scale = state.pop_in.value();
    if scale <= 0.01 {
        return;
    }

    let cx = pixmap.width() as f32 / 2.0;
    let cy = pixmap.height() as f32 / 2.0;
    let dpi = state.dpi_scale;

    let base_w = 340.0 * dpi;
    let base_h = 204.0 * dpi;
    let half_w = (base_w / 2.0) * scale;
    let half_h = (base_h / 2.0) * scale;
    let left = cx - half_w;
    let top = cy - half_h;
    let w = base_w * scale;
    let h = base_h * scale;
    let corner_r = 20.0 * dpi * scale;

    // 1. Soft Ambient Drop Shadows
    draw_card_shadow(pixmap, left, top, w, h, corner_r, scale, dpi);

    // 2. Rounded Solid Card Body
    draw_card_body(pixmap, left, top, w, h, corner_r, scale, dpi, theme);

    // 3. Close Button
    let close_x = left + w - 22.0 * dpi * scale;
    let close_y = top + 22.0 * dpi * scale;
    draw_close_button(pixmap, close_x, close_y, state.is_close_hovered, scale, dpi, theme);

    // 4. Header Title & Subtitle
    let title_text = match state.mode {
        PasswordCardMode::Lock => "Protect PDF",
        PasswordCardMode::Unlock => "Unlock PDF",
    };
    fonts.draw_text_left(
        pixmap,
        title_text,
        left + 22.0 * dpi * scale,
        top + 24.0 * dpi * scale,
        18.5 * dpi * scale,
        theme.text_primary,
        true,
    );

    let sub_y = top + 44.0 * dpi * scale;
    match state.mode {
        PasswordCardMode::Unlock => {
            if state.error_msg.is_some() {
                let red_color = Color::from_rgba8(255, 69, 58, 255);
                fonts.draw_text_left(
                    pixmap,
                    "Incorrect password. Please try again.",
                    left + 22.0 * dpi * scale,
                    sub_y,
                    13.0 * dpi * scale,
                    red_color,
                    true,
                );
            } else {
                fonts.draw_text_left(
                    pixmap,
                    "Enter valid document open password",
                    left + 22.0 * dpi * scale,
                    sub_y,
                    13.0 * dpi * scale,
                    theme.text_secondary,
                    false,
                );
            }
        }
        PasswordCardMode::Lock => {
            if let Some(ref err) = state.error_msg {
                let red_color = Color::from_rgba8(255, 69, 58, 255);
                fonts.draw_text_left(
                    pixmap,
                    err,
                    left + 22.0 * dpi * scale,
                    sub_y,
                    13.0 * dpi * scale,
                    red_color,
                    false,
                );
            } else {
                let display_name = truncate_filename(&state.file_name, 20);
                fonts.draw_text_left(
                    pixmap,
                    &display_name,
                    left + 22.0 * dpi * scale,
                    sub_y,
                    13.0 * dpi * scale,
                    theme.text_secondary,
                    false,
                );
            }
        }
    }

    // 5. Password Input Box (Height 40px, subtle border, NO colored focus ring!)
    let input_w = 296.0 * dpi * scale;
    let input_h = 40.0 * dpi * scale;
    let input_left = cx - input_w / 2.0;
    let input_top = top + 68.0 * dpi * scale;
    let input_corner_r = 8.0 * dpi * scale;

    draw_input_box(pixmap, input_left, input_top, input_w, input_h, input_corner_r, scale, dpi, theme);

    // 6. Password Text / Placeholder (Vertically centered)
    let input_center_y = input_top + input_h / 2.0;
    let font_size = 16.0 * dpi * scale;
    if state.password_input.is_empty() {
        let placeholder = match state.mode {
            PasswordCardMode::Lock => "Enter password...",
            PasswordCardMode::Unlock => "Enter document password...",
        };
        fonts.draw_text_left(
            pixmap,
            placeholder,
            input_left + 14.0 * dpi * scale,
            input_center_y,
            font_size,
            theme.text_secondary,
            false,
        );
    } else if state.show_password {
        // Plaintext password visible on Tab press
        fonts.draw_text_left(
            pixmap,
            &state.password_input,
            input_left + 14.0 * dpi * scale,
            input_center_y,
            font_size,
            theme.text_primary,
            true,
        );
    } else {
        // Masked bullets: "•••••"
        let bullets: String = "\u{2022}".repeat(state.password_input.len());
        fonts.draw_text_left(
            pixmap,
            &bullets,
            input_left + 14.0 * dpi * scale,
            input_center_y,
            font_size,
            theme.text_primary,
            true,
        );
    }

    // Blinking Cursor Line (crisp 1.2px caret)
    let text_w = if state.password_input.is_empty() {
        0.0
    } else if state.show_password {
        fonts.measure_text(&state.password_input, font_size, true)
    } else {
        let bullets: String = "\u{2022}".repeat(state.password_input.len());
        fonts.measure_text(&bullets, font_size, true)
    };
    let cursor_x = input_left + 14.0 * dpi * scale + text_w + 2.0 * dpi * scale;
    draw_text_cursor(
        pixmap,
        cursor_x,
        input_center_y,
        18.0 * dpi * scale,
        scale,
        dpi,
        theme.text_primary,
    );

    // 7. Hint text below input: "Press Tab to view or hide"
    let hint_y = top + 120.0 * dpi * scale;
    fonts.draw_text_left(
        pixmap,
        "Press Tab to view or hide",
        input_left + 2.0 * dpi * scale,
        hint_y,
        12.5 * dpi * scale,
        theme.text_secondary,
        false,
    );

    // 8. Action Button
    let btn_w = 296.0 * dpi * scale;
    let btn_h = 40.0 * dpi * scale;
    let btn_left = cx - btn_w / 2.0;
    let btn_top = top + 144.0 * dpi * scale;
    let btn_corner_r = 8.0 * dpi * scale;

    let btn_text = if state.is_submitting {
        match state.mode {
            PasswordCardMode::Lock => "Protecting...",
            PasswordCardMode::Unlock => "Unlocking...",
        }
    } else {
        match state.mode {
            PasswordCardMode::Lock => "Protect & Save",
            PasswordCardMode::Unlock => "Unlock Document",
        }
    };

    let has_password = !state.password_input.is_empty();

    draw_action_button(
        pixmap,
        btn_left,
        btn_top,
        btn_w,
        btn_h,
        btn_text,
        has_password,
        state.is_submit_hovered,
        false,
        btn_corner_r,
        scale,
        dpi,
        theme,
        fonts,
    );
}

fn truncate_filename(name: &str, max_chars: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_chars {
        name.to_string()
    } else {
        let half = (max_chars.saturating_sub(3)) / 2;
        let prefix: String = name.chars().take(half).collect();
        let suffix: String = name.chars().skip(char_count.saturating_sub(half)).collect();
        format!("{}...{}", prefix, suffix)
    }
}
