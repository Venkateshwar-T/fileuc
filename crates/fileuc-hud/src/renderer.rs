use crate::donut_geometry::DonutLayout;
use crate::font::FontContext;
use crate::spring::Spring;
use crate::theme::Theme;
use fileuc_types::ConversionAction;
use std::f32::consts::TAU;
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

pub struct DonutState {
    pub actions: Vec<ConversionAction>,
    pub file_count: usize,
    pub file_label: Option<String>,
    pub hovered_slice: Option<usize>,
    pub center_hot: bool,
    pub pop_in: Spring,
    pub per_slice_highlight: Vec<Spring>,
    pub center_pulse: Spring,
    pub center_hover: Spring,
    pub dpi_scale: f32,
    pub has_files: bool,
}

impl DonutState {
    pub fn new(actions: Vec<ConversionAction>, file_count: usize, dpi_scale: f32) -> Self {
        let count = actions.len().max(1);
        let mut pop_in = Spring::new(0.0, 750.0, 36.0);
        pop_in.set_target(1.0);

        let per_slice_highlight = (0..count)
            .map(|_| Spring::new(0.0, 350.0, 26.0))
            .collect();

        let has_files = file_count > 0;

        Self {
            actions,
            file_count,
            file_label: None,
            hovered_slice: None,
            center_hot: false,
            pop_in,
            per_slice_highlight,
            center_pulse: Spring::new(0.0, 260.0, 12.0),
            center_hover: Spring::new(0.0, 350.0, 26.0),
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
            has_files,
        }
    }

    pub fn set_file_label(&mut self, label: impl Into<String>) {
        self.file_label = Some(label.into());
    }

    pub fn update_content(
        &mut self,
        actions: Vec<ConversionAction>,
        file_count: usize,
        file_label: Option<String>,
    ) {
        let count = actions.len().max(1);
        if count != self.per_slice_highlight.len() {
            self.per_slice_highlight = (0..count)
                .map(|_| Spring::new(0.0, 350.0, 26.0))
                .collect();
            self.hovered_slice = None;
        }
        self.actions = actions;
        self.file_count = file_count;
        self.file_label = file_label;
        self.has_files = true;
    }

    pub fn set_center_hot(&mut self, hot: bool) {
        self.center_hot = hot;
        if hot {
            self.center_hover.set_target(1.0);
            self.hovered_slice = None;
            for spring in &mut self.per_slice_highlight {
                spring.set_target(0.0);
            }
        } else {
            self.center_hover.set_target(0.0);
        }
    }

    pub fn set_hovered(&mut self, hovered: Option<usize>) {
        self.hovered_slice = hovered;
        if hovered.is_some() {
            self.center_hot = false;
            self.center_hover.set_target(0.0);
        }
        for (i, spring) in self.per_slice_highlight.iter_mut().enumerate() {
            if Some(i) == hovered {
                spring.set_target(1.0);
            } else {
                spring.set_target(0.0);
            }
        }
    }

    pub fn pulse_center(&mut self) {
        self.center_pulse.set_velocity(6.0);
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let mut animating = self.pop_in.tick(dt);
        if self.center_pulse.tick(dt) {
            animating = true;
        }
        if self.center_hover.tick(dt) {
            animating = true;
        }
        for spring in &mut self.per_slice_highlight {
            if spring.tick(dt) {
                animating = true;
            }
        }
        animating
    }
}

pub fn render_donut(
    state: &DonutState,
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
    let dpi_scale = state.dpi_scale;

    let base_outer_r = 150.0 * dpi_scale * scale;
    let base_inner_r = 63.0 * dpi_scale * scale;
    let pop_out = 7.0 * dpi_scale;
    let shadow_margin = 14.0 * dpi_scale;

    let count = state.actions.len().max(1);
    let layout = DonutLayout::new(base_inner_r, base_outer_r, count);
    let span = layout.slice_span();

    // 0. Invisible Hit-Test Shield covering the entire donut disk
    // Alpha = 1 is completely imperceptible to the human eye (< 0.4% opacity),
    // but prevents Windows OLE from passing drag events through the gaps (between
    // center circle and slices, and between slices) to Windows Explorer!
    let shield_r = base_outer_r + pop_out + 4.0 * dpi_scale;
    let mut shield_pb = PathBuilder::new();
    shield_pb.push_circle(cx, cy, shield_r);
    if let Some(shield_path) = shield_pb.finish() {
        let mut shield_paint = Paint::default();
        shield_paint.set_color_rgba8(0, 0, 0, 1);
        shield_paint.anti_alias = true;
        pixmap.fill_path(&shield_path, &shield_paint, FillRule::Winding, Transform::identity(), None);
    }

    // 1. Draw Soft Radial Shadow underneath slices
    let shadow_r = base_outer_r + shadow_margin;
    let shadow_center_y = cy + 4.0 * dpi_scale;
    let shadow_base_alpha = if theme.is_dark { 46.0 } else { 58.0 };
    let shadow_stops = vec![
        tiny_skia::GradientStop::new(0.0, Color::from_rgba8(0, 0, 0, 0)),
        tiny_skia::GradientStop::new(
            ((base_outer_r - 18.0 * dpi_scale) / shadow_r).clamp(0.0, 1.0),
            Color::from_rgba8(0, 0, 0, 0),
        ),
        tiny_skia::GradientStop::new(
            ((base_outer_r - 2.0 * dpi_scale) / shadow_r).clamp(0.0, 1.0),
            Color::from_rgba8(0, 0, 0, (shadow_base_alpha * scale.min(1.0)) as u8),
        ),
        tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0, 0, 0, 0)),
    ];
    if let Some(shader) = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx, shadow_center_y),
        tiny_skia::Point::from_xy(cx, shadow_center_y),
        shadow_r,
        shadow_stops,
        tiny_skia::SpreadMode::Pad,
        tiny_skia::Transform::identity(),
    ) {
        let mut shadow_paint = Paint::default();
        shadow_paint.shader = shader;
        shadow_paint.anti_alias = true;
        let mut spb = PathBuilder::new();
        spb.push_circle(cx, shadow_center_y, shadow_r);
        if let Some(spath) = spb.finish() {
            pixmap.fill_path(&spath, &shadow_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 2. Draw Slices
    let gap_px = 3.0 * dpi_scale;
    let corner_r = 10.0 * dpi_scale;

    for i in 0..count {
        let h = state.per_slice_highlight.get(i).map_or(0.0, |s| s.value());
        let offset_dist = h * pop_out * scale;
        let mid_angle = layout.slice_mid_angle(i);
        let off_x = offset_dist * mid_angle.sin();
        let off_y = -offset_dist * mid_angle.cos();

        let outer_half = (span / 2.0 - gap_px / (2.0 * base_outer_r)).max(0.01);
        let inner_half = (span / 2.0 - gap_px / (2.0 * base_inner_r)).max(0.01);

        let start_angle_outer = mid_angle - outer_half;
        let end_angle_outer = mid_angle + outer_half;
        let start_angle_inner = mid_angle - inner_half;
        let end_angle_inner = mid_angle + inner_half;

        let d_angle_out = (corner_r / base_outer_r).min(outer_half * 0.45);
        let d_angle_in = (corner_r / base_inner_r).min(inner_half * 0.45);
        let d_r = corner_r.min((base_outer_r - base_inner_r) * 0.35);

        let pt = |angle: f32, radius: f32| -> (f32, f32) {
            (
                cx + off_x + radius * angle.sin(),
                cy + off_y - radius * angle.cos(),
            )
        };

        let mut pb = PathBuilder::new();

        // Start just past outer-start corner
        let p_start = pt(start_angle_outer + d_angle_out, base_outer_r);
        pb.move_to(p_start.0, p_start.1);

        // 1. Arc along outer radius to end_angle_outer - d_angle_out
        let steps = 16;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let a = (start_angle_outer + d_angle_out)
                + t * ((end_angle_outer - d_angle_out) - (start_angle_outer + d_angle_out));
            let p = pt(a, base_outer_r);
            pb.line_to(p.0, p.1);
        }

        // 2. Corner at outer end
        let c_outer_end = pt(end_angle_outer, base_outer_r);
        let p_line_top = pt(end_angle_outer, base_outer_r - d_r);
        pb.quad_to(c_outer_end.0, c_outer_end.1, p_line_top.0, p_line_top.1);

        // 3. Straight line down to inner radius
        let p_line_bot = pt(end_angle_inner, base_inner_r + d_r);
        pb.line_to(p_line_bot.0, p_line_bot.1);

        // 4. Corner at inner end
        let c_inner_end = pt(end_angle_inner, base_inner_r);
        let p_inner_arc_start = pt(end_angle_inner - d_angle_in, base_inner_r);
        pb.quad_to(c_inner_end.0, c_inner_end.1, p_inner_arc_start.0, p_inner_arc_start.1);

        // 5. Arc along inner radius back to start_angle_inner + d_angle_in
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let a = (end_angle_inner - d_angle_in)
                - t * ((end_angle_inner - d_angle_in) - (start_angle_inner + d_angle_in));
            let p = pt(a, base_inner_r);
            pb.line_to(p.0, p.1);
        }

        // 6. Corner at inner start
        let c_inner_start = pt(start_angle_inner, base_inner_r);
        let p_start_bot = pt(start_angle_inner, base_inner_r + d_r);
        pb.quad_to(c_inner_start.0, c_inner_start.1, p_start_bot.0, p_start_bot.1);

        // 7. Straight line up to outer radius
        let p_start_top = pt(start_angle_outer, base_outer_r - d_r);
        pb.line_to(p_start_top.0, p_start_top.1);

        // 8. Corner at outer start
        let c_outer_start = pt(start_angle_outer, base_outer_r);
        pb.quad_to(c_outer_start.0, c_outer_start.1, p_start.0, p_start.1);
        pb.close();

        if let Some(path) = pb.finish() {
            let h_clamped = h.clamp(0.0, 1.0);

            // 1. Interpolate background fill continuously
            let mut paint = Paint::default();
            let r = theme.slice_fill.red() + h_clamped * (theme.slice_hover_fill.red() - theme.slice_fill.red());
            let g = theme.slice_fill.green() + h_clamped * (theme.slice_hover_fill.green() - theme.slice_fill.green());
            let b = theme.slice_fill.blue() + h_clamped * (theme.slice_hover_fill.blue() - theme.slice_fill.blue());
            let a = theme.slice_fill.alpha() + h_clamped * (theme.slice_hover_fill.alpha() - theme.slice_fill.alpha());

            paint.set_color_rgba8(
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                (a * 255.0).round() as u8,
            );
            paint.anti_alias = true;

            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

            // 2. Interpolate border stroke color in 100% lockstep sync with background
            let br = theme.border_subtle.red() + h_clamped * (theme.slice_hover_border.red() - theme.border_subtle.red());
            let bg = theme.border_subtle.green() + h_clamped * (theme.slice_hover_border.green() - theme.border_subtle.green());
            let bb = theme.border_subtle.blue() + h_clamped * (theme.slice_hover_border.blue() - theme.border_subtle.blue());
            let ba = theme.border_subtle.alpha() + h_clamped * (theme.slice_hover_border.alpha() - theme.border_subtle.alpha());

            let mut stroke_paint = Paint::default();
            stroke_paint.set_color_rgba8(
                (br * 255.0).round() as u8,
                (bg * 255.0).round() as u8,
                (bb * 255.0).round() as u8,
                (ba * 255.0).round() as u8,
            );
            stroke_paint.anti_alias = true;
            let stroke = Stroke {
                width: (0.9 + 0.1 * h_clamped) * dpi_scale,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
        }

        // Draw Slice Action text (Single centered prominent label, in 100% color sync)
        if state.has_files {
            if let Some(action) = state.actions.get(i) {
                let mid_r = (base_inner_r + base_outer_r) / 2.0;
                let label_x = cx + off_x + mid_r * mid_angle.sin();
                let label_y = cy + off_y - mid_r * mid_angle.cos();

                let h_clamped = h.clamp(0.0, 1.0);
                let tr = theme.text_primary.red() + h_clamped * (theme.text_on_accent.red() - theme.text_primary.red());
                let tg = theme.text_primary.green() + h_clamped * (theme.text_on_accent.green() - theme.text_primary.green());
                let tb = theme.text_primary.blue() + h_clamped * (theme.text_on_accent.blue() - theme.text_primary.blue());
                let ta = theme.text_primary.alpha() + h_clamped * (theme.text_on_accent.alpha() - theme.text_primary.alpha());

                let title_color = Color::from_rgba8(
                    (tr * 255.0).round() as u8,
                    (tg * 255.0).round() as u8,
                    (tb * 255.0).round() as u8,
                    (ta * 255.0).round() as u8,
                );

                fonts.draw_text_centered(
                    pixmap,
                    action.label(),
                    label_x,
                    label_y,
                    18.5 * dpi_scale,
                    title_color,
                    true,
                );
            }
        }
    }

    // 3. Draw Center Hub
    let pulse = 1.0 + state.center_pulse.value() * 0.06;
    let center_r = (base_inner_r - 6.0 * dpi_scale) * pulse;

    let mut center_pb = PathBuilder::new();
    center_pb.push_circle(cx, cy, center_r);

    if let Some(center_path) = center_pb.finish() {
        let ch = state.center_hover.value().clamp(0.0, 1.0);

        // Smoothly interpolate center circle background to slight red on hover
        let cr = theme.center_fill.red() + ch * (theme.danger_fill.red() - theme.center_fill.red());
        let cg = theme.center_fill.green() + ch * (theme.danger_fill.green() - theme.center_fill.green());
        let cb = theme.center_fill.blue() + ch * (theme.danger_fill.blue() - theme.center_fill.blue());
        let ca = theme.center_fill.alpha() + ch * (theme.danger_fill.alpha() - theme.center_fill.alpha());

        let mut center_paint = Paint::default();
        center_paint.set_color_rgba8(
            (cr * 255.0).round() as u8,
            (cg * 255.0).round() as u8,
            (cb * 255.0).round() as u8,
            (ca * 255.0).round() as u8,
        );
        center_paint.anti_alias = true;
        pixmap.fill_path(&center_path, &center_paint, FillRule::Winding, Transform::identity(), None);

        // Smoothly interpolate center circle border stroke in 100% lockstep sync
        let cbr = theme.border_subtle.red() + ch * (theme.danger_border.red() - theme.border_subtle.red());
        let cbg = theme.border_subtle.green() + ch * (theme.danger_border.green() - theme.border_subtle.green());
        let cbb = theme.border_subtle.blue() + ch * (theme.danger_border.blue() - theme.border_subtle.blue());
        let cba = theme.border_subtle.alpha() + ch * (theme.danger_border.alpha() - theme.border_subtle.alpha());

        let mut stroke_paint = Paint::default();
        stroke_paint.set_color_rgba8(
            (cbr * 255.0).round() as u8,
            (cbg * 255.0).round() as u8,
            (cbb * 255.0).round() as u8,
            (cba * 255.0).round() as u8,
        );
        stroke_paint.anti_alias = true;
        let stroke = Stroke {
            width: (0.85 + 0.15 * ch) * dpi_scale,
            ..Default::default()
        };
        pixmap.stroke_path(&center_path, &stroke_paint, &stroke, Transform::identity(), None);

        // Center Text
        let title_text = match &state.file_label {
            Some(lbl) => lbl.clone(),
            None => {
                if state.file_count == 1 {
                    "1 File".to_string()
                } else {
                    format!("{} Files", state.file_count)
                }
            }
        };

        let subtitle_text = if ch > 0.4 {
            "Cancel".to_string()
        } else if let Some(idx) = state.hovered_slice {
            if let Some(action) = state.actions.get(idx) {
                action.center_prompt().to_string()
            } else {
                "Drop to Convert".to_string()
            }
        } else {
            "Drop to Convert".to_string()
        };

        let tit_r = theme.text_primary.red() + ch * (theme.danger_text.red() - theme.text_primary.red());
        let tit_g = theme.text_primary.green() + ch * (theme.danger_text.green() - theme.text_primary.green());
        let tit_b = theme.text_primary.blue() + ch * (theme.danger_text.blue() - theme.text_primary.blue());
        let tit_a = theme.text_primary.alpha() + ch * (theme.danger_text.alpha() - theme.text_primary.alpha());
        let title_color = Color::from_rgba8(
            (tit_r * 255.0).round() as u8,
            (tit_g * 255.0).round() as u8,
            (tit_b * 255.0).round() as u8,
            (tit_a * 255.0).round() as u8,
        );

        let sub_r = theme.text_secondary.red() + ch * (theme.danger_text.red() - theme.text_secondary.red());
        let sub_g = theme.text_secondary.green() + ch * (theme.danger_text.green() - theme.text_secondary.green());
        let sub_b = theme.text_secondary.blue() + ch * (theme.danger_text.blue() - theme.text_secondary.blue());
        let sub_a = theme.text_secondary.alpha() + ch * (theme.danger_text.alpha() - theme.text_secondary.alpha());
        let subtitle_color = Color::from_rgba8(
            (sub_r * 255.0).round() as u8,
            (sub_g * 255.0).round() as u8,
            (sub_b * 255.0).round() as u8,
            (sub_a * 255.0).round() as u8,
        );

        fonts.draw_text_centered(
            pixmap,
            &title_text,
            cx,
            cy - 9.0 * dpi_scale,
            16.5 * dpi_scale,
            title_color,
            true,
        );

        fonts.draw_text_centered(
            pixmap,
            &subtitle_text,
            cx,
            cy + 11.5 * dpi_scale,
            15.5 * dpi_scale,
            subtitle_color,
            false,
        );
    }
}

pub fn render_progress(
    tracker: &crate::progress_ring::ProgressTracker,
    theme: &Theme,
    fonts: &FontContext,
    dpi_scale: f32,
    pixmap: &mut Pixmap,
) {
    pixmap.fill(Color::TRANSPARENT);

    let cx = pixmap.width() as f32 / 2.0;
    let cy = pixmap.height() as f32 / 2.0;
    let plate_radius = 44.0 * dpi_scale;
    let track_radius = 35.0 * dpi_scale; // 9px padding from true circumference!
    let stroke_w = 4.8 * dpi_scale;

    // 1. Draw Plate Soft Shadow
    for i in 1..=4 {
        let shadow_r = plate_radius + (i as f32) * 1.5 * dpi_scale;
        let mut shadow_pb = PathBuilder::new();
        shadow_pb.push_circle(cx, cy + 2.0 * dpi_scale, shadow_r);
        if let Some(shadow_path) = shadow_pb.finish() {
            let mut shadow_paint = Paint::default();
            shadow_paint.set_color_rgba8(0, 0, 0, 8);
            shadow_paint.anti_alias = true;
            pixmap.fill_path(&shadow_path, &shadow_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 2. Draw Solid Plate Background (Solid White in Light Mode)
    let mut plate_pb = PathBuilder::new();
    plate_pb.push_circle(cx, cy, plate_radius);
    if let Some(plate_path) = plate_pb.finish() {
        let mut plate_paint = Paint::default();
        if theme.is_dark {
            plate_paint.set_color_rgba8(32, 32, 30, 255);
        } else {
            plate_paint.set_color_rgba8(255, 255, 255, 255); // Solid White
        }
        plate_paint.anti_alias = true;
        pixmap.fill_path(&plate_path, &plate_paint, FillRule::Winding, Transform::identity(), None);

        // Subtle plate border
        let mut border_paint = Paint::default();
        border_paint.set_color(theme.border_subtle);
        border_paint.anti_alias = true;
        let border_stroke = Stroke {
            width: 1.0 * dpi_scale,
            ..Default::default()
        };
        pixmap.stroke_path(&plate_path, &border_paint, &border_stroke, Transform::identity(), None);
    }

    // 3. Draw Circular Background Track
    let mut bg_track_pb = PathBuilder::new();
    bg_track_pb.push_circle(cx, cy, track_radius);
    if let Some(bg_track_path) = bg_track_pb.finish() {
        let mut bg_track_paint = Paint::default();
        if theme.is_dark {
            bg_track_paint.set_color_rgba8(60, 60, 56, 255);
        } else {
            bg_track_paint.set_color_rgba8(235, 233, 228, 255);
        }
        bg_track_paint.anti_alias = true;
        let track_stroke = Stroke {
            width: stroke_w,
            ..Default::default()
        };
        pixmap.stroke_path(&bg_track_path, &bg_track_paint, &track_stroke, Transform::identity(), None);
    }

    // 4. Progress Arc or Outcome
    match tracker.state() {
        crate::progress_ring::ProgressState::Converting(_) | crate::progress_ring::ProgressState::Success => {
            let f = tracker.shown_progress().clamp(0.0, 1.0);

            // Draw Green Circular Rounded Loader Progress Bar on Circumference
            if f > 0.005 {
                let arc_steps = (64.0 * f).ceil().max(2.0) as usize;
                let mut arc_pb = PathBuilder::new();
                for s in 0..=arc_steps {
                    let t = s as f32 / arc_steps as f32;
                    let a = t * f * TAU;
                    let px = cx + track_radius * a.sin();
                    let py = cy - track_radius * a.cos();
                    if s == 0 {
                        arc_pb.move_to(px, py);
                    } else {
                        arc_pb.line_to(px, py);
                    }
                }

                if let Some(arc_path) = arc_pb.finish() {
                    let mut arc_paint = Paint::default();
                    arc_paint.set_color_rgba8(46, 213, 115, 255); // Emerald Green
                    arc_paint.anti_alias = true;
                    let arc_stroke = Stroke {
                        width: stroke_w,
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&arc_path, &arc_paint, &arc_stroke, Transform::identity(), None);
                }
            }

            if tracker.is_popped() {
                // Checkmark grows quickly from center
                let scale = tracker.badge_scale() * dpi_scale;
                if scale > 0.01 {
                    let mut check_pb = PathBuilder::new();
                    let p1 = (cx - 7.5 * scale, cy + 0.5 * scale);
                    let p2 = (cx - 2.0 * scale, cy + 6.0 * scale);
                    let p3 = (cx + 8.0 * scale, cy - 4.5 * scale);
                    check_pb.move_to(p1.0, p1.1);
                    check_pb.line_to(p2.0, p2.1);
                    check_pb.line_to(p3.0, p3.1);

                    if let Some(check_path) = check_pb.finish() {
                        let mut check_paint = Paint::default();
                        check_paint.set_color_rgba8(46, 213, 115, 255);
                        check_paint.anti_alias = true;
                        let check_stroke = Stroke {
                            width: 3.2 * scale,
                            line_cap: LineCap::Round,
                            line_join: LineJoin::Round,
                            ..Default::default()
                        };
                        pixmap.stroke_path(&check_path, &check_paint, &check_stroke, Transform::identity(), None);
                    }
                }
            } else {
                // Percentage in the center (Increased font size to 22px!)
                let pct_text = format!("{:.0}%", (f * 100.0).clamp(0.0, 100.0));
                fonts.draw_text_centered(
                    pixmap,
                    &pct_text,
                    cx,
                    cy + 1.0 * dpi_scale,
                    22.0 * dpi_scale,
                    theme.text_primary,
                    true,
                );
            }
        }
        crate::progress_ring::ProgressState::Failure(msg) => {
            // Failure red ring
            let mut ring_pb = PathBuilder::new();
            ring_pb.push_circle(cx, cy, track_radius);
            if let Some(ring_path) = ring_pb.finish() {
                let mut ring_paint = Paint::default();
                ring_paint.set_color(theme.danger_text);
                ring_paint.anti_alias = true;
                let stroke = Stroke {
                    width: stroke_w,
                    ..Default::default()
                };
                pixmap.stroke_path(&ring_path, &ring_paint, &stroke, Transform::identity(), None);
            }

            // Cross in center
            let mut x_pb = PathBuilder::new();
            let x_scale = dpi_scale;
            x_pb.move_to(cx - 7.0 * x_scale, cy - 7.0 * x_scale);
            x_pb.line_to(cx + 7.0 * x_scale, cy + 7.0 * x_scale);
            x_pb.move_to(cx + 7.0 * x_scale, cy - 7.0 * x_scale);
            x_pb.line_to(cx - 7.0 * x_scale, cy + 7.0 * x_scale);
            if let Some(x_path) = x_pb.finish() {
                let mut x_paint = Paint::default();
                x_paint.set_color(theme.danger_text);
                x_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 2.8 * x_scale,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };
                pixmap.stroke_path(&x_path, &x_paint, &stroke, Transform::identity(), None);
            }

            fonts.draw_text_centered(
                pixmap,
                msg,
                cx,
                cy + 24.0 * dpi_scale,
                11.0 * dpi_scale,
                theme.danger_text,
                false,
            );
        }
    }
}
