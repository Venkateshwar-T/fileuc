use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum ProgressState {
    Converting(f32),
    Success,
    Failure(String),
}

pub struct ProgressTracker {
    state: ProgressState,
    completed_at: Option<Instant>,
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            state: ProgressState::Converting(0.0),
            completed_at: None,
        }
    }

    pub fn update(&mut self, fraction: f32) {
        if matches!(self.state, ProgressState::Converting(_)) {
            self.state = ProgressState::Converting(fraction.clamp(0.0, 1.0));
        }
    }

    pub fn mark_success(&mut self) {
        self.state = ProgressState::Success;
        self.completed_at = Some(Instant::now());
    }

    pub fn mark_failure(&mut self, reason: String) {
        self.state = ProgressState::Failure(reason);
        self.completed_at = Some(Instant::now());
    }

    pub fn state(&self) -> &ProgressState {
        &self.state
    }

    pub fn fade_elapsed(&self) -> bool {
        match (&self.state, self.completed_at) {
            (ProgressState::Success, Some(at)) => at.elapsed().as_secs_f32() >= 1.5,
            (ProgressState::Failure(_), Some(at)) => at.elapsed().as_secs_f32() >= 3.0,
            _ => false,
        }
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}
