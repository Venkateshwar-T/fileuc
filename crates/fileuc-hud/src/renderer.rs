use crate::card_common::push_rounded_rect;
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
    pub missing_plugins: Vec<ConversionAction>,
    pub expanded_group: Option<usize>,
    pub expand_spring: Spring,
    pub sub_hover: Option<usize>,
    pub pending_group: Option<usize>,
    pub pending_since: Option<std::time::Instant>,
    pub pending_distance: f32,
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
            missing_plugins: Vec::new(),
            expanded_group: None,
            expand_spring: Spring::new(0.0, 320.0, 24.0),
            sub_hover: None,
            pending_group: None,
            pending_since: None,
            pending_distance: 0.0,
        }
    }

    pub fn set_file_label(&mut self, label: impl Into<String>) {
        self.file_label = Some(label.into());
    }

    pub fn set_missing_plugins(&mut self, missing: Vec<ConversionAction>) {
        self.missing_plugins = missing;
    }

    pub fn is_plugin_missing(&self, action: ConversionAction) -> bool {
        if action == ConversionAction::OfficeGroup {
            return self.missing_plugins.contains(&ConversionAction::OfficeGroup)
                || self.missing_plugins.contains(&ConversionAction::PdfToDocx);
        }
        self.missing_plugins.contains(&action)
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
        self.expanded_group = None;
        self.expand_spring = Spring::new(0.0, 320.0, 24.0);
        self.sub_hover = None;
        self.pending_group = None;
        self.pending_since = None;
        self.pending_distance = 0.0;
    }

    pub fn expand_group(&mut self, group_idx: usize) {
        if self.expanded_group != Some(group_idx) {
            self.expanded_group = Some(group_idx);
            self.expand_spring = Spring::new(0.0, 320.0, 24.0);
            self.expand_spring.set_target(1.0);
            self.sub_hover = None;
        }
        self.pending_group = None;
        self.pending_since = None;
    }

    pub fn collapse_group(&mut self) {
        if self.expanded_group.is_some() {
            self.expanded_group = None;
            self.expand_spring.set_damping(36.0);
            self.expand_spring.set_target(0.0);
            self.sub_hover = None;
        }
        self.pending_group = None;
        self.pending_since = None;
    }

    pub fn track_pointer(&mut self, rel_x: f32, rel_y: f32, pop_out: f32) -> crate::donut_geometry::HitResult {
        let count = self.actions.len().max(1);
        let scale = self.pop_in.value();
        let layout = DonutLayout::with_dpi(
            63.0 * self.dpi_scale * scale,
            150.0 * self.dpi_scale * scale,
            count,
            self.dpi_scale * scale,
        );

        let dist = (rel_x * rel_x + rel_y * rel_y).sqrt();

        // 1. Check if pointer hits an expanded sub-slice
        if let Some(group_idx) = self.expanded_group {
            if group_idx < self.actions.len() {
                let action = self.actions[group_idx];
                if let Some(children) = crate::smart_filter::get_action_children(action) {
                    if let Some(sub_idx) = layout.hit_test_sub(rel_x, rel_y, group_idx, children.len(), pop_out) {
                        self.sub_hover = Some(sub_idx);
                        self.set_hovered(None);
                        return crate::donut_geometry::HitResult::SubSlice { group_idx, sub_idx };
                    }
                }
            }
        }

        // 2. Normal donut slice testing
        let hit = layout.hit_test_with_slack(rel_x, rel_y, pop_out);
        match hit {
            crate::donut_geometry::HitResult::Slice(idx) if idx < self.actions.len() => {
                let action = self.actions[idx];
                let is_group = crate::smart_filter::get_action_children(action).is_some();
                if is_group {
                    if self.expanded_group != Some(idx) {
                        if self.pending_group != Some(idx) {
                            self.pending_group = Some(idx);
                            self.pending_since = Some(std::time::Instant::now());
                            self.pending_distance = dist;
                        } else {
                            // Check for 10px outward movement
                            if dist - self.pending_distance >= 10.0 * self.dpi_scale {
                                self.expand_group(idx);
                            } else if let Some(since) = self.pending_since {
                                if since.elapsed() >= std::time::Duration::from_millis(160) {
                                    self.expand_group(idx);
                                }
                            }
                        }
                    }
                } else if self.expanded_group.is_some() {
                    self.collapse_group();
                }
                self.sub_hover = None;
                self.set_hovered(Some(idx));
                hit
            }
            crate::donut_geometry::HitResult::CenterHole => {
                self.collapse_group();
                self.set_center_hot(true);
                hit
            }
            crate::donut_geometry::HitResult::Outside => {
                // If group is expanded, check if pointer is anywhere in the outer fan corridor
                if let Some(group_idx) = self.expanded_group {
                    let (_, outer) = layout.sub_radii(pop_out);
                    let min_r = layout.outer_radius();
                    let max_r = outer + 20.0 * self.dpi_scale;
                    let parent_mid = layout.slice_mid_angle(group_idx);
                    let mut angle = rel_x.atan2(-rel_y);
                    if angle < 0.0 {
                        angle += std::f32::consts::TAU;
                    }
                    let mut delta = (angle - parent_mid) % std::f32::consts::TAU;
                    if delta > std::f32::consts::PI {
                        delta -= std::f32::consts::TAU;
                    } else if delta < -std::f32::consts::PI {
                        delta += std::f32::consts::TAU;
                    }

                    if dist >= min_r && dist <= max_r && delta.abs() <= 0.6 {
                        // In the corridor: keep expanded and don't collapse!
                        return crate::donut_geometry::HitResult::Outside;
                    }
                }
                self.collapse_group();
                self.set_hovered(None);
                self.set_center_hot(false);
                hit
            }
            crate::donut_geometry::HitResult::SubSlice { group_idx: _, sub_idx } => {
                self.sub_hover = Some(sub_idx);
                hit
            }
            crate::donut_geometry::HitResult::Slice(_) => {
                self.collapse_group();
                hit
            }
        }
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
        if self.expand_spring.tick(dt) {
            animating = true;
        }
        if let (Some(pending_idx), Some(since)) = (self.pending_group, self.pending_since) {
            if since.elapsed() >= std::time::Duration::from_millis(160) {
                self.expand_group(pending_idx);
                animating = true;
            }
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
    let layout = DonutLayout::with_dpi(base_inner_r, base_outer_r, count, dpi_scale * scale);
    let span = layout.slice_span();

    // 0. Invisible Hit-Test Shield covering the entire donut disk
    // Alpha = 1 is completely imperceptible to the human eye (< 0.4% opacity),
    // but prevents Windows OLE from passing drag events through the gaps (between
    // center circle and slices, and between slices) to Windows Explorer!
    // Expands to cover outer sub-slices when a slice group is expanded.
    let shield_r = if state.expanded_group.is_some() {
        let (_, sub_outer_r) = layout.sub_radii(pop_out);
        sub_outer_r + pop_out + 6.0 * dpi_scale
    } else {
        base_outer_r + pop_out + 4.0 * dpi_scale
    };
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

                let is_missing = state.is_plugin_missing(*action);
                let font_size = 18.5 * dpi_scale;

                if is_missing {
                    let label = action.label();
                    let text_w = fonts.measure_text(label, font_size, true);
                    let arrow_w = 8.0 * dpi_scale;
                    let gap = 5.0 * dpi_scale;
                    let total_w = text_w + gap + arrow_w;

                    // Draw label text shifted so [Text + Gap + Arrow] is centered at label_x
                    let text_cx = label_x - (total_w / 2.0) + (text_w / 2.0);
                    fonts.draw_text_centered(
                        pixmap,
                        label,
                        text_cx,
                        label_y,
                        font_size,
                        title_color,
                        true,
                    );

                    // Draw clean vector down-arrow icon (vertical stem + downward arrowhead)
                    let ar_x = label_x + (total_w / 2.0) - (arrow_w / 2.0);
                    let ar_y = label_y;
                    let stem_top = ar_y - 5.5 * dpi_scale;
                    let stem_bot = ar_y + 5.5 * dpi_scale;
                    let wing_dx = 3.8 * dpi_scale;
                    let wing_dy = 3.8 * dpi_scale;

                    let mut ar_pb = PathBuilder::new();
                    // Vertical stem
                    ar_pb.move_to(ar_x, stem_top);
                    ar_pb.line_to(ar_x, stem_bot);
                    // Downward arrowhead wings
                    ar_pb.move_to(ar_x - wing_dx, stem_bot - wing_dy);
                    ar_pb.line_to(ar_x, stem_bot);
                    ar_pb.line_to(ar_x + wing_dx, stem_bot - wing_dy);

                    if let Some(ar_path) = ar_pb.finish() {
                        let mut ar_paint = Paint::default();
                        ar_paint.set_color(title_color);
                        ar_paint.anti_alias = true;
                        let stroke = Stroke {
                            width: 2.0 * dpi_scale,
                            line_cap: LineCap::Round,
                            line_join: LineJoin::Round,
                            ..Default::default()
                        };
                        pixmap.stroke_path(&ar_path, &ar_paint, &stroke, Transform::identity(), None);
                    }
                } else {
                    fonts.draw_text_centered(
                        pixmap,
                        action.label(),
                        label_x,
                        label_y,
                        font_size,
                        title_color,
                        true,
                    );
                }
            }
        }
    }

    // 2.5 Draw Sub-slices fan-out if an action group is expanded
    if state.expand_spring.value() > 0.005 {
        if let Some(group_idx) = state.expanded_group {
            if group_idx < state.actions.len() {
                let parent_action = state.actions[group_idx];
                if let Some(children) = crate::smart_filter::get_action_children(parent_action) {
                    let t = state.expand_spring.value().clamp(0.0, 1.0);
                    let scale_sub = 0.8 + 0.2 * t;
                    let (sub_inner_r, sub_outer_r) = layout.sub_radii(pop_out);
                    let parent_mid = layout.slice_mid_angle(group_idx);
                    let sub_span = layout.sub_span(children.len());

                    let anchor_x = cx + sub_inner_r * parent_mid.sin();
                    let anchor_y = cy - sub_inner_r * parent_mid.cos();

                    let corner_sub_r = 10.0 * dpi_scale;
                    let gap_px = 3.0 * dpi_scale;

                    for (sub_k, child_act) in children.iter().enumerate() {
                        let sub_mid = layout.sub_mid_angle(parent_mid, sub_k, children.len(), sub_span);
                        let sub_outer_half = (sub_span / 2.0 - gap_px / (2.0 * sub_outer_r)).max(0.01);
                        let sub_inner_half = (sub_span / 2.0 - gap_px / (2.0 * sub_inner_r)).max(0.01);

                        let start_angle_outer = sub_mid - sub_outer_half;
                        let end_angle_outer = sub_mid + sub_outer_half;
                        let start_angle_inner = sub_mid - sub_inner_half;
                        let end_angle_inner = sub_mid + sub_inner_half;

                        let d_sub_out = (corner_sub_r / sub_outer_r).min(sub_outer_half * 0.45);
                        let d_sub_in = (corner_sub_r / sub_inner_r).min(sub_inner_half * 0.45);
                        let d_sub_r = corner_sub_r.min((sub_outer_r - sub_inner_r) * 0.35);

                        let pt_sub = |angle: f32, radius: f32| -> (f32, f32) {
                            let raw_x = cx + radius * angle.sin();
                            let raw_y = cy - radius * angle.cos();
                            (
                                anchor_x + (raw_x - anchor_x) * scale_sub,
                                anchor_y + (raw_y - anchor_y) * scale_sub,
                            )
                        };

                        let mut sub_pb = PathBuilder::new();
                        let p_start = pt_sub(start_angle_outer + d_sub_out, sub_outer_r);
                        sub_pb.move_to(p_start.0, p_start.1);

                        let steps = 16;
                        for s in 1..=steps {
                            let frac = s as f32 / steps as f32;
                            let a = (start_angle_outer + d_sub_out) + frac * ((end_angle_outer - d_sub_out) - (start_angle_outer + d_sub_out));
                            let p = pt_sub(a, sub_outer_r);
                            sub_pb.line_to(p.0, p.1);
                        }

                        let c_outer_end = pt_sub(end_angle_outer, sub_outer_r);
                        let p_line_top = pt_sub(end_angle_outer, sub_outer_r - d_sub_r);
                        sub_pb.quad_to(c_outer_end.0, c_outer_end.1, p_line_top.0, p_line_top.1);

                        let p_line_bot = pt_sub(end_angle_inner, sub_inner_r + d_sub_r);
                        sub_pb.line_to(p_line_bot.0, p_line_bot.1);

                        let c_inner_end = pt_sub(end_angle_inner, sub_inner_r);
                        let p_inner_arc_start = pt_sub(end_angle_inner - d_sub_in, sub_inner_r);
                        sub_pb.quad_to(c_inner_end.0, c_inner_end.1, p_inner_arc_start.0, p_inner_arc_start.1);

                        for s in 1..=steps {
                            let frac = s as f32 / steps as f32;
                            let a = (end_angle_inner - d_sub_in) - frac * ((end_angle_inner - d_sub_in) - (start_angle_inner + d_sub_in));
                            let p = pt_sub(a, sub_inner_r);
                            sub_pb.line_to(p.0, p.1);
                        }

                        let c_inner_start = pt_sub(start_angle_inner, sub_inner_r);
                        let p_start_bot = pt_sub(start_angle_inner, sub_inner_r + d_sub_r);
                        sub_pb.quad_to(c_inner_start.0, c_inner_start.1, p_start_bot.0, p_start_bot.1);

                        let p_start_top = pt_sub(start_angle_outer, sub_outer_r - d_sub_r);
                        sub_pb.line_to(p_start_top.0, p_start_top.1);

                        let c_outer_start = pt_sub(start_angle_outer, sub_outer_r);
                        sub_pb.quad_to(c_outer_start.0, c_outer_start.1, p_start.0, p_start.1);
                        sub_pb.close();

                        if let Some(sub_path) = sub_pb.finish() {
                            // Sub-slice drop shadow (ambient soft depth)
                            let mut shadow_paint = Paint::default();
                            shadow_paint.set_color_rgba8(0, 0, 0, (28.0 * t).round() as u8);
                            shadow_paint.anti_alias = true;
                            let shadow_trans = Transform::from_translate(0.0, 3.0 * dpi_scale);
                            pixmap.fill_path(&sub_path, &shadow_paint, FillRule::Winding, shadow_trans, None);

                            let mut shadow_paint2 = Paint::default();
                            shadow_paint2.set_color_rgba8(0, 0, 0, (14.0 * t).round() as u8);
                            shadow_paint2.anti_alias = true;
                            let shadow_trans2 = Transform::from_translate(0.0, 6.0 * dpi_scale);
                            pixmap.fill_path(&sub_path, &shadow_paint2, FillRule::Winding, shadow_trans2, None);

                            let is_sub_hovered = state.sub_hover == Some(sub_k);
                            let mut sub_paint = Paint::default();
                            if is_sub_hovered {
                                sub_paint.set_color(theme.slice_hover_fill);
                            } else {
                                sub_paint.set_color(theme.slice_fill);
                            }
                            sub_paint.anti_alias = true;
                            pixmap.fill_path(&sub_path, &sub_paint, FillRule::Winding, Transform::identity(), None);

                            let mut sub_stroke = Paint::default();
                            if is_sub_hovered {
                                sub_stroke.set_color(theme.slice_hover_border);
                            } else {
                                sub_stroke.set_color(theme.border_subtle);
                            }
                            sub_stroke.anti_alias = true;
                            let stroke = Stroke {
                                width: 1.0 * dpi_scale,
                                ..Default::default()
                            };
                            pixmap.stroke_path(&sub_path, &sub_stroke, &stroke, Transform::identity(), None);

                            let sub_mid_r = (sub_inner_r + sub_outer_r) / 2.0;
                            let raw_lx = cx + sub_mid_r * sub_mid.sin();
                            let raw_ly = cy - sub_mid_r * sub_mid.cos();
                            let lx = anchor_x + (raw_lx - anchor_x) * scale_sub;
                            let ly = anchor_y + (raw_ly - anchor_y) * scale_sub;

                            let label_color = if is_sub_hovered {
                                theme.text_on_accent
                            } else {
                                theme.text_primary
                            };

                            let is_missing = state.is_plugin_missing(*child_act);
                            let font_size = 18.5 * dpi_scale;

                            if is_missing {
                                let label = child_act.label();
                                let text_w = fonts.measure_text(label, font_size, true);
                                let arrow_w = 8.0 * dpi_scale;
                                let gap = 5.0 * dpi_scale;
                                let total_w = text_w + gap + arrow_w;

                                let text_cx = lx - (total_w / 2.0) + (text_w / 2.0);
                                fonts.draw_text_centered(
                                    pixmap,
                                    label,
                                    text_cx,
                                    ly,
                                    font_size,
                                    label_color,
                                    true,
                                );

                                let ar_x = lx + (total_w / 2.0) - (arrow_w / 2.0);
                                let ar_y = ly;
                                let stem_top = ar_y - 5.5 * dpi_scale;
                                let stem_bot = ar_y + 5.5 * dpi_scale;
                                let wing_dx = 3.8 * dpi_scale;
                                let wing_dy = 3.8 * dpi_scale;

                                let mut ar_pb = PathBuilder::new();
                                ar_pb.move_to(ar_x, stem_top);
                                ar_pb.line_to(ar_x, stem_bot);
                                ar_pb.move_to(ar_x - wing_dx, stem_bot - wing_dy);
                                ar_pb.line_to(ar_x, stem_bot);
                                ar_pb.line_to(ar_x + wing_dx, stem_bot - wing_dy);

                                if let Some(ar_path) = ar_pb.finish() {
                                    let mut ar_paint = Paint::default();
                                    ar_paint.set_color(label_color);
                                    ar_paint.anti_alias = true;
                                    let stroke = Stroke {
                                        width: 2.0 * dpi_scale,
                                        line_cap: LineCap::Round,
                                        line_join: LineJoin::Round,
                                        ..Default::default()
                                    };
                                    pixmap.stroke_path(&ar_path, &ar_paint, &stroke, Transform::identity(), None);
                                }
                            } else {
                                fonts.draw_text_centered(
                                    pixmap,
                                    child_act.label(),
                                    lx,
                                    ly,
                                    font_size,
                                    label_color,
                                    true,
                                );
                            }
                        }
                    }
                }
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
        let default_title_text = match &state.file_label {
            Some(lbl) => lbl.clone(),
            None => {
                if state.file_count == 1 {
                    "1 File".to_string()
                } else {
                    format!("{} Files", state.file_count)
                }
            }
        };

        let is_hovered_missing = if ch <= 0.4 {
            if let Some(sub_idx) = state.sub_hover {
                if let Some(group_idx) = state.expanded_group {
                    if let Some(action) = state.actions.get(group_idx) {
                        if let Some(children) = crate::smart_filter::get_action_children(*action) {
                            if let Some(child_act) = children.get(sub_idx) {
                                state.is_plugin_missing(*child_act)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else if let Some(idx) = state.hovered_slice {
                if let Some(action) = state.actions.get(idx) {
                    state.is_plugin_missing(*action)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        let subtitle = if ch > 0.4 {
            "Cancel".to_string()
        } else if let Some(sub_idx) = state.sub_hover {
            if let Some(group_idx) = state.expanded_group {
                if let Some(action) = state.actions.get(group_idx) {
                    if let Some(children) = crate::smart_filter::get_action_children(*action) {
                        if let Some(child_act) = children.get(sub_idx) {
                            child_act.center_prompt().to_string()
                        } else {
                            "Drop to Convert".to_string()
                        }
                    } else {
                        "Drop to Convert".to_string()
                    }
                } else {
                    "Drop to Convert".to_string()
                }
            } else {
                "Drop to Convert".to_string()
            }
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
        let tit_c = Color::from_rgba8(
            (tit_r * 255.0).round() as u8,
            (tit_g * 255.0).round() as u8,
            (tit_b * 255.0).round() as u8,
            (tit_a * 255.0).round() as u8,
        );

        let sub_r = theme.text_secondary.red() + ch * (theme.danger_text.red() - theme.text_secondary.red());
        let sub_g = theme.text_secondary.green() + ch * (theme.danger_text.green() - theme.text_secondary.green());
        let sub_b = theme.text_secondary.blue() + ch * (theme.danger_text.blue() - theme.text_secondary.blue());
        let sub_a = theme.text_secondary.alpha() + ch * (theme.danger_text.alpha() - theme.text_secondary.alpha());
        let sub_c = Color::from_rgba8(
            (sub_r * 255.0).round() as u8,
            (sub_g * 255.0).round() as u8,
            (sub_b * 255.0).round() as u8,
            (sub_a * 255.0).round() as u8,
        );

        if is_hovered_missing {
            fonts.draw_text_centered(
                pixmap,
                "Plugin",
                cx,
                cy - 10.5 * dpi_scale,
                16.5 * dpi_scale,
                theme.danger_text,
                true,
            );
            fonts.draw_text_centered(
                pixmap,
                "Required",
                cx,
                cy + 10.5 * dpi_scale,
                16.5 * dpi_scale,
                theme.danger_text,
                true,
            );
        } else {
            let mut title_font_size = 16.5 * dpi_scale;
            let max_title_w = 88.0 * dpi_scale;
            let measured_title_w = fonts.measure_text(&default_title_text, title_font_size, true);
            if measured_title_w > max_title_w && measured_title_w > 0.0 {
                let scale_ratio = (max_title_w / measured_title_w).max(0.65);
                title_font_size *= scale_ratio;
            }
            let sub_font_size = 14.5 * dpi_scale;
            let max_sub_w = 90.0 * dpi_scale;

            let sub_lines = wrap_center_subtitle(fonts, &subtitle, sub_font_size, max_sub_w);
            let sub_line_count = sub_lines.len().max(1) as f32;
            let gap = 5.0 * dpi_scale;
            let sub_line_h = 16.0 * dpi_scale;
            let total_h = title_font_size + gap + sub_line_count * sub_line_h;

            let top_y = cy - (total_h / 2.0);
            let title_y = top_y + (title_font_size / 2.0);
            let sub_y0 = top_y + title_font_size + gap + (sub_line_h / 2.0);

            fonts.draw_text_centered(
                pixmap,
                &default_title_text,
                cx,
                title_y,
                title_font_size,
                tit_c,
                true,
            );

            for (i, line) in sub_lines.iter().enumerate() {
                let line_y = sub_y0 + (i as f32) * sub_line_h;
                fonts.draw_text_centered(
                    pixmap,
                    line,
                    cx,
                    line_y,
                    sub_font_size,
                    sub_c,
                    false,
                );
            }
        }
    }
}

fn wrap_center_subtitle<'a>(fonts: &FontContext, text: &'a str, font_size: f32, max_w: f32) -> Vec<&'a str> {
    if text.is_empty() {
        return Vec::new();
    }
    let total_w = fonts.measure_text(text, font_size, false);
    if total_w <= max_w {
        return vec![text];
    }

    let mut lines: Vec<&'a str> = Vec::with_capacity(3);
    let mut line_start = 0;
    let mut last_space = 0;

    for (idx, ch) in text.char_indices() {
        if ch.is_whitespace() {
            let candidate = &text[line_start..idx];
            if fonts.measure_text(candidate, font_size, false) > max_w && last_space > line_start {
                lines.push(&text[line_start..last_space]);
                line_start = last_space + 1;
            }
            last_space = idx;
        }
    }

    if line_start < text.len() {
        let remainder = &text[line_start..];
        if fonts.measure_text(remainder, font_size, false) > max_w && last_space > line_start {
            lines.push(&text[line_start..last_space]);
            lines.push(&text[last_space + 1..]);
        } else {
            lines.push(remainder);
        }
    }

    lines
}

fn wrap_failure_message(fonts: &FontContext, msg: &str, font_size: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in msg.lines() {
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            continue;
        }
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let mut current_line = String::new();
        for word in words {
            let candidate = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };

            if fonts.measure_text(&candidate, font_size, false) <= max_w {
                current_line = candidate;
            } else {
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        lines.push("Conversion failed".to_string());
    }
    lines
}

/// Hit-tests whether a click relative to the HUD center hits the progress plate (allowing dragging).
pub fn hit_test_progress(rel_x: f32, rel_y: f32, dpi_scale: f32) -> bool {
    let half_w = 70.0 * dpi_scale;
    let half_h = 70.0 * dpi_scale;
    rel_x.abs() <= half_w && rel_y.abs() <= half_h
}

/// Traces a partial rounded rectangle perimeter starting from top-center (cx, cy - h/2) clockwise
/// corresponding to progress fraction `progress` in [0.0, 1.0].
fn push_rounded_square_progress_arc(
    pb: &mut PathBuilder,
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    r: f32,
    progress: f32,
) {
    let f = progress.clamp(0.0, 1.0);
    if f <= 0.001 {
        return;
    }

    let left = cx - w / 2.0;
    let right = cx + w / 2.0;
    let top = cy - h / 2.0;
    let bottom = cy + h / 2.0;
    let r = r.min(w / 2.0).min(h / 2.0);

    let l_top_half = (w / 2.0 - r).max(0.0);
    let l_vert = (h - 2.0 * r).max(0.0);
    let l_horiz = (w - 2.0 * r).max(0.0);
    let c_corner = std::f32::consts::FRAC_PI_2 * r;

    let total_p = 2.0 * l_top_half + 2.0 * l_vert + l_horiz + 4.0 * c_corner;
    let mut d = f * total_p;

    pb.move_to(cx, top);

    // 1. Top-right straight segment: from (cx, top) to (right - r, top)
    if d <= l_top_half {
        pb.line_to(cx + d, top);
        return;
    }
    pb.line_to(right - r, top);
    d -= l_top_half;

    // Helper to draw partial or full corner arc
    let draw_arc = |pb: &mut PathBuilder, ccx: f32, ccy: f32, start_angle: f32, rem_d: &mut f32| -> bool {
        let arc_len = c_corner;
        let arc_sweep = if *rem_d <= arc_len {
            let frac = *rem_d / arc_len.max(0.001);
            *rem_d = 0.0;
            frac * std::f32::consts::FRAC_PI_2
        } else {
            *rem_d -= arc_len;
            std::f32::consts::FRAC_PI_2
        };

        let steps = 8;
        for i in 1..=steps {
            let a = start_angle + arc_sweep * (i as f32 / steps as f32);
            pb.line_to(ccx + r * a.cos(), ccy + r * a.sin());
        }

        *rem_d == 0.0
    };

    // 2. Top-right corner: center (right - r, top + r), -PI/2 to 0
    if draw_arc(pb, right - r, top + r, -std::f32::consts::FRAC_PI_2, &mut d) {
        return;
    }

    // 3. Right vertical: from (right, top + r) to (right, bottom - r)
    if d <= l_vert {
        pb.line_to(right, top + r + d);
        return;
    }
    pb.line_to(right, bottom - r);
    d -= l_vert;

    // 4. Bottom-right corner: center (right - r, bottom - r), 0 to PI/2
    if draw_arc(pb, right - r, bottom - r, 0.0, &mut d) {
        return;
    }

    // 5. Bottom horizontal: from (right - r, bottom) to (left + r, bottom)
    if d <= l_horiz {
        pb.line_to(right - r - d, bottom);
        return;
    }
    pb.line_to(left + r, bottom);
    d -= l_horiz;

    // 6. Bottom-left corner: center (left + r, bottom - r), PI/2 to PI
    if draw_arc(pb, left + r, bottom - r, std::f32::consts::FRAC_PI_2, &mut d) {
        return;
    }

    // 7. Left vertical: from (left, bottom - r) to (left, top + r)
    if d <= l_vert {
        pb.line_to(left, bottom - r - d);
        return;
    }
    pb.line_to(left, top + r);
    d -= l_vert;

    // 8. Top-left corner: center (left + r, top + r), PI to 3*PI/2
    if draw_arc(pb, left + r, top + r, std::f32::consts::PI, &mut d) {
        return;
    }

    // 9. Top-left straight segment: from (left + r, top) to (cx, top)
    if d <= l_top_half {
        pb.line_to(left + r + d, top);
        return;
    }
    pb.line_to(cx, top);
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
    let err_font_size = 13.5 * dpi_scale;
    let max_text_w = 260.0 * dpi_scale;
    let (plate_w, plate_h, is_failure, wrapped_failure_lines) = match tracker.state() {
        crate::progress_ring::ProgressState::Failure(msg) => {
            let wrapped = wrap_failure_message(fonts, msg, err_font_size, max_text_w);
            let mut longest = 0.0f32;
            for line in &wrapped {
                let w = fonts.measure_text(line, err_font_size, false);
                if w > longest {
                    longest = w;
                }
            }
            let plate_w = (longest + 48.0 * dpi_scale).clamp(180.0 * dpi_scale, pixmap.width() as f32 - 32.0 * dpi_scale);
            let line_count = wrapped.len().clamp(1, 4);
            let plate_h = 56.0 * dpi_scale + (line_count as f32) * 19.0 * dpi_scale;
            (plate_w, plate_h, true, wrapped)
        }
        _ => {
            if tracker.is_detailed() {
                (108.0 * dpi_scale, 108.0 * dpi_scale, false, Vec::new())
            } else {
                (88.0 * dpi_scale, 88.0 * dpi_scale, false, Vec::new())
            }
        }
    };

    let half_w = plate_w / 2.0;
    let half_h = plate_h / 2.0;
    let corner_r = if tracker.is_detailed() { 20.0 * dpi_scale } else { 18.0 * dpi_scale };
    let track_radius = if tracker.is_detailed() { 44.0 * dpi_scale } else { 35.0 * dpi_scale };
    let stroke_w = if tracker.is_detailed() { 4.5 * dpi_scale } else { 4.8 * dpi_scale };

    // 1. Draw Plate Soft Shadow
    for i in 1..=4 {
        let shadow_margin = (i as f32) * 1.5 * dpi_scale;
        let mut shadow_pb = PathBuilder::new();
        push_rounded_rect(
            &mut shadow_pb,
            cx - half_w - shadow_margin,
            cy - half_h - shadow_margin + 2.0 * dpi_scale,
            plate_w + shadow_margin * 2.0,
            plate_h + shadow_margin * 2.0,
            corner_r + shadow_margin,
        );
        if let Some(shadow_path) = shadow_pb.finish() {
            let mut shadow_paint = Paint::default();
            shadow_paint.set_color_rgba8(0, 0, 0, 8);
            shadow_paint.anti_alias = true;
            pixmap.fill_path(&shadow_path, &shadow_paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    // 2. Draw Solid Plate Background
    let mut plate_pb = PathBuilder::new();
    push_rounded_rect(
        &mut plate_pb,
        cx - half_w,
        cy - half_h,
        plate_w,
        plate_h,
        corner_r,
    );
    if let Some(plate_path) = plate_pb.finish() {
        let mut plate_paint = Paint::default();
        if theme.is_dark {
            plate_paint.set_color_rgba8(32, 32, 30, 255);
        } else {
            plate_paint.set_color_rgba8(255, 255, 255, 255);
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

    // 3. Draw Background Track (for non-failure progress)
    if !is_failure {
        let mut bg_track_pb = PathBuilder::new();
        if tracker.is_detailed() {
            let track_w = 88.0 * dpi_scale;
            let track_h = 88.0 * dpi_scale;
            let track_r = 14.0 * dpi_scale;
            push_rounded_rect(&mut bg_track_pb, cx - track_w / 2.0, cy - track_h / 2.0, track_w, track_h, track_r);
        } else {
            bg_track_pb.push_circle(cx, cy, track_radius);
        }
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
    }

    // 4. Progress Arc or Outcome
    match tracker.state() {
        crate::progress_ring::ProgressState::Converting(_) | crate::progress_ring::ProgressState::Success => {
            let f = tracker.shown_progress().clamp(0.0, 1.0);

            // Draw Green Loader Progress Bar on Perimeter (Rounded Square in Detailed mode, Circle in Standard mode)
            if f > 0.005 {
                let mut arc_pb = PathBuilder::new();
                if tracker.is_detailed() {
                    let track_w = 88.0 * dpi_scale;
                    let track_h = 88.0 * dpi_scale;
                    let track_r = 14.0 * dpi_scale;
                    push_rounded_square_progress_arc(&mut arc_pb, cx, cy, track_w, track_h, track_r, f);
                } else {
                    let arc_steps = (64.0 * f).ceil().max(2.0) as usize;
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
            } else if tracker.is_detailed() {
                // 1. Green percentage text
                let pct_text = format!("{:.0}%", (f * 100.0).clamp(0.0, 100.0));
                fonts.draw_text_centered(
                    pixmap,
                    &pct_text,
                    cx,
                    cy - 14.0 * dpi_scale,
                    20.0 * dpi_scale,
                    Color::from_rgba8(46, 213, 115, 255), // Emerald Green
                    true,
                );

                // 2. Live approximate completion time text
                let eta_str = tracker.eta_text();
                let eta_color = if theme.is_dark {
                    Color::from_rgba8(168, 168, 168, 255)
                } else {
                    Color::from_rgba8(100, 100, 100, 255)
                };
                fonts.draw_text_centered(
                    pixmap,
                    &eta_str,
                    cx,
                    cy + 4.5 * dpi_scale,
                    12.5 * dpi_scale,
                    eta_color,
                    false,
                );

                // 3. Clickable STOP button
                let stop_y = cy + 22.0 * dpi_scale;
                let pill_w = 44.0 * dpi_scale;
                let pill_h = 17.0 * dpi_scale;
                let mut pill_pb = PathBuilder::new();
                push_rounded_rect(
                    &mut pill_pb,
                    cx - pill_w / 2.0,
                    stop_y - pill_h / 2.0 + 1.0 * dpi_scale,
                    pill_w,
                    pill_h,
                    5.0 * dpi_scale,
                );
                if let Some(pill_path) = pill_pb.finish() {
                    let mut pill_paint = Paint::default();
                    if theme.is_dark {
                        pill_paint.set_color_rgba8(239, 68, 68, 42);
                    } else {
                        pill_paint.set_color_rgba8(239, 68, 68, 28);
                    }
                    pill_paint.anti_alias = true;
                    pixmap.fill_path(&pill_path, &pill_paint, FillRule::Winding, Transform::identity(), None);
                }

                fonts.draw_text_centered(
                    pixmap,
                    "STOP",
                    cx,
                    stop_y + 1.0 * dpi_scale,
                    10.5 * dpi_scale,
                    Color::from_rgba8(239, 68, 68, 255), // Vibrant red
                    true,
                );
            } else {
                // Standard mode: Percentage in the center
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
        crate::progress_ring::ProgressState::Failure(_) => {
            let badge_y = cy - (plate_h / 2.0) + 24.0 * dpi_scale;

            // Failure red ring
            let mut ring_pb = PathBuilder::new();
            ring_pb.push_circle(cx, badge_y, 13.0 * dpi_scale);
            if let Some(ring_path) = ring_pb.finish() {
                let mut ring_paint = Paint::default();
                ring_paint.set_color(theme.danger_text);
                ring_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 2.2 * dpi_scale,
                    ..Default::default()
                };
                pixmap.stroke_path(&ring_path, &ring_paint, &stroke, Transform::identity(), None);
            }

            // Cross in center
            let mut x_pb = PathBuilder::new();
            let x_scale = dpi_scale * 0.75;
            x_pb.move_to(cx - 4.5 * x_scale, badge_y - 4.5 * x_scale);
            x_pb.line_to(cx + 4.5 * x_scale, badge_y + 4.5 * x_scale);
            x_pb.move_to(cx + 4.5 * x_scale, badge_y - 4.5 * x_scale);
            x_pb.line_to(cx - 4.5 * x_scale, badge_y + 4.5 * x_scale);
            if let Some(x_path) = x_pb.finish() {
                let mut x_paint = Paint::default();
                x_paint.set_color(theme.danger_text);
                x_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 2.0 * x_scale,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };
                pixmap.stroke_path(&x_path, &x_paint, &stroke, Transform::identity(), None);
            }

            // Draw wrapped lines centered with comfortable spacing and padding
            let text_start_y = badge_y + 20.0 * dpi_scale;
            for (i, line) in wrapped_failure_lines.iter().take(4).enumerate() {
                let line_y = text_start_y + (i as f32) * 18.5 * dpi_scale;
                fonts.draw_text_centered(
                    pixmap,
                    line,
                    cx,
                    line_y,
                    err_font_size,
                    if i == 0 { theme.danger_text } else { theme.text_primary },
                    i == 0,
                );
            }
        }
    }
}
