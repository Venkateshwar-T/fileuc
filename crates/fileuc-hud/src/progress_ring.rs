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
}

impl ProgressTracker {
    pub fn new() -> Self {
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
        }
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
            let diff = self.target_progress - self.shown_progress;
            if diff > 0.001 {
                // Adaptive catch-up: scales smoothly with jump distance for liquid flow
                let track_rate = (diff * 5.0).clamp(0.20, 2.5);
                self.shown_progress = (self.shown_progress + track_rate * dt).min(self.target_progress);
                animating = true;
            } else if self.shown_progress < 0.95 {
                // Continuous subtle forward momentum so the ring never sits dead/frozen
                self.shown_progress = (self.shown_progress + 0.015 * dt).min(0.95);
                animating = true;
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
