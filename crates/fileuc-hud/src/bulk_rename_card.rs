//! High-DPI Bulk Rename Modal UI Card.
//! Renders a native modal dialog for renaming multiple files cleanly in selection order.
//! Powered by shared `card_common` primitives for a unified, maintainable Win32 HUD experience.

use crate::card_common::{
    draw_action_button, draw_card_body, draw_card_shadow, draw_close_button, draw_input_box,
    is_cursor_visible,
};
use crate::font::FontContext;
use crate::spring::Spring;
use crate::theme::Theme;
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub struct BulkRenameCardState {
    pub file_count: usize,
    pub sample_ext: String,
    pub input_text: String,
    pub pop_in: Spring,
    pub is_close_hovered: bool,
    pub is_submit_hovered: bool,
    pub cursor_timer: f32,
    pub last_cursor_visible: bool,
    pub dpi_scale: f32,
}

impl BulkRenameCardState {
    pub fn new(file_count: usize, sample_ext: &str, dpi_scale: f32) -> Self {
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        let formatted_ext = if sample_ext.is_empty() || sample_ext.starts_with('.') {
            sample_ext.to_string()
        } else {
            format!(".{}", sample_ext)
        };

        Self {
            file_count,
            sample_ext: formatted_ext,
            input_text: String::new(),
            pop_in,
            is_close_hovered: false,
            is_submit_hovered: false,
            cursor_timer: 0.0,
            last_cursor_visible: true,
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
        }
    }

    pub fn paste_text(&mut self, s: &str) {
        for c in s.chars() {
            if self.input_text.len() >= 64 {
                break;
            }
            if !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
                && !c.is_control()
            {
                self.input_text.push(c);
            }
        }
    }

    pub fn push_char(&mut self, c: char) {
        // Disallow standard invalid Windows filename characters: \ / : * ? " < > |
        if !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            && !c.is_control()
            && self.input_text.len() < 64
        {
            self.input_text.push(c);
        }
    }

    pub fn pop_char(&mut self) {
        self.input_text.pop();
    }

    pub fn clear(&mut self) {
        self.input_text.clear();
    }

    pub fn card_bounds(&self, center_x: f32, center_y: f32) -> (f32, f32, f32, f32) {
        let w = 340.0 * self.dpi_scale;
        let h = 204.0 * self.dpi_scale;
        (center_x - w / 2.0, center_y - h / 2.0, w, h)
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let animating = self.pop_in.tick(dt);
        let cur_visible = is_cursor_visible();
        let blink_changed = cur_visible != self.last_cursor_visible;
        if blink_changed {
            self.last_cursor_visible = cur_visible;
        }
        animating || blink_changed
    }

    pub fn hit_test_card(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (l, t, w, h) = self.card_bounds(cx, cy);
        let px = cx + rel_x;
        let py = cy + rel_y;
        px >= l && px <= l + w && py >= t && py <= t + h
    }

    pub fn hit_test_close(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (l, t, w, _) = self.card_bounds(cx, cy);
        let btn_x = l + w - 22.0 * self.dpi_scale;
        let btn_y = t + 22.0 * self.dpi_scale;
        let px = cx + rel_x;
        let py = cy + rel_y;
        let dist = ((px - btn_x).powi(2) + (py - btn_y).powi(2)).sqrt();
        dist <= 18.0 * self.dpi_scale
    }

    pub fn hit_test_submit(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (_, t, _, _) = self.card_bounds(cx, cy);
        let btn_h = 40.0 * self.dpi_scale;
        let btn_w = 296.0 * self.dpi_scale;
        let btn_x = cx - btn_w / 2.0;
        let btn_y = t + 144.0 * self.dpi_scale;
        let px = cx + rel_x;
        let py = cy + rel_y;

        px >= btn_x && px <= btn_x + btn_w && py >= btn_y && py <= btn_y + btn_h
    }
}

pub fn render_bulk_rename_card(
    state: &BulkRenameCardState,
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

    // 4. Header: Title & Subtitle
    fonts.draw_text_left(
        pixmap,
        "Bulk Rename",
        left + 22.0 * dpi * scale,
        top + 24.0 * dpi * scale,
        18.5 * dpi * scale,
        theme.text_primary,
        true,
    );

    let sub_text = format!("Renaming {} item(s) in selection order", state.file_count);
    fonts.draw_text_left(
        pixmap,
        &sub_text,
        left + 22.0 * dpi * scale,
        top + 44.0 * dpi * scale,
        13.0 * dpi * scale,
        theme.text_secondary,
        false,
    );

    // 5. Input Field (Height 40px, 8.0px rounded corners)
    let input_y = top + 68.0 * dpi * scale;
    let input_w = 296.0 * dpi * scale;
    let input_h = 40.0 * dpi * scale;
    let input_x = cx - input_w / 2.0;
    let in_corner_r = 8.0 * dpi * scale;

    draw_input_box(pixmap, input_x, input_y, input_w, input_h, in_corner_r, scale, dpi, theme);

    let input_center_y = input_y + input_h / 2.0;

    // Input text / placeholder (vertically centered)
    if state.input_text.is_empty() {
        fonts.draw_text_left(
            pixmap,
            "Base name (e.g. Trip)",
            input_x + 14.0 * dpi * scale,
            input_center_y,
            14.5 * dpi * scale,
            theme.text_secondary,
            false,
        );
    } else {
        fonts.draw_text_left(
            pixmap,
            &state.input_text,
            input_x + 14.0 * dpi * scale,
            input_center_y,
            15.5 * dpi * scale,
            theme.text_primary,
            true,
        );
    }

    // Cursor blink (vertically centered, crisp 1.2px caret)
    if is_cursor_visible() {
        let text_w = if state.input_text.is_empty() {
            0.0
        } else {
            fonts.measure_text(&state.input_text, 15.5 * dpi * scale, true)
        };
        let cursor_x = input_x + 14.0 * dpi * scale + text_w + 2.0 * dpi * scale;
        let cursor_half = 9.0 * dpi * scale;
        let mut cur_pb = PathBuilder::new();
        cur_pb.move_to(cursor_x, input_center_y - cursor_half);
        cur_pb.line_to(cursor_x, input_center_y + cursor_half);
        if let Some(cur_path) = cur_pb.finish() {
            let mut cur_paint = Paint::default();
            cur_paint.set_color(theme.text_primary);
            cur_paint.anti_alias = true;
            let str_cfg = Stroke {
                width: 1.2 * dpi * scale,
                line_cap: LineCap::Butt,
                ..Default::default()
            };
            pixmap.stroke_path(&cur_path, &cur_paint, &str_cfg, Transform::identity(), None);
        }
    }

    // 6. Dynamic Live Preview line (tight spacing, 12px gap from input, bounds-checked)
    let preview_y = top + 120.0 * dpi * scale;
    let base = if state.input_text.trim().is_empty() {
        "Trip"
    } else {
        state.input_text.trim()
    };
    let ext = &state.sample_ext;
    let preview_text = if state.file_count <= 2 {
        format!("Preview: {base}_01{ext}, {base}_02{ext}")
    } else {
        format!("Preview: {base}_01{ext}, {base}_02{ext}, ... {base}_{:02}{ext}", state.file_count)
    };

    let font_size = 13.0 * dpi * scale;
    let max_w = input_w - 4.0 * dpi * scale;
    let mut display_text = preview_text;
    if fonts.measure_text(&display_text, font_size, false) > max_w {
        while !display_text.is_empty()
            && fonts.measure_text(&format!("{display_text}..."), font_size, false) > max_w
        {
            display_text.pop();
        }
        let trimmed = display_text.trim_end_matches([',', ' ']);
        display_text = format!("{trimmed}...");
    }

    fonts.draw_text_left(
        pixmap,
        &display_text,
        input_x + 2.0 * dpi * scale,
        preview_y,
        font_size,
        theme.text_secondary,
        false,
    );

    // 7. Action Button [ Rename All ] (tight spacing, 10px gap from preview)
    let btn_y = top + 144.0 * dpi * scale;
    let btn_w = 296.0 * dpi * scale;
    let btn_h = 40.0 * dpi * scale;
    let btn_x = cx - btn_w / 2.0;
    let btn_corner_r = 8.0 * dpi * scale;
    let has_name = !state.input_text.trim().is_empty();

    draw_action_button(
        pixmap,
        btn_x,
        btn_y,
        btn_w,
        btn_h,
        "Rename All",
        has_name,
        state.is_submit_hovered,
        false,
        btn_corner_r,
        scale,
        dpi,
        theme,
        fonts,
    );
}
