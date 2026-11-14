use std::f32::consts::TAU;

pub const SUB_GAP: f32 = 6.0;
pub const SUB_BAND: f32 = 68.0;
pub const SUB_OVERHANG: f32 = 0.25;
pub const MIN_SUB_WIDTH: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitResult {
    Outside,
    CenterHole,
    Slice(usize),
    SubSlice { group_idx: usize, sub_idx: usize },
}

#[derive(Debug, Clone, Copy)]
pub struct DonutLayout {
    inner_r: f32,
    outer_r: f32,
    slice_count: usize,
    dpi_scale: f32,
}

impl DonutLayout {
    pub fn new(inner_r: f32, outer_r: f32, slice_count: usize) -> Self {
        Self::with_dpi(inner_r, outer_r, slice_count, 1.0)
    }

    pub fn with_dpi(inner_r: f32, outer_r: f32, slice_count: usize, dpi_scale: f32) -> Self {
        Self {
            inner_r,
            outer_r,
            slice_count: slice_count.max(1),
            dpi_scale: if dpi_scale <= 0.0 { 1.0 } else { dpi_scale },
        }
    }

    pub fn inner_radius(&self) -> f32 {
        self.inner_r
    }

    pub fn outer_radius(&self) -> f32 {
        self.outer_r
    }

    pub fn slice_count(&self) -> usize {
        self.slice_count
    }

    pub fn slice_span(&self) -> f32 {
        TAU / (self.slice_count as f32)
    }

    pub fn slice_mid_angle(&self, index: usize) -> f32 {
        (index as f32) * self.slice_span()
    }

    pub fn hit_test(&self, x: f32, y: f32) -> HitResult {
        self.hit_test_with_slack(x, y, 0.0)
    }

    pub fn hit_test_with_slack(&self, x: f32, y: f32, slack: f32) -> HitResult {
        let r_sq = x * x + y * y;
        if r_sq < self.inner_r * self.inner_r {
            return HitResult::CenterHole;
        }
        let max_r = self.outer_r + slack;
        if r_sq > max_r * max_r {
            return HitResult::Outside;
        }

        // Clockwise from 12 o'clock:
        // atan2(x, -y) gives 0 at (0, -1), PI/2 at (1, 0), PI at (0, 1), -PI/2 at (-1, 0)
        let mut angle = x.atan2(-y);
        if angle < 0.0 {
            angle += TAU;
        }

        let span = self.slice_span();
        let idx = ((angle + span / 2.0) / span).floor() as usize % self.slice_count;
        HitResult::Slice(idx)
    }

    /// Calculates the inner and outer radii for the fanned-out sub-ring.
    pub fn sub_radii(&self, pop_out: f32) -> (f32, f32) {
        let inner = self.outer_r + pop_out + SUB_GAP * self.dpi_scale;
        let outer = inner + SUB_BAND * self.dpi_scale;
        (inner, outer)
    }

    /// Width (in radians) of each slice of a group's outer fan ring.
    pub fn sub_span(&self, count: usize) -> f32 {
        if count == 0 {
            return 0.0;
        }
        let (inner, outer) = self.sub_radii(7.0 * self.dpi_scale);
        let mid_r = (inner + outer) / 2.0;
        let parent_span = self.slice_span();
        let mut span = parent_span * (1.0 + 2.0 * SUB_OVERHANG) / (count as f32);
        if mid_r > 0.0 {
            span = span.max((MIN_SUB_WIDTH * self.dpi_scale) / mid_r);
        }
        span.min(TAU / (count as f32))
    }

    /// Mid-angle of slice `sub_idx` of a group fan centered on `parent_mid`.
    pub fn sub_mid_angle(&self, parent_mid: f32, sub_idx: usize, count: usize, sub_span: f32) -> f32 {
        parent_mid + (sub_idx as f32 - (count as f32 - 1.0) / 2.0) * sub_span
    }

    /// Hit-tests a point against an expanded group's fanned outer ring.
    /// Bridges the radial gap from parent slice outer radius outward, and ensures
    /// zero dead gap between child sub-slices.
    pub fn hit_test_sub(
        &self,
        x: f32,
        y: f32,
        group_idx: usize,
        count: usize,
        pop_out: f32,
    ) -> Option<usize> {
        if count == 0 {
            return None;
        }
        let (_inner, outer) = self.sub_radii(pop_out);
        let dist = (x * x + y * y).sqrt();
        // Bridge seamlessly from parent slice outer boundary outward past outer arc
        let min_r = self.outer_r + pop_out - 4.0 * self.dpi_scale;
        let max_r = outer + 16.0 * self.dpi_scale;
        if dist < min_r || dist > max_r {
            return None;
        }

        let parent_mid = self.slice_mid_angle(group_idx);
        let span = self.sub_span(count);

        let mut angle = x.atan2(-y);
        if angle < 0.0 {
            angle += TAU;
        }

        // Difference from parent mid-angle, wrapped to [-PI, PI]
        let mut delta = (angle - parent_mid) % TAU;
        if delta > std::f32::consts::PI {
            delta -= TAU;
        } else if delta < -std::f32::consts::PI {
            delta += TAU;
        }

        // Total angular half-span of the expanded group, with generous corridor slack (0.15 rad)
        let total_half_span = (count as f32 * span) / 2.0 + 0.15;
        if delta.abs() > total_half_span {
            return None;
        }

        // Clamp index so there is zero empty dead zone between child 0 and child 1!
        let sub = (delta / span + (count as f32) / 2.0).floor() as isize;
        let clamped = sub.clamp(0, (count - 1) as isize) as usize;
        Some(clamped)
    }
}
