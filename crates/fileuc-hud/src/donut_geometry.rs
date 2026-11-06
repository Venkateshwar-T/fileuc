use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitResult {
    Outside,
    CenterHole,
    Slice(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct DonutLayout {
    inner_r: f32,
    outer_r: f32,
    slice_count: usize,
}

impl DonutLayout {
    pub fn new(inner_r: f32, outer_r: f32, slice_count: usize) -> Self {
        Self {
            inner_r,
            outer_r,
            slice_count: slice_count.max(1),
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
        let r_sq = x * x + y * y;
        if r_sq < self.inner_r * self.inner_r {
            return HitResult::CenterHole;
        }
        if r_sq > self.outer_r * self.outer_r {
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
}
