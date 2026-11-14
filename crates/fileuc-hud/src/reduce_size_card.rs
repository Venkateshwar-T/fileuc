//! High-DPI Reduce Size Card.
//! Renders a compact modal popup (260px width) with number-only input and
//! segmented [MB | KB] tabs to specify a target compression size limit.
//! Powered by shared `card_common` primitives for 100% visual consistency.

use crate::card_common::{
    draw_action_button, draw_card_body, draw_card_shadow, draw_close_button, draw_input_box,
    draw_segmented_tabs, is_cursor_visible,
};
use crate::font::FontContext;
use crate::spring::Spring;
use crate::theme::Theme;
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeUnit {
    Mb,
    Kb,
}

pub struct ReduceSizeCardState {
    pub input_text: String,
    pub unit: SizeUnit,
    pub file_count: usize,
    pub pop_in: Spring,
    pub is_close_hovered: bool,
    pub is_submit_hovered: bool,
    pub hovered_unit: Option<SizeUnit>,
    pub cursor_timer: f32,
    pub last_cursor_visible: bool,
    pub dpi_scale: f32,
}

impl ReduceSizeCardState {
    pub fn new(file_count: usize, dpi_scale: f32) -> Self {
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        Self {
            input_text: String::new(),
            unit: SizeUnit::Mb,
            file_count,
            pop_in,
            is_close_hovered: false,
            is_submit_hovered: false,
            hovered_unit: None,
            cursor_timer: 0.0,
            last_cursor_visible: true,
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
        }
    }

    pub fn push_char(&mut self, ch: char) {
        // Strictly allow only numbers
        if ch.is_ascii_digit() && self.input_text.len() < 8 {
            self.input_text.push(ch);
        }
    }

    pub fn pop_char(&mut self) {
        self.input_text.pop();
    }

    pub fn clear(&mut self) {
        self.input_text.clear();
    }

    pub fn target_bytes(&self) -> Option<u64> {
        let num: u64 = self.input_text.trim().parse().ok()?;
        if num == 0 {
            return None;
        }
        match self.unit {
            SizeUnit::Mb => Some(num * 1024 * 1024),
            SizeUnit::Kb => Some(num * 1024),
        }
    }

    pub fn card_bounds(&self, center_x: f32, center_y: f32) -> (f32, f32, f32, f32) {
        let w = 260.0 * self.dpi_scale;
        let h = 216.0 * self.dpi_scale;
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

    pub fn hit_test_unit(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> Option<SizeUnit> {
        let (_, top, _, _) = self.card_bounds(cx, cy);
        let px = cx + rel_x;
        let py = cy + rel_y;

        let tab_w = 140.0 * self.dpi_scale;
        let tab_h = 30.0 * self.dpi_scale;
        let tab_left = cx - tab_w / 2.0;
        let tabs_y = top + 64.0 * self.dpi_scale;

        if px >= tab_left && px <= tab_left + tab_w && py >= tabs_y && py <= tabs_y + tab_h {
            if px < cx {
                Some(SizeUnit::Mb)
            } else {
                Some(SizeUnit::Kb)
            }
        } else {
            None
        }
    }

    pub fn hit_test_submit(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (_, t, _, _) = self.card_bounds(cx, cy);
        let btn_h = 40.0 * self.dpi_scale;
        let btn_w = 216.0 * self.dpi_scale;
        let btn_x = cx - btn_w / 2.0;
        let btn_y = t + 154.0 * self.dpi_scale;
        let px = cx + rel_x;
        let py = cy + rel_y;

        px >= btn_x && px <= btn_x + btn_w && py >= btn_y && py <= btn_y + btn_h
    }
}

pub fn render_reduce_size_card(
    state: &ReduceSizeCardState,
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

    let base_w = 260.0 * dpi;
    let base_h = 216.0 * dpi;
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
        "Reduce Size",
        left + 22.0 * dpi * scale,
        top + 24.0 * dpi * scale,
        18.5 * dpi * scale,
        theme.text_primary,
        true,
    );

    let sub_text = format!("Target file size for {} item(s)", state.file_count);
    fonts.draw_text_left(
        pixmap,
        &sub_text,
        left + 22.0 * dpi * scale,
        top + 44.0 * dpi * scale,
        13.0 * dpi * scale,
        theme.text_secondary,
        false,
    );

    // 5. Segmented Mode Switcher Tabs [ MB | KB ] (Centered, matching LOCAL | INTERNET)
    let tab_w = 140.0 * dpi * scale;
    let tab_h = 30.0 * dpi * scale;
    let tab_left = cx - tab_w / 2.0;
    let tabs_y = top + 64.0 * dpi * scale;

    let active_idx = match state.unit {
        SizeUnit::Mb => 0,
        SizeUnit::Kb => 1,
    };
    let hovered_idx = state.hovered_unit.map(|u| match u {
        SizeUnit::Mb => 0,
        SizeUnit::Kb => 1,
    });

    draw_segmented_tabs(
        pixmap,
        tab_left,
        tabs_y,
        tab_w,
        tab_h,
        &["MB", "KB"],
        active_idx,
        hovered_idx,
        scale,
        dpi,
        theme,
        fonts,
    );

    // 6. Number-only Input Field (Height 40px, 8.0px rounded corners, 10px gap from tabs)
    let input_y = top + 104.0 * dpi * scale;
    let input_w = 216.0 * dpi * scale;
    let input_h = 40.0 * dpi * scale;
    let input_x = cx - input_w / 2.0;
    let in_corner_r = 8.0 * dpi * scale;

    draw_input_box(pixmap, input_x, input_y, input_w, input_h, in_corner_r, scale, dpi, theme);

    let input_center_y = input_y + input_h / 2.0;

    // Centered input text / placeholder
    if state.input_text.is_empty() {
        let placeholder = match state.unit {
            SizeUnit::Mb => "e.g. 2 (MB)",
            SizeUnit::Kb => "e.g. 500 (KB)",
        };
        fonts.draw_text_centered(
            pixmap,
            placeholder,
            cx,
            input_center_y,
            14.5 * dpi * scale,
            theme.text_secondary,
            false,
        );
    } else {
        fonts.draw_text_centered(
            pixmap,
            &state.input_text,
            cx,
            input_center_y,
            16.0 * dpi * scale,
            theme.text_primary,
            true,
        );

        // Blinking cursor positioned after centered text (crisp 1.2px caret)
        if is_cursor_visible() {
            let text_w = fonts.measure_text(&state.input_text, 16.0 * dpi * scale, true);
            let cursor_x = cx + text_w / 2.0 + 2.0 * dpi * scale;
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
    }

    // 7. Action Button [ Reduce Size ] (Height 40px, 10px gap from input)
    let btn_y = top + 154.0 * dpi * scale;
    let btn_w = 216.0 * dpi * scale;
    let btn_h = 40.0 * dpi * scale;
    let btn_x = cx - btn_w / 2.0;
    let btn_corner_r = 8.0 * dpi * scale;
    let has_valid_number = state.target_bytes().is_some();

    draw_action_button(
        pixmap,
        btn_x,
        btn_y,
        btn_w,
        btn_h,
        "Reduce Size",
        has_valid_number,
        state.is_submit_hovered,
        false,
        btn_corner_r,
        scale,
        dpi,
        theme,
        fonts,
    );
}
