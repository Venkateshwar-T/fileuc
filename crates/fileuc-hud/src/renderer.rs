use crate::donut_geometry::DonutLayout;
use crate::spring::Spring;
use crate::theme::Theme;
use fileuc_types::ConversionAction;
use std::f32::consts::TAU;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub struct DonutState {
    pub actions: Vec<ConversionAction>,
    pub file_count: usize,
    pub hovered_slice: Option<usize>,
    pub pop_in: Spring,
    pub per_slice_highlight: Vec<Spring>,
}

impl DonutState {
    pub fn new(actions: Vec<ConversionAction>, file_count: usize) -> Self {
        let count = actions.len().max(1);
        let mut pop_in = Spring::new(0.0, 320.0, 24.0);
        pop_in.set_target(1.0);

        let per_slice_highlight = (0..count)
            .map(|_| Spring::new(0.0, 350.0, 26.0))
            .collect();

        Self {
            actions,
            file_count,
            hovered_slice: None,
            pop_in,
            per_slice_highlight,
        }
    }

    pub fn set_hovered(&mut self, hovered: Option<usize>) {
        self.hovered_slice = hovered;
        for (i, spring) in self.per_slice_highlight.iter_mut().enumerate() {
            if Some(i) == hovered {
                spring.set_target(1.0);
            } else {
                spring.set_target(0.0);
            }
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let mut animating = self.pop_in.tick(dt);
        for spring in &mut self.per_slice_highlight {
            if spring.tick(dt) {
                animating = true;
            }
        }
        animating
    }
}

pub fn render_donut(state: &DonutState, theme: &Theme, pixmap: &mut Pixmap) {
    pixmap.fill(Color::TRANSPARENT);

    let scale = state.pop_in.value();
    if scale <= 0.01 {
        return;
    }

    let cx = pixmap.width() as f32 / 2.0;
    let cy = pixmap.height() as f32 / 2.0;

    let base_inner_r = 45.0 * scale;
    let base_outer_r = 120.0 * scale;
    let count = state.actions.len().max(1);
    let layout = DonutLayout::new(base_inner_r, base_outer_r, count);
    let span = layout.slice_span();
    let gap_rad = 0.025; // gap between slices

    for i in 0..count {
        let h = state.per_slice_highlight.get(i).map_or(0.0, |s| s.value());
        let outer_r = base_outer_r + 4.0 * h * scale;
        let inner_r = base_inner_r;

        let mid_angle = layout.slice_mid_angle(i);
        let start_angle = mid_angle - (span / 2.0) + gap_rad;
        let end_angle = mid_angle + (span / 2.0) - gap_rad;

        let mut pb = PathBuilder::new();

        // Build wedge: arc along outer radius, then line to inner, arc back
        let steps = 16;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let a = start_angle + t * (end_angle - start_angle);
            let px = cx + outer_r * a.sin();
            let py = cy - outer_r * a.cos();
            if s == 0 {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }

        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let a = end_angle - t * (end_angle - start_angle);
            let px = cx + inner_r * a.sin();
            let py = cy - inner_r * a.cos();
            pb.line_to(px, py);
        }
        pb.close();

        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            let r = theme.slice_fill.red() + h * (theme.slice_hover_fill.red() - theme.slice_fill.red());
            let g = theme.slice_fill.green() + h * (theme.slice_hover_fill.green() - theme.slice_fill.green());
            let b = theme.slice_fill.blue() + h * (theme.slice_hover_fill.blue() - theme.slice_fill.blue());
            let a = theme.slice_fill.alpha() + h * (theme.slice_hover_fill.alpha() - theme.slice_fill.alpha());

            paint.set_color_rgba8(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
                (a * 255.0) as u8,
            );
            paint.anti_alias = true;

            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

            // Subtle border if hovered
            if h > 0.05 {
                let mut stroke_paint = Paint::default();
                stroke_paint.set_color_rgba8(255, 255, 255, (h * 180.0) as u8);
                stroke_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 1.5,
                    ..Default::default()
                };
                pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
            }
        }
    }

    // Render Center Hole
    let mut center_pb = PathBuilder::new();
    let center_r = base_inner_r - 2.0;
    let steps = 32;
    for s in 0..=steps {
        let a = (s as f32 / steps as f32) * TAU;
        let px = cx + center_r * a.sin();
        let py = cy - center_r * a.cos();
        if s == 0 {
            center_pb.move_to(px, py);
        } else {
            center_pb.line_to(px, py);
        }
    }
    center_pb.close();

    if let Some(center_path) = center_pb.finish() {
        let mut center_paint = Paint::default();
        center_paint.set_color(theme.center_fill);
        center_paint.anti_alias = true;
        pixmap.fill_path(&center_path, &center_paint, FillRule::Winding, Transform::identity(), None);

        let mut stroke_paint = Paint::default();
        stroke_paint.set_color_rgba8(255, 255, 255, 30);
        stroke_paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.0,
            ..Default::default()
        };
        pixmap.stroke_path(&center_path, &stroke_paint, &stroke, Transform::identity(), None);
    }
}

pub fn render_progress(
    tracker: &crate::progress_ring::ProgressTracker,
    theme: &Theme,
    pixmap: &mut Pixmap,
) {
    pixmap.fill(Color::TRANSPARENT);

    let cx = pixmap.width() as f32 / 2.0;
    let cy = pixmap.height() as f32 / 2.0;
    let r = 40.0;
    let stroke_w = 6.0;

    // Draw background track
    let mut bg_pb = PathBuilder::new();
    bg_pb.push_circle(cx, cy, r);

    if let Some(bg_path) = bg_pb.finish() {
        let mut bg_paint = Paint::default();
        bg_paint.set_color_rgba8(255, 255, 255, 35);
        bg_paint.anti_alias = true;
        let bg_stroke = Stroke {
            width: stroke_w,
            ..Default::default()
        };
        pixmap.stroke_path(&bg_path, &bg_paint, &bg_stroke, Transform::identity(), None);
    }

    match tracker.state() {
        crate::progress_ring::ProgressState::Converting(fraction) => {
            let f = fraction.clamp(0.01, 1.0);
            let arc_steps = (40.0 * f).ceil() as usize;
            let mut arc_pb = PathBuilder::new();
            for s in 0..=arc_steps {
                let t = s as f32 / arc_steps as f32;
                let a = t * f * TAU;
                let px = cx + r * a.sin();
                let py = cy - r * a.cos();
                if s == 0 {
                    arc_pb.move_to(px, py);
                } else {
                    arc_pb.line_to(px, py);
                }
            }

            if let Some(arc_path) = arc_pb.finish() {
                let mut arc_paint = Paint::default();
                arc_paint.set_color(theme.accent_color);
                arc_paint.anti_alias = true;
                let arc_stroke = Stroke {
                    width: stroke_w,
                    ..Default::default()
                };
                pixmap.stroke_path(&arc_path, &arc_paint, &arc_stroke, Transform::identity(), None);
            }
        }
        crate::progress_ring::ProgressState::Success => {
            let mut ring_pb = PathBuilder::new();
            ring_pb.push_circle(cx, cy, r);
            if let Some(ring_path) = ring_pb.finish() {
                let mut ring_paint = Paint::default();
                ring_paint.set_color_rgba8(46, 213, 115, 255); // Emerald green
                ring_paint.anti_alias = true;
                let stroke = Stroke {
                    width: stroke_w,
                    ..Default::default()
                };
                pixmap.stroke_path(&ring_path, &ring_paint, &stroke, Transform::identity(), None);
            }

            // Checkmark
            let mut check_pb = PathBuilder::new();
            check_pb.move_to(cx - 14.0, cy - 2.0);
            check_pb.line_to(cx - 4.0, cy + 8.0);
            check_pb.line_to(cx + 14.0, cy - 10.0);
            if let Some(check_path) = check_pb.finish() {
                let mut check_paint = Paint::default();
                check_paint.set_color_rgba8(46, 213, 115, 255);
                check_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 4.5,
                    ..Default::default()
                };
                pixmap.stroke_path(&check_path, &check_paint, &stroke, Transform::identity(), None);
            }
        }
        crate::progress_ring::ProgressState::Failure(_) => {
            let mut ring_pb = PathBuilder::new();
            ring_pb.push_circle(cx, cy, r);
            if let Some(ring_path) = ring_pb.finish() {
                let mut ring_paint = Paint::default();
                ring_paint.set_color_rgba8(255, 71, 87, 255); // Crimson
                ring_paint.anti_alias = true;
                let stroke = Stroke {
                    width: stroke_w,
                    ..Default::default()
                };
                pixmap.stroke_path(&ring_path, &ring_paint, &stroke, Transform::identity(), None);
            }

            // Cross
            let mut cross_pb = PathBuilder::new();
            cross_pb.move_to(cx - 10.0, cy - 10.0);
            cross_pb.line_to(cx + 10.0, cy + 10.0);
            cross_pb.move_to(cx + 10.0, cy - 10.0);
            cross_pb.line_to(cx - 10.0, cy + 10.0);
            if let Some(cross_path) = cross_pb.finish() {
                let mut cross_paint = Paint::default();
                cross_paint.set_color_rgba8(255, 71, 87, 255);
                cross_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 4.5,
                    ..Default::default()
                };
                pixmap.stroke_path(&cross_path, &cross_paint, &stroke, Transform::identity(), None);
            }
        }
    }
}
