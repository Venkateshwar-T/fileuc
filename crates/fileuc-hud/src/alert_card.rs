//! Modal Alert UI Card for validation warnings and critical user notifications.
//! Renders an animated modal dialog with spring pop-in physics, standard Win32 HUD styling,
//! clear messaging, and outside-click dismissal support.
//! Powered by shared `card_common` primitives for 100% visual parity across all FileUC popups.

use crate::card_common::{
    draw_action_button, draw_card_body, draw_card_shadow, draw_close_button,
};
use crate::font::FontContext;
use crate::spring::Spring;
use crate::theme::Theme;
use tiny_skia::{Color, Pixmap};

#[derive(Debug, Clone)]
pub struct AlertCardState {
    pub title: String,
    pub message: String,
    pub pop_in: Spring,
    pub is_close_hovered: bool,
    pub is_ok_hovered: bool,
    pub dpi_scale: f32,
}

impl AlertCardState {
    pub fn new(message: impl Into<String>, dpi_scale: f32) -> Self {
        Self::new_with_title("Cannot Merge PDF", message, dpi_scale)
    }

    pub fn new_with_title(title: impl Into<String>, message: impl Into<String>, dpi_scale: f32) -> Self {
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        Self {
            title: title.into(),
            message: message.into(),
            pop_in,
            is_close_hovered: false,
            is_ok_hovered: false,
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        self.pop_in.tick(dt)
    }

    pub fn card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let card_w = 340.0 * self.dpi_scale;
        let card_h = 168.0 * self.dpi_scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn scaled_card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let card_w = 340.0 * self.dpi_scale * scale;
        let card_h = 168.0 * self.dpi_scale * scale;
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

    pub fn hit_test_ok(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let px = cx + rel_x;
        let py = cy + rel_y;
        let (_, top, _, _) = self.scaled_card_bounds(cx, cy);
        let btn_w = 296.0 * self.dpi_scale * scale;
        let btn_h = 40.0 * self.dpi_scale * scale;
        let btn_left = cx - btn_w / 2.0;
        let btn_top = top + 104.0 * self.dpi_scale * scale;

        px >= btn_left && px <= (btn_left + btn_w) && py >= btn_top && py <= (btn_top + btn_h)
    }

    pub fn hit_test_card(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (left, top, w, h) = self.scaled_card_bounds(cx, cy);
        let px = cx + rel_x;
        let py = cy + rel_y;
        px >= left && px <= (left + w) && py >= top && py <= (top + h)
    }
}

pub fn render_alert_card(
    state: &AlertCardState,
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
    let base_h = 168.0 * dpi;
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

    // 4. Header Title
    fonts.draw_text_left(
        pixmap,
        &state.title,
        left + 22.0 * dpi * scale,
        top + 24.0 * dpi * scale,
        18.5 * dpi * scale,
        theme.text_primary,
        true,
    );

    // 5. Warning / Explanation Message
    let sub_y = top + 52.0 * dpi * scale;
    fonts.draw_text_left(
        pixmap,
        &state.message,
        left + 22.0 * dpi * scale,
        sub_y,
        13.5 * dpi * scale,
        theme.text_secondary,
        false,
    );

    // 6. Action Button ("OK" / "Got it")
    let btn_w = 296.0 * dpi * scale;
    let btn_h = 40.0 * dpi * scale;
    let btn_left = cx - btn_w / 2.0;
    let btn_top = top + 104.0 * dpi * scale;
    let btn_corner_r = 8.0 * dpi * scale;

    draw_action_button(
        pixmap,
        btn_left,
        btn_top,
        btn_w,
        btn_h,
        "Got it",
        true,
        state.is_ok_hovered,
        false,
        btn_corner_r,
        scale,
        dpi,
        theme,
        fonts,
    );
}
