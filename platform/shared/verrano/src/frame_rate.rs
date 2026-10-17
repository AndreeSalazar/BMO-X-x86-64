//! Ritmo de fotogramas nuevos aceptados, separado del refresco del monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample { pub milli_fps: u64, pub milliseconds: u64, pub intervals: u64, pub at: u64 }
pub struct FrameRate { start: u64, last: u64, intervals: u64, ready: bool, sample: Option<Sample> }
impl FrameRate {
    pub const fn new() -> Self { Self { start: 0, last: 0, intervals: 0, ready: false, sample: None } }
    pub fn observe(&mut self, now: u64) {
        if !self.ready || now < self.last || now - self.last > 1000 {
            self.start = now; self.last = now; self.intervals = 0; self.ready = true; return;
        }
        self.last = now;
        self.intervals += 1;
        let elapsed = now - self.start;
        if elapsed >= 1000 {
            self.sample = Some(Sample { milli_fps: self.intervals.saturating_mul(1_000_000) / elapsed,
                milliseconds: elapsed, intervals: self.intervals, at: now });
            self.start = now; self.intervals = 0;
        }
    }
    pub fn sample(&self) -> Option<Sample> { self.sample }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sixty_intervals_take_a_second_without_counting_the_first_frame_twice() {
        let mut r = FrameRate::new();
        for n in 0..=60 { r.observe(n * 1000 / 60); }
        assert_eq!(r.sample().unwrap().milli_fps, 60_000);
        assert_eq!(r.sample().unwrap().intervals, 60);
    }
    #[test]
    fn idle_is_not_reported_as_game_rendering_speed() {
        let mut r = FrameRate::new(); r.observe(0); r.observe(20_000);
        assert!(r.sample().is_none());
        for n in 1..=50 { r.observe(20_000 + 20 * n); }
        assert_eq!(r.sample().unwrap().milli_fps, 50_000);
    }
}
