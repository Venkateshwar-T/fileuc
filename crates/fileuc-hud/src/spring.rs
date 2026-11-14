#[derive(Debug, Clone, Copy)]
pub struct Spring {
    value: f32,
    target: f32,
    velocity: f32,
    stiffness: f32,
    damping: f32,
}

impl Spring {
    pub fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
            stiffness,
            damping,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn set_velocity(&mut self, velocity: f32) {
        self.velocity = velocity;
    }

    pub fn set_damping(&mut self, damping: f32) {
        self.damping = damping;
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        // 4 sub-steps for ultra-smooth numerical stability regardless of frame delta jitter
        let substeps = 4;
        let sub_dt = dt / (substeps as f32);
        for _ in 0..substeps {
            let force = self.stiffness * (self.target - self.value) - self.damping * self.velocity;
            self.velocity += force * sub_dt;
            self.value += self.velocity * sub_dt;
        }

        let moving = self.velocity.abs() > 0.0005 || (self.target - self.value).abs() > 0.0005;
        if !moving {
            self.value = self.target;
            self.velocity = 0.0;
        }
        moving
    }
}
