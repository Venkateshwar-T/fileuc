//! High-DPI Rounded Rectangle QR Code Sharing Card.
//! Renders an elegant, zero-friction transition card when files are dropped on "Share (Mobile)".
//! Supports switchable [LOCAL | INTERNET] tabs with automatic secure internet tunneling.

use crate::card_common::{draw_card_body, draw_card_shadow, draw_close_button, draw_segmented_tabs, push_rounded_rect};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrShareTab {
    Local,
    Internet,
}

pub struct QrCardState {
    pub local_url: String,
    pub internet_url: Option<String>,
    pub tab: QrShareTab,
    pub hovered_tab: Option<QrShareTab>,
    pub is_connecting_tunnel: bool,
    pub tunnel_error: Option<String>,
    pub is_cloudflared_installed: bool,
    pub is_downloading: bool,
    pub download_pct: Option<u8>,
    pub is_download_btn_hovered: bool,
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
        local_url: &str,
        file_name: &str,
        file_size_bytes: u64,
        dpi_scale: f32,
    ) -> Result<Self, String> {
        let qr = QrCode::encode_text(local_url)?;
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        Ok(Self {
            local_url: local_url.to_string(),
            internet_url: None,
            tab: QrShareTab::Local,
            hovered_tab: None,
            is_connecting_tunnel: false,
            tunnel_error: None,
            is_cloudflared_installed: true,
            is_downloading: false,
            download_pct: None,
            is_download_btn_hovered: false,
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
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
        })
    }

    pub fn switch_tab(&mut self, tab: QrShareTab) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        match tab {
            QrShareTab::Local => {
                if let Ok(qr) = QrCode::encode_text(&self.local_url) {
                    self.qr_matrix = qr.modules;
                }
            }
            QrShareTab::Internet => {
                if !self.is_cloudflared_installed {
                    self.is_connecting_tunnel = false;
                    self.tunnel_error = None;
                } else if let Some(ref url) = self.internet_url {
                    if let Ok(qr) = QrCode::encode_text(url) {
                        self.qr_matrix = qr.modules;
                    }
                } else {
                    self.is_connecting_tunnel = true;
                    self.tunnel_error = None;
                }
            }
        }
    }

    pub fn set_internet_url(&mut self, url: &str) {
        self.internet_url = Some(url.to_string());
        self.is_connecting_tunnel = false;
        self.tunnel_error = None;
        if self.tab == QrShareTab::Internet {
            if let Ok(qr) = QrCode::encode_text(url) {
                self.qr_matrix = qr.modules;
            }
        }
    }

    pub fn set_tunnel_error(&mut self, err: &str) {
        self.tunnel_error = Some(err.to_string());
        self.is_connecting_tunnel = false;
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
        if self.is_connecting_tunnel {
            animating = true;
        }
        animating
    }

    pub fn card_bounds(&self, cx: f32, cy: f32) -> (f32, f32, f32, f32) {
        let card_w = 290.0 * self.dpi_scale;
        let card_h = 356.0 * self.dpi_scale;
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        (left, top, card_w, card_h)
    }

    pub fn hit_test_close(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        let (left, top, w, _) = self.card_bounds(cx, cy);
        let btn_cx = left + w - 24.0 * self.dpi_scale;
        let btn_cy = top + 24.0 * self.dpi_scale;
        let hit_radius = 18.0 * self.dpi_scale;
        let dx = (cx + rel_x) - btn_cx;
        let dy = (cy + rel_y) - btn_cy;
        (dx * dx + dy * dy) <= (hit_radius * hit_radius)
    }

    pub fn hit_test_tab(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> Option<QrShareTab> {
        let scale = self.pop_in.value();
        let (_, top, _, _) = self.card_bounds(cx, cy);
        let px = cx + rel_x;
        let py = cy + rel_y;

        let tab_w = 210.0 * self.dpi_scale * scale;
        let tab_h = 32.0 * self.dpi_scale * scale;
        let tab_left = cx - tab_w / 2.0;
        let tabs_y = top + 64.0 * self.dpi_scale * scale;

        if px >= tab_left && px <= tab_left + tab_w && py >= tabs_y && py <= tabs_y + tab_h {
            let seg_w = tab_w / 2.0;
            if px < tab_left + seg_w {
                Some(QrShareTab::Local)
            } else {
                Some(QrShareTab::Internet)
            }
        } else {
            None
        }
    }

    pub fn hit_test_download_btn(&self, rel_x: f32, rel_y: f32, cx: f32, cy: f32) -> bool {
        if self.tab != QrShareTab::Internet || self.is_cloudflared_installed || self.is_downloading {
            return false;
        }
        let scale = self.pop_in.value();
        let dpi = self.dpi_scale;
        let (_, top, _, _) = self.card_bounds(cx, cy);
        let tab_h = 32.0 * dpi * scale;
        let tabs_y = top + 64.0 * dpi * scale;
        let tabs_bottom = tabs_y + tab_h;

        let btn_w = 256.0 * dpi * scale;
        let btn_h = 40.0 * dpi * scale;
        let btn_left = cx - btn_w / 2.0;
        let btn_top = tabs_bottom + 98.0 * dpi * scale;

        let px = cx + rel_x;
        let py = cy + rel_y;
        px >= btn_left && px <= btn_left + btn_w && py >= btn_top && py <= btn_top + btn_h
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
        format!("{:.1} KB", bytes as f32 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f32 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f32 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn truncate_filename(name: &str, max_chars: usize) -> String {
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

    let base_w = 290.0 * dpi;
    let base_h = 356.0 * dpi;
    let half_w = (base_w / 2.0) * scale;
    let half_h = (base_h / 2.0) * scale;
    let left = cx - half_w;
    let top = cy - half_h;
    let w = base_w * scale;
    let h = base_h * scale;
    let corner_r = 18.0 * dpi * scale;

    // 1. Soft Ambient Shadow (using unified card_common)
    draw_card_shadow(pixmap, left, top, w, h, corner_r, scale, dpi);

    // 2. Rounded Rectangle Solid Card Body
    draw_card_body(pixmap, left, top, w, h, corner_r, scale, dpi, theme);

    // 3. Close Button ("✕")
    let close_x = cx + (base_w / 2.0 - 24.0 * dpi) * scale;
    let close_y = cy - (base_h / 2.0 - 24.0 * dpi) * scale;
    draw_close_button(pixmap, close_x, close_y, state.is_close_hovered, scale, dpi, theme);

    // 4. Header: File Name & Size
    let short_name = truncate_filename(&state.file_name, 22);
    let title_y = cy - (base_h / 2.0 - 24.0 * dpi) * scale;
    fonts.draw_text_centered(
        pixmap,
        &short_name,
        cx,
        title_y,
        18.5 * dpi * scale,
        theme.text_primary,
        true,
    );

    let sub_y = cy - (base_h / 2.0 - 45.0 * dpi) * scale;
    let sub_text = format!("{} • Ready to Download", state.file_size_formatted);
    fonts.draw_text_centered(
        pixmap,
        &sub_text,
        cx,
        sub_y,
        13.5 * dpi * scale,
        theme.text_secondary,
        false,
    );

    // 5. Segmented Tabs: [ LOCAL | INTERNET ] (Height 32px, 8.0px rounded container)
    let tab_w = 210.0 * dpi * scale;
    let tab_h = 32.0 * dpi * scale;
    let tab_left = cx - tab_w / 2.0;
    let tabs_y = cy - (base_h / 2.0 - 64.0 * dpi) * scale;

    let active_idx = match state.tab {
        QrShareTab::Local => 0,
        QrShareTab::Internet => 1,
    };
    let hovered_idx = state.hovered_tab.map(|t| match t {
        QrShareTab::Local => 0,
        QrShareTab::Internet => 1,
    });
    draw_segmented_tabs(
        pixmap,
        tab_left,
        tabs_y,
        tab_w,
        tab_h,
        &["LOCAL", "INTERNET"],
        active_idx,
        hovered_idx,
        scale,
        dpi,
        theme,
        fonts,
    );

    // 6. QR Code High-Contrast Inset White Pad (only when QR or tunnel messages are active)
    let show_qr_pad = state.tab == QrShareTab::Local
        || (state.tab == QrShareTab::Internet && state.is_cloudflared_installed);
    let pad_size = 176.0 * dpi * scale;
    let pad_left = cx - pad_size / 2.0;
    let pad_top = tabs_y + tab_h + 10.0 * dpi * scale;
    let pad_r = 8.0 * dpi * scale;

    if show_qr_pad {
        let mut pad_pb = PathBuilder::new();
        push_rounded_rect(&mut pad_pb, pad_left, pad_top, pad_size, pad_size, pad_r);
        if let Some(pad_path) = pad_pb.finish() {
            let mut pad_paint = Paint::default();
            pad_paint.set_color_rgba8(255, 255, 255, 255);
            pad_paint.anti_alias = true;
            pixmap.fill_path(&pad_path, &pad_paint, FillRule::Winding, Transform::identity(), None);

            let mut pad_border = Paint::default();
            pad_border.set_color_rgba8(220, 220, 225, 255);
            pad_border.anti_alias = true;
            let pad_stroke = Stroke {
                width: 1.0 * dpi * scale,
                ..Default::default()
            };
            pixmap.stroke_path(&pad_path, &pad_border, &pad_stroke, Transform::identity(), None);
        }
    }

    // 7. QR Modules or Cloudflare Tunnel Download / Setup UI
    if state.tab == QrShareTab::Internet && !state.is_cloudflared_installed {
        let tabs_bottom = tabs_y + tab_h;
        if state.is_downloading {
            fonts.draw_text_centered(
                pixmap,
                "Downloading Cloudflare Tunnel...",
                cx,
                tabs_bottom + 48.0 * dpi * scale,
                18.0 * dpi * scale,
                theme.text_primary,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                "One-time setup for secure internet sharing",
                cx,
                tabs_bottom + 76.0 * dpi * scale,
                13.5 * dpi * scale,
                theme.text_secondary,
                false,
            );

            // Full-width progress bar
            let bar_w = 256.0 * dpi * scale;
            let bar_h = 8.0 * dpi * scale;
            let bar_left = cx - bar_w / 2.0;
            let bar_top = tabs_bottom + 104.0 * dpi * scale;
            let mut bar_bg_pb = PathBuilder::new();
            push_rounded_rect(&mut bar_bg_pb, bar_left, bar_top, bar_w, bar_h, 4.0 * dpi * scale);
            if let Some(p) = bar_bg_pb.finish() {
                let mut pt = Paint::default();
                pt.set_color(theme.border_subtle);
                pt.anti_alias = true;
                pixmap.fill_path(&p, &pt, FillRule::Winding, Transform::identity(), None);
            }
            let fill_pct = (state.download_pct.unwrap_or(30) as f32 / 100.0).clamp(0.05, 1.0);
            let mut bar_fg_pb = PathBuilder::new();
            push_rounded_rect(&mut bar_fg_pb, bar_left, bar_top, bar_w * fill_pct, bar_h, 4.0 * dpi * scale);
            if let Some(p) = bar_fg_pb.finish() {
                let mut pt = Paint::default();
                pt.set_color(theme.slice_hover_fill);
                pt.anti_alias = true;
                pixmap.fill_path(&p, &pt, FillRule::Winding, Transform::identity(), None);
            }

            let pct = state.download_pct.unwrap_or(30);
            let pct_text = format!("{pct}% • Please wait...");
            fonts.draw_text_centered(
                pixmap,
                &pct_text,
                cx,
                tabs_bottom + 132.0 * dpi * scale,
                13.0 * dpi * scale,
                theme.text_secondary,
                false,
            );
        } else {
            fonts.draw_text_centered(
                pixmap,
                "Internet Sharing",
                cx,
                tabs_bottom + 42.0 * dpi * scale,
                18.5 * dpi * scale,
                theme.text_primary,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                "Uses Cloudflare Tunnel for end-to-end HTTPS transfers",
                cx,
                tabs_bottom + 70.0 * dpi * scale,
                13.5 * dpi * scale,
                theme.text_secondary,
                false,
            );

            // Download & Enable button directly on popup body
            let btn_w = 256.0 * dpi * scale;
            let btn_h = 40.0 * dpi * scale;
            let btn_left = cx - btn_w / 2.0;
            let btn_top = tabs_bottom + 98.0 * dpi * scale;
            let mut btn_pb = PathBuilder::new();
            push_rounded_rect(&mut btn_pb, btn_left, btn_top, btn_w, btn_h, 8.0 * dpi * scale);
            if let Some(btn_path) = btn_pb.finish() {
                let mut btn_paint = Paint::default();
                if state.is_download_btn_hovered {
                    let r = (theme.slice_hover_fill.red() * 1.15).min(1.0);
                    let g = (theme.slice_hover_fill.green() * 1.15).min(1.0);
                    let b = (theme.slice_hover_fill.blue() * 1.15).min(1.0);
                    btn_paint.set_color_rgba8((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255);
                } else {
                    btn_paint.set_color(theme.slice_hover_fill);
                }
                btn_paint.anti_alias = true;
                pixmap.fill_path(&btn_path, &btn_paint, FillRule::Winding, Transform::identity(), None);
            }
            fonts.draw_text_centered(
                pixmap,
                "Download & Enable",
                cx,
                btn_top + btn_h / 2.0,
                15.0 * dpi * scale,
                Color::WHITE,
                true,
            );

            fonts.draw_text_centered(
                pixmap,
                "~50 MB • One-time setup",
                cx,
                btn_top + btn_h + 20.0 * dpi * scale,
                12.5 * dpi * scale,
                theme.text_secondary,
                false,
            );
        }
    } else if state.tab == QrShareTab::Internet && state.is_connecting_tunnel {
        fonts.draw_text_centered(
            pixmap,
            "Creating secure link...",
            cx,
            pad_top + pad_size / 2.0 - 8.0 * dpi * scale,
            16.0 * dpi * scale,
            Color::from_rgba8(80, 80, 95, 255),
            true,
        );
        fonts.draw_text_centered(
            pixmap,
            "Preparing public share",
            cx,
            pad_top + pad_size / 2.0 + 14.0 * dpi * scale,
            13.5 * dpi * scale,
            Color::from_rgba8(120, 120, 135, 255),
            false,
        );
    } else if state.tab == QrShareTab::Internet && state.tunnel_error.is_some() {
        fonts.draw_text_centered(
            pixmap,
            "Connection Failed",
            cx,
            pad_top + pad_size / 2.0 - 8.0 * dpi * scale,
            16.0 * dpi * scale,
            Color::from_rgba8(220, 60, 60, 255),
            true,
        );
        fonts.draw_text_centered(
            pixmap,
            "Switch to Local tab",
            cx,
            pad_top + pad_size / 2.0 + 14.0 * dpi * scale,
            13.5 * dpi * scale,
            Color::from_rgba8(120, 120, 135, 255),
            false,
        );
    } else {
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
                        if let Some(rect) = Rect::from_xywh(mx, my, module_size + 0.1, module_size + 0.1) {
                            qr_pb.push_rect(rect);
                        }
                    }
                }
            }

            if let Some(qr_path) = qr_pb.finish() {
                let mut qr_paint = Paint::default();
                qr_paint.set_color_rgba8(18, 18, 20, 255);
                qr_paint.anti_alias = true;
                pixmap.fill_path(&qr_path, &qr_paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }

    // 8. Footer Status (only when QR code is visible)
    if show_qr_pad {
        let status_y = pad_top + pad_size + 18.0 * dpi * scale;
        let instruction_y = status_y + 18.0 * dpi * scale;

        match &state.status {
            QrShareStatus::WaitingForScan => {
                fonts.draw_text_centered(
                    pixmap,
                    "Scan with Phone Camera",
                    cx,
                    status_y,
                    16.0 * dpi * scale,
                    theme.text_primary,
                    true,
                );
                let mode_desc = match state.tab {
                    QrShareTab::Local => "Connects via Local Wi-Fi",
                    QrShareTab::Internet => "Connects securely over the Internet",
                };
                fonts.draw_text_centered(
                    pixmap,
                    mode_desc,
                    cx,
                    instruction_y,
                    13.5 * dpi * scale,
                    theme.text_secondary,
                    false,
                );
            }
            QrShareStatus::Scanned => {
                fonts.draw_text_centered(
                    pixmap,
                    "Device Connected",
                    cx,
                    status_y,
                    16.0 * dpi * scale,
                    theme.slice_hover_fill,
                    true,
                );
                fonts.draw_text_centered(
                    pixmap,
                    "Tap download on phone to begin",
                    cx,
                    instruction_y,
                    13.5 * dpi * scale,
                    theme.text_secondary,
                    false,
                );
            }
            QrShareStatus::Sending => {
                fonts.draw_text_centered(
                    pixmap,
                    "Sending to Phone...",
                    cx,
                    status_y,
                    16.0 * dpi * scale,
                    theme.slice_hover_fill,
                    true,
                );
                fonts.draw_text_centered(
                    pixmap,
                    "Downloading file on device",
                    cx,
                    instruction_y,
                    13.5 * dpi * scale,
                    theme.text_secondary,
                    false,
                );
            }
            QrShareStatus::Success => {
                let emerald = Color::from_rgba8(34, 197, 94, 255);
                let b_scale = state.badge_spring.value().max(0.0) * dpi * scale;

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
                fonts.draw_text_centered(
                    pixmap,
                    "Transfer Failed",
                    cx,
                    status_y,
                    15.5 * dpi * scale,
                    theme.danger_text,
                    true,
                );
                fonts.draw_text_centered(
                    pixmap,
                    err,
                    cx,
                    instruction_y,
                    12.5 * dpi * scale,
                    theme.danger_text,
                    false,
                );
            }
        }
    }
}
