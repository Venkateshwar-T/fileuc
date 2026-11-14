//! Reusable HUD Card Rendering Primitives.
//! Provides shared, consistent Win32 tiny-skia UI components across all FileUC modal popups:
//! - Ambient drop shadows
//! - Rounded solid card bodies
//! - Vector stroke close buttons
//! - Segmented pill tabs
//! - Clean, subtle-bordered input boxes
//! - Standardized action buttons

use crate::font::FontContext;
use crate::theme::Theme;
use tiny_skia::{
    Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

/// Appends a rounded rectangle with corner radius `r` to `pb`.
pub fn push_rounded_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
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

/// Returns true during the active (visible) half of a 1.0s blink cycle based on steady wall-clock time.
pub fn is_cursor_visible() -> bool {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    (millis % 1000) < 530
}

/// Renders a soft, natural drop shadow underneath a card without multi-layer visible rings.
pub fn draw_card_shadow(
    pixmap: &mut Pixmap,
    left: f32,
    top: f32,
    w: f32,
    h: f32,
    corner_r: f32,
    scale: f32,
    dpi: f32,
) {
    // 3 subtle, graduated passes with low alpha (matches qr_card, zero visible rings or animation lag)
    for i in 1..=3 {
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
}

/// Renders a solid, rounded card background with subtle border.
pub fn draw_card_body(
    pixmap: &mut Pixmap,
    left: f32,
    top: f32,
    w: f32,
    h: f32,
    corner_r: f32,
    scale: f32,
    dpi: f32,
    theme: &Theme,
) {
    let mut card_pb = PathBuilder::new();
    push_rounded_rect(&mut card_pb, left, top, w, h, corner_r);
    if let Some(card_path) = card_pb.finish() {
        let mut card_paint = Paint::default();
        if theme.is_dark {
            card_paint.set_color_rgba8(26, 26, 28, 255);
        } else {
            card_paint.set_color_rgba8(255, 255, 255, 255);
        }
        card_paint.anti_alias = true;
        pixmap.fill_path(&card_path, &card_paint, FillRule::Winding, Transform::identity(), None);

        let mut border_paint = Paint::default();
        border_paint.set_color(theme.border_subtle);
        border_paint.anti_alias = true;
        let border_stroke = Stroke {
            width: 1.0 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&card_path, &border_paint, &border_stroke, Transform::identity(), None);
    }
}

/// Renders a clean vector stroke close button with circular hover highlight.
pub fn draw_close_button(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    is_hovered: bool,
    scale: f32,
    dpi: f32,
    theme: &Theme,
) {
    if is_hovered {
        let hover_r = 14.0 * dpi * scale;
        let mut hover_pb = PathBuilder::new();
        hover_pb.push_circle(cx, cy, hover_r);
        if let Some(hover_path) = hover_pb.finish() {
            let mut hover_paint = Paint::default();
            if theme.is_dark {
                hover_paint.set_color_rgba8(255, 255, 255, 24);
            } else {
                hover_paint.set_color_rgba8(0, 0, 0, 18);
            }
            hover_paint.anti_alias = true;
            pixmap.fill_path(&hover_path, &hover_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    let close_half = 5.0 * dpi * scale;
    let mut close_pb = PathBuilder::new();
    close_pb.move_to(cx - close_half, cy - close_half);
    close_pb.line_to(cx + close_half, cy + close_half);
    close_pb.move_to(cx + close_half, cy - close_half);
    close_pb.line_to(cx - close_half, cy + close_half);
    if let Some(close_path) = close_pb.finish() {
        let mut close_paint = Paint::default();
        close_paint.set_color(if is_hovered {
            theme.text_primary
        } else {
            theme.text_secondary
        });
        close_paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.8 * dpi * scale,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        pixmap.stroke_path(&close_path, &close_paint, &stroke, Transform::identity(), None);
    }
}

/// Renders a segmented pill tab bar with rounded container and active pill.
pub fn draw_segmented_tabs(
    pixmap: &mut Pixmap,
    tab_left: f32,
    tab_top: f32,
    tab_w: f32,
    tab_h: f32,
    labels: &[&str],
    active_idx: usize,
    hovered_idx: Option<usize>,
    scale: f32,
    dpi: f32,
    theme: &Theme,
    fonts: &FontContext,
) {
    let seg_count = labels.len().max(1);
    let seg_w = tab_w / (seg_count as f32);

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

    for (i, label) in labels.iter().enumerate() {
        let seg_left = tab_left + (i as f32) * seg_w;
        let is_active = i == active_idx;
        let is_hov = hovered_idx == Some(i) && !is_active;

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
                active_paint.set_color(theme.slice_hover_fill);
                active_paint.anti_alias = true;
                pixmap.fill_path(&active_path, &active_paint, FillRule::Winding, Transform::identity(), None);
            }
        } else if is_hov {
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

        let txt_color = if is_active {
            Color::WHITE
        } else {
            theme.text_secondary
        };
        fonts.draw_text_centered(
            pixmap,
            label,
            seg_left + seg_w / 2.0,
            tab_top + tab_h / 2.0,
            13.5 * dpi * scale,
            txt_color,
            is_active,
        );
    }
}

/// Renders the standard clean input field box (theme-aware background + subtle border, NO colored borders).
pub fn draw_input_box(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    corner_r: f32,
    scale: f32,
    dpi: f32,
    theme: &Theme,
) {
    let mut in_pb = PathBuilder::new();
    push_rounded_rect(&mut in_pb, x, y, w, h, corner_r);
    if let Some(in_path) = in_pb.finish() {
        let mut in_paint = Paint::default();
        if theme.is_dark {
            in_paint.set_color_rgba8(34, 34, 38, 255);
        } else {
            in_paint.set_color_rgba8(245, 246, 248, 255);
        }
        in_paint.anti_alias = true;
        pixmap.fill_path(&in_path, &in_paint, FillRule::Winding, Transform::identity(), None);

        let mut in_stroke = Paint::default();
        in_stroke.set_color(theme.border_subtle);
        in_stroke.anti_alias = true;
        let stroke_cfg = Stroke {
            width: 1.0 * dpi * scale,
            ..Default::default()
        };
        pixmap.stroke_path(&in_path, &in_stroke, &stroke_cfg, Transform::identity(), None);
    }
}

/// Renders a standardized action button with disabled, hover, enabled, and success feedback states.
pub fn draw_action_button(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    is_enabled: bool,
    is_hovered: bool,
    is_success: bool,
    corner_r: f32,
    scale: f32,
    dpi: f32,
    theme: &Theme,
    fonts: &FontContext,
) {
    let mut btn_pb = PathBuilder::new();
    push_rounded_rect(&mut btn_pb, x, y, w, h, corner_r);
    if let Some(btn_path) = btn_pb.finish() {
        let mut paint = Paint::default();
        if is_success {
            paint.set_color_rgba8(34, 197, 94, 255); // Emerald green
        } else if !is_enabled {
            if theme.is_dark {
                paint.set_color_rgba8(45, 45, 52, 160);
            } else {
                paint.set_color_rgba8(230, 230, 235, 255);
            }
        } else if is_hovered {
            let r = (theme.slice_hover_fill.red() * 1.15).min(1.0);
            let g = (theme.slice_hover_fill.green() * 1.15).min(1.0);
            let b = (theme.slice_hover_fill.blue() * 1.15).min(1.0);
            paint.set_color_rgba8((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255);
        } else {
            paint.set_color(theme.slice_hover_fill);
        }
        paint.anti_alias = true;
        pixmap.fill_path(&btn_path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    let btn_txt_color = if is_enabled || is_success {
        Color::WHITE
    } else if theme.is_dark {
        Color::from_rgba8(140, 140, 150, 200)
    } else {
        Color::from_rgba8(160, 160, 170, 255)
    };

    fonts.draw_text_centered(
        pixmap,
        label,
        x + w / 2.0,
        y + h / 2.0,
        15.0 * dpi * scale,
        btn_txt_color,
        true,
    );
}

/// Renders a steady-blinking text caret cursor centered vertically at (x, center_y).
pub fn draw_text_cursor(
    pixmap: &mut Pixmap,
    x: f32,
    center_y: f32,
    height: f32,
    scale: f32,
    dpi: f32,
    color: Color,
) {
    if !is_cursor_visible() {
        return;
    }
    let mut pb = PathBuilder::new();
    let half_h = height / 2.0;
    pb.move_to(x, center_y - half_h);
    pb.line_to(x, center_y + half_h);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.2 * dpi * scale,
            line_cap: LineCap::Butt,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}
