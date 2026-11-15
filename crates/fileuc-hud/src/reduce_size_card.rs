//! High-DPI Reduce Size Card.
//! Renders a compact modal popup (260px width) with number-only input and
//! segmented [MB | KB] tabs to specify a target compression size limit.
//! Powered by shared `card_common` primitives for 100% visual consistency.

use crate::card_common::{
    draw_action_button, draw_card_body, draw_card_shadow, draw_close_button, draw_input_box,
    draw_segmented_tabs_ex, is_cursor_visible,
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
    pub total_size_bytes: u64,
    pub pop_in: Spring,
    pub is_close_hovered: bool,
    pub is_submit_hovered: bool,
    pub hovered_unit: Option<SizeUnit>,
    pub cursor_timer: f32,
    pub last_cursor_visible: bool,
    pub dpi_scale: f32,
}

impl ReduceSizeCardState {
    pub fn new(file_count: usize, total_size_bytes: u64, dpi_scale: f32) -> Self {
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        // If file is already in KB (< 1 MB), default to KB and disable MB
        let unit = if total_size_bytes > 0 && total_size_bytes < 1024 * 1024 {
            SizeUnit::Kb
        } else {
            SizeUnit::Mb
        };

        Self {
            input_text: String::new(),
            unit,
            file_count,
            total_size_bytes,
            pop_in,
            is_close_hovered: false,
            is_submit_hovered: false,
            hovered_unit: None,
            cursor_timer: 0.0,
            last_cursor_visible: true,
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
        }
    }

    /// MB tab is disabled if the source file is already under 1 MB.
    pub fn is_mb_disabled(&self) -> bool {
        self.total_size_bytes > 0 && self.total_size_bytes < 1024 * 1024
    }

    /// Sets the unit, ignoring MB if disabled and clearing any oversized input.
    pub fn set_unit(&mut self, unit: SizeUnit) {
        if unit == SizeUnit::Mb && self.is_mb_disabled() {
            return;
        }
        self.unit = unit;

        // If the current input number in the new unit exceeds the original file size, clear it
        if let Ok(num) = self.input_text.trim().parse::<u64>() {
            let bytes = match self.unit {
                SizeUnit::Mb => num.saturating_mul(1024 * 1024),
                SizeUnit::Kb => num.saturating_mul(1024),
            };
            if self.total_size_bytes > 0 && bytes >= self.total_size_bytes {
                self.input_text.clear();
            }
        }
    }

    pub fn push_char(&mut self, ch: char) {
        // Strictly allow only numbers and max length 8
        if !ch.is_ascii_digit() || self.input_text.len() >= 8 {
            return;
        }

        let tentative = format!("{}{}", self.input_text, ch);
        if let Ok(num) = tentative.parse::<u64>() {
            let tentative_bytes = match self.unit {
                SizeUnit::Mb => num.saturating_mul(1024 * 1024),
                SizeUnit::Kb => num.saturating_mul(1024),
            };

            // Strict rule: user cannot enter a number greater than or equal to current size
            if self.total_size_bytes > 0 && tentative_bytes >= self.total_size_bytes {
                return; // Keystroke rejected!
            }

            self.input_text = tentative;
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
        let bytes = match self.unit {
            SizeUnit::Mb => num.saturating_mul(1024 * 1024),
            SizeUnit::Kb => num.saturating_mul(1024),
        };
        if self.total_size_bytes > 0 && bytes >= self.total_size_bytes {
            return None;
        }
        Some(bytes)
    }

    pub fn is_valid_submission(&self) -> bool {
        match self.target_bytes() {
            Some(b) => b > 0 && (self.total_size_bytes == 0 || b < self.total_size_bytes),
            None => false,
        }
    }

    pub fn card_bounds(&self, center_x: f32, center_y: f32) -> (f32, f32, f32, f32) {
        let w = 260.0 * self.dpi_scale;
        let h = 236.0 * self.dpi_scale;
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
                if self.is_mb_disabled() {
                    None // MB is disabled!
                } else {
                    Some(SizeUnit::Mb)
                }
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
        let btn_y = t + 174.0 * self.dpi_scale;
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
    let base_h = 236.0 * dpi;
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

    // 5. Segmented Mode Switcher Tabs [ MB | KB ]
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

    let disabled_indices: &[usize] = if state.is_mb_disabled() { &[0] } else { &[] };

    draw_segmented_tabs_ex(
        pixmap,
        tab_left,
        tabs_y,
        tab_w,
        tab_h,
        &["MB", "KB"],
        active_idx,
        hovered_idx,
        disabled_indices,
        scale,
        dpi,
        theme,
        fonts,
    );

    // 6. Current File Size Display (Under Tabs for UX)
    let formatted_current = if state.total_size_bytes == 0 {
        "Current size: Unknown".to_string()
    } else if state.total_size_bytes < 1024 {
        format!("Current size: {} B", state.total_size_bytes)
    } else if state.total_size_bytes < 1024 * 1024 {
        let kb = state.total_size_bytes as f64 / 1024.0;
        if kb < 10.0 {
            format!("Current size: {:.1} KB", kb)
        } else {
            format!("Current size: {:.0} KB", kb)
        }
    } else {
        let mb = state.total_size_bytes as f64 / (1024.0 * 1024.0);
        format!("Current size: {:.1} MB", mb)
    };

    fonts.draw_text_centered(
        pixmap,
        &formatted_current,
        cx,
        top + 104.0 * dpi * scale,
        12.0 * dpi * scale,
        theme.text_secondary,
        false,
    );

    // 7. Number-only Input Field
    let input_y = top + 120.0 * dpi * scale;
    let input_w = 216.0 * dpi * scale;
    let input_h = 38.0 * dpi * scale;
    let input_x = cx - input_w / 2.0;
    let in_corner_r = 8.0 * dpi * scale;

    draw_input_box(pixmap, input_x, input_y, input_w, input_h, in_corner_r, scale, dpi, theme);

    let input_center_y = input_y + input_h / 2.0;

    // Centered input text / placeholder
    if state.input_text.is_empty() {
        let placeholder = match state.unit {
            SizeUnit::Mb => "e.g. 2 (MB)",
            SizeUnit::Kb => "e.g. 200 (KB)",
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

    // 8. Action Button [ Reduce Size ]
    let btn_y = top + 174.0 * dpi * scale;
    let btn_w = 216.0 * dpi * scale;
    let btn_h = 40.0 * dpi * scale;
    let btn_x = cx - btn_w / 2.0;
    let btn_corner_r = 8.0 * dpi * scale;
    let has_valid_number = state.is_valid_submission();

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
