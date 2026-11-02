// Client-side estimate from server SetTime packets, never from render frames or the predicted clock.
use std::collections::VecDeque;
pub struct Sampler {
    points: VecDeque<(u64, i64)>,
    target: f64,
    pub frozen: bool,
}
impl Default for Sampler {
    fn default() -> Self {
        Self {
            points: VecDeque::new(),
            target: 20.0,
            frozen: false,
        }
    }
}
impl Sampler {
    pub fn rate(&mut self, rate: f32, frozen: bool) {
        self.target = if rate.is_finite() {
            f64::from(rate.clamp(1.0, 1000.0))
        } else {
            20.0
        };
        if self.frozen != frozen {
            self.points.clear();
        }
        self.frozen = frozen;
    }
    pub fn sample(&mut self, now: u64, ticks: i64) {
        if self.frozen {
            return;
        }
        if let Some(&(at, last)) = self.points.back() {
            if ticks < last || now < at || now.saturating_sub(at) > 60000 {
                self.points.clear();
            } else if ticks == last {
                return;
            }
        }
        self.points.push_back((now, ticks));
        while self.points.len() > 64
            || self
                .points
                .front()
                .is_some_and(|(at, _)| now.saturating_sub(*at) > 30000)
                && self.points.len() > 2
        {
            self.points.pop_front();
        }
    }
    pub fn value(&self, now: u64) -> Option<f64> {
        let &(end, ticks) = self.points.back()?;
        let &(start, first) = self.points.front()?;
        if self.frozen || now.saturating_sub(end) > 8000 || end.saturating_sub(start) < 500 {
            return None;
        }
        let delta = ticks.checked_sub(first)?;
        if delta <= 0 {
            return None;
        }
        Some((delta as f64 * 1000.0 / (end - start) as f64).clamp(0.0, self.target))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estimate_follows_server_time_not_fps_and_expires() {
        let mut t = Sampler::default();
        t.sample(1000, 20);
        assert!(t.value(1000).is_none());
        t.sample(2000, 40);
        assert_eq!(t.value(2000), Some(20.));
        t.sample(5000, 60);
        assert_eq!(t.value(5000), Some(10.));
        assert!(t.value(14000).is_none());
        t.sample(6000, 1);
        assert!(t.value(6000).is_none());
        t.rate(20., true);
        assert!(t.value(6000).is_none());
        t.rate(10., false);
        t.sample(7000, 20);
        t.sample(8000, 40);
        assert_eq!(t.value(8000), Some(10.));
    }
}
