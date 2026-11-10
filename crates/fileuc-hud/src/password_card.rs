//! High-DPI Password Modal UI Card for PDF Encryption and Decryption.
//! Renders an animated modal dialog with spring pop-in physics, brand styling,
//! masked password input, and interactive submission controls.

use crate::font::FontContext;
use crate::spring::Spring;
use crate::theme::Theme;
use std::path::PathBuf;
use tiny_skia::{
    Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

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
    pub cursor_timer: f32,
    pub dpi_scale: f32,
}

impl PasswordCardState {
    pub fn new(file_path: PathBuf, dpi_scale: f32) -> Self {
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
            cursor_timer: 0.0,
            dpi_scale,
        }
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
        let mut animating = self.pop_in.tick(dt);
        self.cursor_timer = (self.cursor_timer + dt) % 1.0;
        if self.is_submitting {
            animating = true;
        }
        animating
    }

    pub fn card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let card_w = 340.0 * self.dpi_scale;
        let card_h = 226.0 * self.dpi_scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn hit_test_close(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (left, top, w, _) = self.card_bounds(cx, cy);
        let btn_cx = left + w - 24.0 * self.dpi_scale;
        let btn_cy = top + 24.0 * self.dpi_scale;
        let hit_radius = 20.0 * self.dpi_scale;
        let dx = (cx + rel_x) - btn_cx;
        let dy = (cy + rel_y) - btn_cy;
        (dx * dx + dy * dy) <= (hit_radius * hit_radius)
    }

    pub fn hit_test_submit(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let px = cx + rel_x;
        let py = cy + rel_y;
        let (_, top, _, _) = self.card_bounds(cx, cy);
        let btn_w = 290.0 * self.dpi_scale;
        let btn_h = 44.0 * self.dpi_scale;
        let btn_left = cx - btn_w / 2.0;
        let btn_top = top + 146.0 * self.dpi_scale;

        px >= btn_left && px <= (btn_left + btn_w) && py >= btn_top && py <= (btn_top + btn_h)
    }

    pub fn hit_test_card(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (left, top, w, h) = self.card_bounds(cx, cy);
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
    let base_h = 226.0 * dpi;
    let half_w = (base_w / 2.0) * scale;
    let half_h = (base_h / 2.0) * scale;
    let left = cx - half_w;
    let top = cy - half_h;
    let w = base_w * scale;
    let h = base_h * scale;
    let corner_r = 18.0 * dpi * scale;

    // 1. Soft Ambient Shadow (Layered for visual depth)
    for i in 1..=4 {
        let shadow_margin = (i as f32) * 2.5 * dpi * scale;
        let mut shadow_pb = PathBuilder::new();
        push_rounded_rect(
            &mut shadow_pb,
            left - shadow_margin,
            top - shadow_margin + 3.0 * dpi * scale,
            w + shadow_margin * 2.0,
            h + shadow_margin * 2.0,
            corner_r + shadow_margin,
        );
        if let Some(shadow_path) = shadow_pb.finish() {
            let mut shadow_paint = Paint::default();
            shadow_paint.set_color_rgba8(0, 0, 0, 7);
            shadow_paint.anti_alias = true;
            pixmap.fill_path(&shadow_path, &shadow_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 2. Rounded Rectangle Solid Card Body
    let mut card_pb = PathBuilder::new();
    push_rounded_rect(&mut card_pb, left, top, w, h, corner_r);
    if let Some(card_path) = card_pb.finish() {
        let mut card_paint = Paint::default();
        if theme.is_dark {
            card_paint.set_color_rgba8(28, 28, 30, 255); // Deep Charcoal
        } else {
            card_paint.set_color_rgba8(255, 255, 255, 255); // Crisp White
        }
        card_paint.anti_alias = true;
        pixmap.fill_path(&card_path, &card_paint, FillRule::Winding, Transform::identity(), None);

        // Card Border
        let mut border_paint = Paint::default();
        border_paint.set_color(theme.border_subtle);
        border_paint.anti_alias = true;
        let border_stroke = Stroke {
            width: 1.0 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&card_path, &border_paint, &border_stroke, Transform::identity(), None);
    }

    // 3. Close Button ("✕")
    let close_x = cx + (base_w / 2.0 - 24.0 * dpi) * scale;
    let close_y = cy - (base_h / 2.0 - 24.0 * dpi) * scale;

    // Hover background pill/circle behind close button
    if state.is_close_hovered {
        let hover_r = 14.0 * dpi * scale;
        let mut hover_pb = PathBuilder::new();
        hover_pb.push_circle(close_x, close_y, hover_r);
        if let Some(hover_path) = hover_pb.finish() {
            let mut hover_paint = Paint::default();
            if theme.is_dark {
                hover_paint.set_color_rgba8(255, 255, 255, 22);
            } else {
                hover_paint.set_color_rgba8(0, 0, 0, 16);
            }
            hover_paint.anti_alias = true;
            pixmap.fill_path(&hover_path, &hover_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    let close_half = 5.0 * dpi * scale;
    let mut close_pb = PathBuilder::new();
    close_pb.move_to(close_x - close_half, close_y - close_half);
    close_pb.line_to(close_x + close_half, close_y + close_half);
    close_pb.move_to(close_x + close_half, close_y - close_half);
    close_pb.line_to(close_x - close_half, close_y + close_half);
    if let Some(close_path) = close_pb.finish() {
        let mut close_paint = Paint::default();
        if state.is_close_hovered {
            close_paint.set_color(theme.text_primary);
        } else {
            close_paint.set_color(theme.text_secondary);
        }
        close_paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.8 * dpi * scale,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        pixmap.stroke_path(&close_path, &close_paint, &stroke, Transform::identity(), None);
    }

    // 4. Header: Mode Title & File Name
    let title_text = "Protect PDF";
    let title_y = top + 34.0 * dpi * scale;
    fonts.draw_text_centered(
        pixmap,
        title_text,
        cx,
        title_y,
        19.5 * dpi * scale,
        theme.text_primary,
        true,
    );

    let sub_y = top + 59.0 * dpi * scale;
    if let Some(ref err) = state.error_msg {
        let red_color = Color::from_rgba8(255, 69, 58, 255);
        fonts.draw_text_centered(
            pixmap,
            err,
            cx,
            sub_y,
            13.0 * dpi * scale,
            red_color,
            false,
        );
    } else {
        let display_name = truncate_filename(&state.file_name, 42);
        fonts.draw_text_centered(
            pixmap,
            &display_name,
            cx,
            sub_y,
            14.0 * dpi * scale,
            theme.text_secondary,
            false,
        );
    }

    // 5. Password Input Box
    let input_w = 290.0 * dpi * scale;
    let input_h = 44.0 * dpi * scale;
    let input_left = cx - input_w / 2.0;
    let input_top = top + 84.0 * dpi * scale;
    let input_corner_r = 10.0 * dpi * scale;

    let mut input_pb = PathBuilder::new();
    push_rounded_rect(&mut input_pb, input_left, input_top, input_w, input_h, input_corner_r);
    if let Some(input_path) = input_pb.finish() {
        let mut input_bg = Paint::default();
        if theme.is_dark {
            input_bg.set_color_rgba8(44, 44, 46, 255); // Elevated dark field
        } else {
            input_bg.set_color_rgba8(243, 244, 246, 255); // Elevated light field
        }
        input_bg.anti_alias = true;
        pixmap.fill_path(&input_path, &input_bg, FillRule::Winding, Transform::identity(), None);

        // Active purple focus border
        let mut input_border = Paint::default();
        input_border.set_color(theme.slice_hover_fill);
        input_border.anti_alias = true;
        let border_stroke = Stroke {
            width: 1.5 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&input_path, &input_border, &border_stroke, Transform::identity(), None);
    }

    // 6. Password Text / Placeholder
    let input_center_y = input_top + input_h / 2.0;
    if state.password_input.is_empty() {
        fonts.draw_text_centered(
            pixmap,
            "Enter password...",
            cx,
            input_center_y,
            14.5 * dpi * scale,
            theme.text_secondary,
            false,
        );
    } else {
        // Masked bullets: "•••••"
        let bullets: String = "\u{2022}".repeat(state.password_input.len());
        fonts.draw_text_centered(
            pixmap,
            &bullets,
            cx,
            input_center_y,
            16.5 * dpi * scale,
            theme.text_primary,
            true,
        );
    }

    // 7. Submit Action Button
    let btn_w = 290.0 * dpi * scale;
    let btn_h = 44.0 * dpi * scale;
    let btn_left = cx - btn_w / 2.0;
    let btn_top = top + 146.0 * dpi * scale;
    let btn_corner_r = 10.0 * dpi * scale;

    let mut btn_pb = PathBuilder::new();
    push_rounded_rect(&mut btn_pb, btn_left, btn_top, btn_w, btn_h, btn_corner_r);
    if let Some(btn_path) = btn_pb.finish() {
        let mut btn_paint = Paint::default();
        if state.is_submit_hovered {
            let base_color = theme.slice_hover_fill;
            let r = (base_color.red() * 255.0).min(235.0) as u8 + 20;
            let g = (base_color.green() * 255.0).min(235.0) as u8 + 20;
            let b = (base_color.blue() * 255.0).min(235.0) as u8 + 20;
            btn_paint.set_color_rgba8(r, g, b, 255);
        } else {
            btn_paint.set_color(theme.slice_hover_fill);
        }
        btn_paint.anti_alias = true;
        pixmap.fill_path(&btn_path, &btn_paint, FillRule::Winding, Transform::identity(), None);
    }

    let btn_text = if state.is_submitting {
        "Processing..."
    } else {
        "Protect & Save"
    };
    let btn_center_y = btn_top + btn_h / 2.0;
    fonts.draw_text_centered(
        pixmap,
        btn_text,
        cx,
        btn_center_y,
        15.5 * dpi * scale,
        Color::WHITE,
        true,
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

fn push_rounded_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
}
