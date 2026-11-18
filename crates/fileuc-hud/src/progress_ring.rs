use crate::spring::Spring;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum ProgressState {
    Converting(f32),
    Success,
    Failure(String),
}

pub struct ProgressTracker {
    state: ProgressState,
    shown_progress: f32,
    target_progress: f32,
    badge_scale: Spring,
    outcome_popped: bool,
    pending_success: bool,
    completed_at: Option<Instant>,
    is_detailed: bool,
    start_time: Instant,
    smoothed_remaining_secs: f32,
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self::new_detailed(false)
    }

    pub fn new_detailed(is_detailed: bool) -> Self {
        let mut badge = Spring::new(0.0, 420.0, 16.0);
        badge.set_target(1.0);
        Self {
            state: ProgressState::Converting(0.0),
            shown_progress: 0.0,
            target_progress: 0.0,
            badge_scale: badge,
            outcome_popped: false,
            pending_success: false,
            completed_at: None,
            is_detailed,
            start_time: Instant::now(),
            smoothed_remaining_secs: 0.0,
        }
    }

    pub fn is_detailed(&self) -> bool {
        self.is_detailed
    }

    pub fn set_detailed(&mut self, detailed: bool) {
        self.is_detailed = detailed;
    }

    pub fn update(&mut self, fraction: f32) {
        let f = fraction.clamp(0.0, 1.0);
        self.target_progress = self.target_progress.max(f);
        if matches!(self.state, ProgressState::Converting(_)) {
            self.state = ProgressState::Converting(self.target_progress);
        }
    }

    pub fn mark_success(&mut self) {
        self.target_progress = 1.0;
        self.pending_success = true;
        self.state = ProgressState::Success;
    }

    pub fn mark_failure(&mut self, reason: String) {
        self.state = ProgressState::Failure(reason);
        self.completed_at = Some(Instant::now());
    }

    pub fn state(&self) -> &ProgressState {
        &self.state
    }

    pub fn shown_progress(&self) -> f32 {
        self.shown_progress
    }

    pub fn badge_scale(&self) -> f32 {
        if self.outcome_popped {
            self.badge_scale.value().max(0.0)
        } else {
            0.0
        }
    }

    pub fn is_popped(&self) -> bool {
        self.outcome_popped
    }

    pub fn eta_text(&self) -> String {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let f = self.target_progress.max(self.shown_progress).clamp(0.0, 1.0);
        if f < 0.02 || elapsed < 0.8 {
            return "Calculating...".to_string();
        }
        if f >= 0.995 {
            return "Finishing...".to_string();
        }

        let secs = if self.smoothed_remaining_secs > 0.0 {
            self.smoothed_remaining_secs
        } else {
            let raw_rate = f / elapsed.max(0.1);
            ((1.0 - f) / raw_rate.max(0.001)).max(0.0)
        };

        if secs < 1.0 {
            "~1 sec".to_string()
        } else if secs < 60.0 {
            format!("~{:.0} sec", secs)
        } else if secs < 3600.0 {
            let mins = (secs / 60.0).round().max(1.0);
            format!("~{:.0} min", mins)
        } else {
            let hrs = (secs / 3600.0).floor();
            let mins = ((secs % 3600.0) / 60.0).round();
            format!("~{:.0}h {:.0}m", hrs, mins)
        }
    }

    pub fn hit_test_stop(&self, rel_x: f32, rel_y: f32, _cx: f32, _cy: f32, dpi_scale: f32) -> bool {
        if !self.is_detailed {
            return false;
        }
        // In detailed mode, STOP button is centered horizontally at rel_x = 0,
        // vertically at rel_y = 22.0 * dpi_scale, width ~48px, height ~20px.
        let stop_center_y = 22.0 * dpi_scale;
        let half_w = 26.0 * dpi_scale;
        let half_h = 12.0 * dpi_scale;
        rel_x.abs() <= half_w && (rel_y - stop_center_y).abs() <= half_h
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;

        if self.pending_success {
            // Rapid, fluid completion sweep (sweeps remaining distance to 100% in ~220ms)
            let remaining = (1.0 - self.shown_progress).max(0.0);
            if remaining > 0.001 {
                let sweep_speed = (remaining / 0.22).clamp(3.8, 6.0);
                self.shown_progress = (self.shown_progress + sweep_speed * dt).min(1.0);
                animating = true;
            }

            if self.shown_progress >= 0.995 {
                self.shown_progress = 1.0;
                if !self.outcome_popped {
                    self.outcome_popped = true;
                    self.badge_scale.set_velocity(9.5);
                    self.completed_at = Some(Instant::now());
                    animating = true;
                }
            }
        } else if matches!(self.state, ProgressState::Converting(_)) {
            // Active smooth progression while converting:
            // Strictly follow real target_progress; never artificially fake-creep to 95%
            let diff = self.target_progress - self.shown_progress;
            if diff > 0.0001 {
                // Adaptive catch-up: scales smoothly with jump distance for liquid flow
                let track_rate = (diff * 5.0).clamp(0.20, 3.5);
                self.shown_progress = (self.shown_progress + track_rate * dt).min(self.target_progress);
                animating = true;
            } else if self.target_progress == 0.0 && self.shown_progress < 0.05 {
                // Subtle initial startup indicator capped strictly at 5%, never 95%
                self.shown_progress = (self.shown_progress + 0.015 * dt).min(0.05);
                animating = true;
            }

            // Update smoothed ETA with monotonic countdown damping
            let elapsed = self.start_time.elapsed().as_secs_f32();
            let f = self.target_progress.max(self.shown_progress).clamp(0.0, 1.0);
            if f >= 0.02 && elapsed >= 0.8 {
                let raw_rate = f / elapsed;
                let instant_rem = ((1.0 - f) / raw_rate.max(0.001)).max(0.0);
                if self.smoothed_remaining_secs <= 0.0 {
                    self.smoothed_remaining_secs = instant_rem;
                } else {
                    // Natural time countdown: decrease smoothed_remaining_secs by dt
                    let counted_down = (self.smoothed_remaining_secs - dt).max(1.0);
                    // Damped blend: if instant_rem increased (slowdown), blend very gently (5%); if decreased, blend faster (20%)
                    let alpha = if instant_rem > counted_down { 0.05 } else { 0.20 };
                    self.smoothed_remaining_secs = (1.0 - alpha) * counted_down + alpha * instant_rem;
                }
            }
        }

        if self.outcome_popped {
            if self.badge_scale.tick(dt) {
                animating = true;
            }
        }

        animating
    }

    pub fn fade_elapsed(&self) -> bool {
        match (&self.state, self.completed_at) {
            (ProgressState::Success, Some(at)) => {
                self.outcome_popped && at.elapsed().as_secs_f32() >= 0.9
            }
            (ProgressState::Failure(_), Some(at)) => at.elapsed().as_secs_f32() >= 3.5,
            _ => false,
        }
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}
