//! High-DPI Instant QR Code Generator Card.
//! Renders an interactive modal card with real-time QR code generation,
//! spring pop-in physics, editable text/link/Wi-Fi input, and intelligent mode tabs.

use crate::card_common::{
    draw_action_button, draw_card_body, draw_card_shadow, draw_close_button, draw_input_box,
    draw_text_cursor, is_cursor_visible, push_rounded_rect,
};
use crate::font::FontContext;
use crate::qr_encode::QrCode;
use crate::spring::Spring;
use crate::theme::Theme;
use std::time::Instant;
use tiny_skia::{
    Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickQrTab {
    Link,
    Text,
    Wifi,
}

pub struct QuickQrCardState {
    pub input_text: String,
    pub tab: QuickQrTab,
    pub detected_ssid: Option<String>,
    pub qr_matrix: Option<Vec<Vec<bool>>>,
    pub pop_in: Spring,
    pub is_close_hovered: bool,
    pub is_save_hovered: bool,
    pub hovered_tab: Option<QuickQrTab>,
    pub saved_feedback_timer: Option<Instant>,
    pub last_cursor_visible: bool,
    pub dpi_scale: f32,
}

impl QuickQrCardState {
    pub fn new(dpi_scale: f32) -> Self {
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        Self {
            input_text: String::new(),
            tab: QuickQrTab::Link,
            detected_ssid: None,
            qr_matrix: None,
            pop_in,
            is_close_hovered: false,
            is_save_hovered: false,
            hovered_tab: None,
            saved_feedback_timer: None,
            last_cursor_visible: true,
            dpi_scale,
        }
    }

    pub fn is_url_string(s: &str) -> bool {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return false;
        }
        let lower = trimmed.to_lowercase();
        lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("www.")
            || (lower.contains('.') && !lower.contains(' ') && (
                lower.ends_with(".com") || lower.ends_with(".org") || lower.ends_with(".net")
                || lower.ends_with(".io") || lower.ends_with(".app") || lower.ends_with(".dev")
                || lower.ends_with(".in") || lower.ends_with(".co") || lower.ends_with(".ai")
                || lower.ends_with(".me") || lower.ends_with(".xyz") || lower.ends_with(".edu")
                || lower.ends_with(".gov")
            ))
    }

    pub fn is_tab_enabled(&self, t: QuickQrTab) -> bool {
        let trimmed = self.input_text.trim();
        if trimmed.is_empty() {
            return true;
        }
        if Self::is_url_string(trimmed) {
            match t {
                QuickQrTab::Link | QuickQrTab::Text => true,
                QuickQrTab::Wifi => false,
            }
        } else {
            // Normal non-link text
            if self.tab == QuickQrTab::Wifi {
                t == QuickQrTab::Wifi
            } else {
                t == QuickQrTab::Text
            }
        }
    }

    pub fn on_text_changed(&mut self) {
        let trimmed = self.input_text.trim();
        if trimmed.is_empty() {
            // If empty and was in Link or Text, keep Link. If was in Wifi, stay in Wifi.
            if self.tab != QuickQrTab::Wifi {
                self.tab = QuickQrTab::Link;
            }
        } else if Self::is_url_string(trimmed) {
            // URL added: auto-switch to Link tab if on Wifi
            if self.tab == QuickQrTab::Wifi {
                self.tab = QuickQrTab::Link;
            }
        } else {
            // Normal non-link text: auto-switch to Text tab unless user is specifically on Wifi
            if self.tab != QuickQrTab::Wifi {
                self.tab = QuickQrTab::Text;
            }
        }
        self.update_qr();
    }

    pub fn switch_tab(&mut self, target: QuickQrTab) {
        if self.is_tab_enabled(target) {
            self.tab = target;
            self.update_qr();
        }
    }

    pub fn push_char(&mut self, c: char) {
        if self.input_text.len() < 1024 && !c.is_control() {
            self.input_text.push(c);
            self.on_text_changed();
        }
    }

    pub fn pop_char(&mut self) {
        self.input_text.pop();
        self.on_text_changed();
    }

    pub fn paste_text(&mut self, text: &str) {
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.input_text.len() < 1024 {
                self.input_text.push(c);
            }
        }
        self.on_text_changed();
    }

    pub fn clear(&mut self) {
        self.input_text.clear();
        self.on_text_changed();
    }

    pub fn update_qr(&mut self) {
        let trimmed = self.input_text.trim();
        if trimmed.is_empty() {
            self.qr_matrix = None;
            return;
        }

        let payload = match self.tab {
            QuickQrTab::Link => {
                if trimmed.starts_with("www.") {
                    format!("https://{}", trimmed)
                } else {
                    trimmed.to_string()
                }
            }
            QuickQrTab::Text => trimmed.to_string(),
            QuickQrTab::Wifi => {
                let ssid = self.detected_ssid.as_deref().unwrap_or("Wi-Fi Network");
                let escaped_ssid = escape_wifi_field(ssid);
                let escaped_pass = escape_wifi_field(trimmed);
                format!("WIFI:S:{};T:WPA;P:{};;", escaped_ssid, escaped_pass)
            }
        };

        match QrCode::encode_text(&payload) {
            Ok(qr) => self.qr_matrix = Some(qr.modules),
            Err(_) => self.qr_matrix = None,
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let animating = self.pop_in.tick(dt);
        let cur_visible = is_cursor_visible();
        let blink_changed = cur_visible != self.last_cursor_visible;
        if blink_changed {
            self.last_cursor_visible = cur_visible;
        }

        let mut feedback_active = false;
        if let Some(t) = self.saved_feedback_timer {
            if t.elapsed().as_secs_f32() < 1.8 {
                feedback_active = true;
            } else {
                self.saved_feedback_timer = None;
                feedback_active = true;
            }
        }

        animating || blink_changed || feedback_active
    }

    pub fn card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let card_w = 328.0 * self.dpi_scale;
        let card_h = 418.0 * self.dpi_scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn scaled_card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let card_w = 328.0 * self.dpi_scale * scale;
        let card_h = 418.0 * self.dpi_scale * scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn hit_test_card(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (l, t, w, h) = self.scaled_card_bounds(cx, cy);
        let px = cx + rel_x;
        let py = cy + rel_y;
        px >= l && px <= l + w && py >= t && py <= t + h
    }

    pub fn hit_test_close(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let (l, t, w, _) = self.scaled_card_bounds(cx, cy);
        let close_x = l + w - 24.0 * self.dpi_scale * scale;
        let close_y = t + 24.0 * self.dpi_scale * scale;
        let px = cx + rel_x;
        let py = cy + rel_y;
        let dx = px - close_x;
        let dy = py - close_y;
        (dx * dx + dy * dy).sqrt() <= 20.0 * self.dpi_scale * scale
    }

    pub fn hit_test_save(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let (_, t, _, _) = self.scaled_card_bounds(cx, cy);
        let btn_w = 288.0 * self.dpi_scale * scale;
        let btn_h = 42.0 * self.dpi_scale * scale;
        let btn_left = cx - btn_w / 2.0;
        let btn_top = t + 360.0 * self.dpi_scale * scale;

        let px = cx + rel_x;
        let py = cy + rel_y;

        px >= btn_left && px <= btn_left + btn_w && py >= btn_top && py <= btn_top + btn_h
    }

    pub fn hit_test_tab(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> Option<QuickQrTab> {
        let scale = if self.pop_in.value() > 0.0 { self.pop_in.value() } else { 1.0 };
        let (_, t, _, _) = self.scaled_card_bounds(cx, cy);
        let tab_w = 240.0 * self.dpi_scale * scale;
        let tab_h = 30.0 * self.dpi_scale * scale;
        let tab_left = cx - tab_w / 2.0;
        let tab_top = t + 50.0 * self.dpi_scale * scale;

        let px = cx + rel_x;
        let py = cy + rel_y;

        if px >= tab_left && px <= tab_left + tab_w && py >= tab_top && py <= tab_top + tab_h {
            let seg_w = tab_w / 3.0;
            let idx = ((px - tab_left) / seg_w) as usize;
            let tab = match idx {
                0 => QuickQrTab::Link,
                1 => QuickQrTab::Text,
                _ => QuickQrTab::Wifi,
            };
            return Some(tab);
        }
        None
    }
}

fn escape_wifi_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        if matches!(ch, '\\' | ';' | ',' | ':' | '"') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

pub fn render_quick_qr_card(
    state: &QuickQrCardState,
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

    let base_w = 328.0 * dpi;
    let base_h = 418.0 * dpi;
    let half_w = (base_w / 2.0) * scale;
    let half_h = (base_h / 2.0) * scale;
    let left = cx - half_w;
    let top = cy - half_h;
    let w = base_w * scale;
    let h = base_h * scale;
    let corner_r = 20.0 * dpi * scale;

    // 1. Soft Ambient Shadow (using unified card_common)
    draw_card_shadow(pixmap, left, top, w, h, corner_r, scale, dpi);

    // 2. Rounded Rectangle Solid Card Body
    draw_card_body(pixmap, left, top, w, h, corner_r, scale, dpi, theme);

    // 3. Close Button ("✕")
    let close_x = left + w - 24.0 * dpi * scale;
    let close_y = top + 24.0 * dpi * scale;
    draw_close_button(pixmap, close_x, close_y, state.is_close_hovered, scale, dpi, theme);

    // 4. Header: Prominent Title (19.0px font size)
    let title_y = top + 27.0 * dpi * scale;
    fonts.draw_text_centered(
        pixmap,
        "Instant QR Code",
        cx,
        title_y,
        19.0 * dpi * scale,
        theme.text_primary,
        true,
    );

    // 5. Segmented Tabs: [ LINK | TEXT | WI-FI ] (14.0px font size)
    let tab_w = 240.0 * dpi * scale;
    let tab_h = 30.0 * dpi * scale;
    let tab_left = cx - tab_w / 2.0;
    let tab_top = top + 50.0 * dpi * scale;
    let seg_w = tab_w / 3.0;

    // Segmented container background
    let mut tab_container_pb = PathBuilder::new();
    push_rounded_rect(&mut tab_container_pb, tab_left, tab_top, tab_w, tab_h, 8.0 * dpi * scale);
    if let Some(container_path) = tab_container_pb.finish() {
        let mut bg_paint = Paint::default();
        if theme.is_dark {
            bg_paint.set_color_rgba8(36, 36, 40, 255);
        } else {
            bg_paint.set_color_rgba8(238, 239, 242, 255);
        }
        bg_paint.anti_alias = true;
        pixmap.fill_path(&container_path, &bg_paint, FillRule::Winding, Transform::identity(), None);

        let mut border_paint = Paint::default();
        border_paint.set_color(theme.border_subtle);
        border_paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.0 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&container_path, &border_paint, &stroke, Transform::identity(), None);
    }

    let tabs = [
        (QuickQrTab::Link, "LINK"),
        (QuickQrTab::Text, "TEXT"),
        (QuickQrTab::Wifi, "WI-FI"),
    ];

    for (i, (t, label)) in tabs.iter().enumerate() {
        let seg_left = tab_left + (i as f32) * seg_w;
        let is_active = state.tab == *t;
        let is_enabled = state.is_tab_enabled(*t);
        let is_hovered = state.hovered_tab == Some(*t) && is_enabled && !is_active;

        if is_active {
            let mut active_pb = PathBuilder::new();
            push_rounded_rect(
                &mut active_pb,
                seg_left + 2.0 * dpi * scale,
                tab_top + 2.0 * dpi * scale,
                seg_w - 4.0 * dpi * scale,
                tab_h - 4.0 * dpi * scale,
                6.5 * dpi * scale,
            );
            if let Some(active_path) = active_pb.finish() {
                let mut active_paint = Paint::default();
                active_paint.set_color(theme.slice_hover_fill); // Active brand accent
                active_paint.anti_alias = true;
                pixmap.fill_path(&active_path, &active_paint, FillRule::Winding, Transform::identity(), None);
            }
        } else if is_hovered {
            let mut hov_pb = PathBuilder::new();
            push_rounded_rect(
                &mut hov_pb,
                seg_left + 2.0 * dpi * scale,
                tab_top + 2.0 * dpi * scale,
                seg_w - 4.0 * dpi * scale,
                tab_h - 4.0 * dpi * scale,
                6.5 * dpi * scale,
            );
            if let Some(hov_path) = hov_pb.finish() {
                let mut hov_paint = Paint::default();
                if theme.is_dark {
                    hov_paint.set_color_rgba8(255, 255, 255, 14);
                } else {
                    hov_paint.set_color_rgba8(0, 0, 0, 10);
                }
                hov_paint.anti_alias = true;
                pixmap.fill_path(&hov_path, &hov_paint, FillRule::Winding, Transform::identity(), None);
            }
        }

        let text_color = if is_active {
            Color::from_rgba8(255, 255, 255, 255)
        } else if !is_enabled {
            if theme.is_dark {
                Color::from_rgba8(110, 110, 120, 120)
            } else {
                Color::from_rgba8(170, 170, 180, 140)
            }
        } else {
            theme.text_secondary
        };

        fonts.draw_text_centered(
            pixmap,
            label,
            seg_left + seg_w / 2.0,
            tab_top + tab_h / 2.0,
            14.5 * dpi * scale,
            text_color,
            is_active,
        );
    }

    // 6. Subtitle / Wi-Fi Network Name (14.0px font size)
    let sub_y = top + 94.0 * dpi * scale;
    if state.tab == QuickQrTab::Wifi {
        let ssid = state.detected_ssid.as_deref().unwrap_or("Detecting Wi-Fi...");
        let wifi_text = format!("Network: {}", ssid);
        fonts.draw_text_centered(
            pixmap,
            &wifi_text,
            cx,
            sub_y,
            14.0 * dpi * scale,
            theme.text_secondary,
            false,
        );
    } else if state.tab == QuickQrTab::Link {
        fonts.draw_text_centered(
            pixmap,
            "Open website instantly on phone",
            cx,
            sub_y,
            14.0 * dpi * scale,
            theme.text_secondary,
            false,
        );
    } else {
        fonts.draw_text_centered(
            pixmap,
            "Copy text or message to phone",
            cx,
            sub_y,
            14.0 * dpi * scale,
            theme.text_secondary,
            false,
        );
    }

    // 7. QR Code High-Contrast Inset White Pad
    let pad_size = 176.0 * dpi * scale;
    let pad_left = cx - pad_size / 2.0;
    let pad_top = top + 114.0 * dpi * scale;
    let pad_r = 9.0 * dpi * scale;

    let mut pad_pb = PathBuilder::new();
    push_rounded_rect(&mut pad_pb, pad_left, pad_top, pad_size, pad_size, pad_r);
    if let Some(pad_path) = pad_pb.finish() {
        let mut pad_paint = Paint::default();
        pad_paint.set_color_rgba8(255, 255, 255, 255); // Pure white for 100% optical contrast
        pad_paint.anti_alias = true;
        pixmap.fill_path(&pad_path, &pad_paint, FillRule::Winding, Transform::identity(), None);

        let mut pad_border = Paint::default();
        pad_border.set_color_rgba8(215, 215, 222, 255);
        pad_border.anti_alias = true;
        let pad_stroke = Stroke {
            width: 1.0 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&pad_path, &pad_border, &pad_stroke, Transform::identity(), None);
    }

    // 8. Draw QR Modules or Placeholder
    if let Some(ref matrix) = state.qr_matrix {
        let n = matrix.len();
        if n > 0 {
            let quiet_modules = 4.0;
            let total_modules = (n as f32) + quiet_modules * 2.0;
            let module_size = pad_size / total_modules;
            let pad_margin = quiet_modules * module_size;

            let mut qr_pb = PathBuilder::new();
            for r in 0..n {
                for c in 0..n {
                    if matrix[r][c] {
                        let mx = pad_left + pad_margin + (c as f32) * module_size;
                        let my = pad_top + pad_margin + (r as f32) * module_size;
                        if let Some(rect) = Rect::from_xywh(mx, my, module_size + 0.1, module_size + 0.1) {
                            qr_pb.push_rect(rect);
                        }
                    }
                }
            }

            if let Some(qr_path) = qr_pb.finish() {
                let mut qr_paint = Paint::default();
                qr_paint.set_color_rgba8(16, 16, 18, 255); // Deep Jet Black ink
                qr_paint.anti_alias = true;
                pixmap.fill_path(&qr_path, &qr_paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    } else {
        // Large, clear placeholder when input is empty
        let hint_center_y = pad_top + pad_size / 2.0;
        let hint_color_1 = Color::from_rgba8(100, 100, 115, 255);
        let hint_color_2 = Color::from_rgba8(140, 140, 155, 255);

        let box_w = 130.0 * dpi * scale;
        let box_h = 130.0 * dpi * scale;
        let box_left = cx - box_w / 2.0;
        let box_top = pad_top + (pad_size - box_h) / 2.0;
        let mut box_pb = PathBuilder::new();
        push_rounded_rect(&mut box_pb, box_left, box_top, box_w, box_h, 7.0 * dpi * scale);
        if let Some(box_path) = box_pb.finish() {
            let mut box_paint = Paint::default();
            box_paint.set_color_rgba8(232, 233, 238, 255);
            box_paint.anti_alias = true;
            let stroke = Stroke {
                width: 1.2 * dpi * scale,
                ..Default::default()
            };
            pixmap.stroke_path(&box_path, &box_paint, &stroke, Transform::identity(), None);
        }

        fonts.draw_text_centered(
            pixmap,
            "Type below",
            cx,
            hint_center_y - 10.0 * dpi * scale,
            16.0 * dpi * scale,
            hint_color_1,
            true,
        );
        fonts.draw_text_centered(
            pixmap,
            "Generates live QR",
            cx,
            hint_center_y + 14.0 * dpi * scale,
            13.5 * dpi * scale,
            hint_color_2,
            false,
        );
    }

    // 9. Interactive Input Box (Full width, no paste button clutter)
    let input_w = 288.0 * dpi * scale;
    let input_h = 42.0 * dpi * scale;
    let input_left = cx - input_w / 2.0;
    let input_top = top + 312.0 * dpi * scale;
    let input_r = 8.0 * dpi * scale;

    draw_input_box(pixmap, input_left, input_top, input_w, input_h, input_r, scale, dpi, theme);

    // 10. Large, Prominent Input Text & Blinking Cursor (16.0px font size)
    let text_start_x = input_left + 14.0 * dpi * scale;
    let input_center_y = input_top + input_h / 2.0;
    let font_size = 16.0 * dpi * scale;
    let max_text_w = input_w - 28.0 * dpi * scale;

    let cursor_x = if state.input_text.is_empty() {
        let placeholder = match state.tab {
            QuickQrTab::Link => "Paste or type URL (e.g. https://...)",
            QuickQrTab::Text => "Type message or notes...",
            QuickQrTab::Wifi => "Enter Wi-Fi password...",
        };
        fonts.draw_text_left(
            pixmap,
            placeholder,
            text_start_x,
            input_center_y,
            font_size,
            theme.text_secondary,
            false,
        );
        text_start_x
    } else {
        let full_w = fonts.measure_text(&state.input_text, font_size, true);
        if full_w <= max_text_w {
            let rendered_w = fonts.draw_text_left(
                pixmap,
                &state.input_text,
                text_start_x,
                input_center_y,
                font_size,
                theme.text_primary,
                true,
            );
            text_start_x + rendered_w
        } else {
            // Find suffix that fits in max_text_w - ellipsis margin using binary search without allocations
            let target_w = max_text_w - 18.0 * dpi * scale;
            let char_indices: Vec<usize> = state.input_text.char_indices().map(|(i, _)| i).collect();
            let total = char_indices.len();
            let mut low = 0;
            let mut high = total;
            let mut best_start_byte = 0;

            while low < high {
                let mid = (low + high) / 2;
                let byte_idx = char_indices[mid];
                let suffix = &state.input_text[byte_idx..];
                if fonts.measure_text(suffix, font_size, true) <= target_w {
                    best_start_byte = byte_idx;
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }

            let visible_slice = format!("…{}", &state.input_text[best_start_byte..]);
            let rendered_w = fonts.draw_text_left(
                pixmap,
                &visible_slice,
                text_start_x,
                input_center_y,
                font_size,
                theme.text_primary,
                true,
            );
            text_start_x + rendered_w
        }
    };

    // Blinking Cursor Line (crisp 1.2px caret)
    draw_text_cursor(
        pixmap,
        cursor_x + 1.0 * dpi * scale,
        input_center_y,
        18.0 * dpi * scale,
        scale,
        dpi,
        theme.text_primary,
    );

    // 11. "Save to Desktop" Action Button
    let btn_w = 288.0 * dpi * scale;
    let btn_h = 42.0 * dpi * scale;
    let btn_left = cx - btn_w / 2.0;
    let btn_top = top + 360.0 * dpi * scale;
    let btn_corner_r = 9.0 * dpi * scale;

    let is_enabled = state.qr_matrix.is_some();
    let is_feedback = state.saved_feedback_timer.map(|t| t.elapsed().as_secs_f32() < 1.8).unwrap_or(false);
    let btn_text = if is_feedback { "Saved to Desktop!" } else { "Save to Desktop" };

    draw_action_button(
        pixmap,
        btn_left,
        btn_top,
        btn_w,
        btn_h,
        btn_text,
        is_enabled,
        state.is_save_hovered,
        is_feedback,
        btn_corner_r,
        scale,
        dpi,
        theme,
        fonts,
    );
}
