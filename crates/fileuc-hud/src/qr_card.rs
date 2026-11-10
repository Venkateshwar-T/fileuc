//! High-DPI Rounded Rectangle QR Code Sharing Card.
//! Renders an elegant, zero-friction transition card when files are dropped on "Share (Mobile)".
//! Displays the file name, size, pixel-perfect high-contrast QR code, live transfer status, and close controls.

use crate::font::FontContext;
use crate::qr_encode::QrCode;
use crate::spring::Spring;
use crate::theme::Theme;
use std::time::Instant;
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

#[derive(Debug, Clone, PartialEq)]
pub enum QrShareStatus {
    WaitingForScan,
    Scanned,
    Sending,
    Success,
    Failed(String),
}

pub struct QrCardState {
    pub url: String,
    pub file_name: String,
    pub file_size_formatted: String,
    pub status: QrShareStatus,
    pub qr_matrix: Vec<Vec<bool>>,
    pub pop_in: Spring,
    pub badge_spring: Spring,
    pub success_time: Option<Instant>,
    pub session_start: Instant,
    pub scanned_time: Option<Instant>,
    pub is_close_hovered: bool,
    pub dpi_scale: f32,
}

impl QrCardState {
    pub fn new(
        url: &str,
        file_name: &str,
        file_size_bytes: u64,
        dpi_scale: f32,
    ) -> Result<Self, String> {
        let qr = QrCode::encode_text(url)?;
        // Exactly matches the donut appearing animation spring (stiffness 750.0, damping 36.0)
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        Ok(Self {
            url: url.to_string(),
            file_name: file_name.to_string(),
            file_size_formatted: format_file_size(file_size_bytes),
            status: QrShareStatus::WaitingForScan,
            qr_matrix: qr.modules,
            pop_in,
            badge_spring: Spring::new(0.0, 280.0, 24.0),
            success_time: None,
            session_start: Instant::now(),
            scanned_time: None,
            is_close_hovered: false,
            dpi_scale,
        })
    }

    pub fn set_scanned(&mut self) {
        if self.status == QrShareStatus::WaitingForScan {
            self.status = QrShareStatus::Scanned;
            self.scanned_time = Some(Instant::now());
        }
    }

    pub fn set_sending(&mut self) {
        if self.status != QrShareStatus::Sending && self.status != QrShareStatus::Success {
            self.status = QrShareStatus::Sending;
        }
    }

    pub fn set_success(&mut self) {
        if self.status != QrShareStatus::Success {
            self.status = QrShareStatus::Success;
            self.success_time = Some(Instant::now());
            self.badge_spring = Spring::new(0.0, 280.0, 24.0);
            self.badge_spring.set_target(1.0);
        }
    }

    pub fn set_failed(&mut self, reason: &str) {
        self.status = QrShareStatus::Failed(reason.to_string());
    }

    pub fn is_finished(&self) -> bool {
        if let Some(t) = self.success_time {
            t.elapsed().as_secs_f32() > 1.5
        } else {
            false
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        if self.pop_in.tick(dt) {
            animating = true;
        }
        if self.status == QrShareStatus::Success && self.badge_spring.tick(dt) {
            animating = true;
        }
        animating
    }

    pub fn card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let card_w = 270.0 * self.dpi_scale;
        let card_h = 320.0 * self.dpi_scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn hit_test_close(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (left, top, w, _) = self.card_bounds(cx, cy);
        let btn_cx = left + w - 22.0 * self.dpi_scale;
        let btn_cy = top + 22.0 * self.dpi_scale;
        let hit_radius = 18.0 * self.dpi_scale;
        let dx = (cx + rel_x) - btn_cx;
        let dy = (cy + rel_y) - btn_cy;
        (dx * dx + dy * dy) <= (hit_radius * hit_radius)
    }

    pub fn hit_test_card(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (left, top, w, h) = self.card_bounds(cx, cy);
        let px = cx + rel_x;
        let py = cy + rel_y;
        px >= left && px <= (left + w) && py >= top && py <= (top + h)
    }
}

pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn truncate_filename(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        name.to_string()
    } else {
        let truncated: String = name.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

pub fn render_qr_card(
    state: &QrCardState,
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

    let base_w = 270.0 * dpi;
    let base_h = 320.0 * dpi;
    let half_w = (base_w / 2.0) * scale;
    let half_h = (base_h / 2.0) * scale;
    let left = cx - half_w;
    let top = cy - half_h;
    let w = base_w * scale;
    let h = base_h * scale;
    let corner_r = 18.0 * dpi * scale;

    // 1. Soft Ambient Shadow (Layered for depth)
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

    // 3. Close Button ("✕") anchored proportionally to (cx, cy)
    let close_x = cx + (base_w / 2.0 - 22.0 * dpi) * scale;
    let close_y = cy - (base_h / 2.0 - 22.0 * dpi) * scale;

    // Hover background pill/circle behind close button
    if state.is_close_hovered {
        let hover_r = 13.0 * dpi * scale;
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

    let close_half = 4.8 * dpi * scale;
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

    // 4. Header: File Name & Size (Anchored proportionally to center)
    let short_name = truncate_filename(&state.file_name, 24);
    let title_y = cy - (base_h / 2.0 - 26.0 * dpi) * scale;
    fonts.draw_text_centered(
        pixmap,
        &short_name,
        cx,
        title_y,
        17.0 * dpi * scale,
        theme.text_primary,
        true,
    );

    let sub_y = cy - (base_h / 2.0 - 47.0 * dpi) * scale;
    let sub_text = format!("{} • Ready to Download", state.file_size_formatted);
    fonts.draw_text_centered(
        pixmap,
        &sub_text,
        cx,
        sub_y,
        13.0 * dpi * scale,
        theme.text_secondary,
        false,
    );

    // 5. QR Code High-Contrast Inset White Pad (Anchored to center)
    let pad_size = 176.0 * dpi * scale;
    let pad_left = cx - pad_size / 2.0;
    let pad_top = cy - (base_h / 2.0 - 64.0 * dpi) * scale;
    let pad_r = 8.0 * dpi * scale;

    let mut pad_pb = PathBuilder::new();
    push_rounded_rect(&mut pad_pb, pad_left, pad_top, pad_size, pad_size, pad_r);
    if let Some(pad_path) = pad_pb.finish() {
        let mut pad_paint = Paint::default();
        pad_paint.set_color_rgba8(255, 255, 255, 255); // Pure white for 100% optical contrast
        pad_paint.anti_alias = true;
        pixmap.fill_path(&pad_path, &pad_paint, FillRule::Winding, Transform::identity(), None);

        // Pad subtle border
        let mut pad_border = Paint::default();
        pad_border.set_color_rgba8(220, 220, 225, 255);
        pad_border.anti_alias = true;
        let pad_stroke = Stroke {
            width: 1.0 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&pad_path, &pad_border, &pad_stroke, Transform::identity(), None);
    }

    // 6. Draw QR Code Vector Modules in 100% PERFECT LINEAR SYNC with Pad
    // No .floor() quantization so modules scale continuously with the pad
    let n = state.qr_matrix.len();
    if n > 0 {
        let quiet_modules = 4.0;
        let total_modules = (n as f32) + quiet_modules * 2.0;
        let module_size = pad_size / total_modules;
        let pad_margin = quiet_modules * module_size;

        let mut qr_pb = PathBuilder::new();
        for r in 0..n {
            for c in 0..n {
                if state.qr_matrix[r][c] {
                    let mx = pad_left + pad_margin + (c as f32) * module_size;
                    let my = pad_top + pad_margin + (r as f32) * module_size;
                    // +0.1 subpixel overlap ensures continuous solid ink without seams
                    if let Some(rect) = Rect::from_xywh(mx, my, module_size + 0.1, module_size + 0.1) {
                        qr_pb.push_rect(rect);
                    }
                }
            }
        }

        if let Some(qr_path) = qr_pb.finish() {
            let mut qr_paint = Paint::default();
            qr_paint.set_color_rgba8(18, 18, 20, 255); // Deep Jet Black ink
            qr_paint.anti_alias = true; // Subpixel antialiasing for continuous smooth motion
            pixmap.fill_path(&qr_path, &qr_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 7. Footer / Live Status Feedback (Anchored proportionally to center)
    let status_y = cy + (264.0 * dpi - base_h / 2.0) * scale;
    let instruction_y = cy + (288.0 * dpi - base_h / 2.0) * scale;

    match &state.status {
        QrShareStatus::WaitingForScan => {
            fonts.draw_text_centered(
                pixmap,
                "Scan with Phone Camera",
                cx,
                status_y,
                15.5 * dpi * scale,
                theme.text_primary,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                "Connects via Local Wi-Fi",
                cx,
                instruction_y,
                13.0 * dpi * scale,
                theme.text_secondary,
                false,
            );
        }
        QrShareStatus::Scanned => {
            let scanned_color = theme.slice_hover_fill;
            fonts.draw_text_centered(
                pixmap,
                "Device Connected",
                cx,
                status_y,
                15.5 * dpi * scale,
                scanned_color,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                "Tap download on phone to begin",
                cx,
                instruction_y,
                13.0 * dpi * scale,
                theme.text_secondary,
                false,
            );
        }
        QrShareStatus::Sending => {
            let sending_color = theme.slice_hover_fill; // Exactly matches donut slice hover color
            fonts.draw_text_centered(
                pixmap,
                "Sending to Phone...",
                cx,
                status_y,
                15.5 * dpi * scale,
                sending_color,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                "Downloading file on device",
                cx,
                instruction_y,
                13.0 * dpi * scale,
                theme.text_secondary,
                false,
            );
        }
        QrShareStatus::Success => {
            let emerald = Color::from_rgba8(34, 197, 94, 255); // Vibrant Emerald Green
            let b_scale = state.badge_spring.value().max(0.0) * dpi * scale;

            // Emerald Checkmark Icon
            if b_scale > 0.01 {
                let mut check_pb = PathBuilder::new();
                let ch_y = status_y - 2.0 * dpi * scale;
                let p1 = (cx - 74.0 * dpi * scale, ch_y);
                let p2 = (cx - 68.0 * dpi * scale, ch_y + 5.0 * dpi * scale);
                let p3 = (cx - 58.0 * dpi * scale, ch_y - 6.0 * dpi * scale);
                check_pb.move_to(p1.0, p1.1);
                check_pb.line_to(p2.0, p2.1);
                check_pb.line_to(p3.0, p3.1);

                if let Some(check_path) = check_pb.finish() {
                    let mut check_paint = Paint::default();
                    check_paint.set_color(emerald);
                    check_paint.anti_alias = true;
                    let check_stroke = Stroke {
                        width: 2.6 * b_scale,
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&check_path, &check_paint, &check_stroke, Transform::identity(), None);
                }
            }

            fonts.draw_text_centered(
                pixmap,
                "Downloaded to Phone!",
                cx + 10.0 * dpi * scale,
                status_y,
                15.5 * dpi * scale,
                emerald,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                "Transfer Complete",
                cx,
                instruction_y,
                13.0 * dpi * scale,
                theme.text_secondary,
                false,
            );
        }
        QrShareStatus::Failed(err) => {
            let danger = theme.danger_text;
            fonts.draw_text_centered(
                pixmap,
                "Transfer Failed",
                cx,
                status_y,
                15.5 * dpi * scale,
                danger,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                err,
                cx,
                instruction_y,
                12.5 * dpi * scale,
                danger,
                false,
            );
        }
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
