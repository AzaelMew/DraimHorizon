//! Frame-timed zoom, independent of game ticks and camera query frequency.
const FACTORS: [f32; 8] = [2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];
const RESPONSE_SECONDS: f32 = 0.12;

pub struct Zoom {
    level: usize,
    scale: f32,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            level: 3,
            scale: 1.0,
        }
    }
}

impl Zoom {
    pub fn frame(&mut self, elapsed: f32, allowed: bool, held: bool) {
        if !allowed {
            self.scale = 1.0;
            return;
        }
        let target = if held {
            FACTORS[self.level].recip()
        } else {
            1.0
        };
        if elapsed.is_finite() && elapsed > 0.0 {
            let alpha = 1.0 - (-elapsed.min(0.25) / RESPONSE_SECONDS).exp();
            self.scale += (target - self.scale) * alpha;
            if (target - self.scale).abs() < 0.00001 {
                self.scale = target;
            }
        }
    }

    pub fn scroll(&mut self, steps: i32, held: bool) -> bool {
        if !held {
            return false;
        }
        self.level = self
            .level
            .saturating_add_signed(steps as isize)
            .min(FACTORS.len() - 1);
        true
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }
}
