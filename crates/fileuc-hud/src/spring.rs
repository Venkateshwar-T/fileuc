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

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let force = self.stiffness * (self.target - self.value) - self.damping * self.velocity;
        self.velocity += force * dt;
        self.value += self.velocity * dt;

        let moving = self.velocity.abs() > 0.0005 || (self.target - self.value).abs() > 0.0005;
        if !moving {
            self.value = self.target;
            self.velocity = 0.0;
        }
        moving
    }
}
