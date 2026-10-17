//! Captura relativa: foco, contadores HID y entrega sin perder deltas.
#[derive(Default)]
pub struct Capture { owner: u32, last: u64, x: i64, y: i64 }
impl Capture {
    pub const fn new() -> Self { Self { owner: 0, last: 0, x: 0, y: 0 } }
    pub fn owner(&self) -> u32 { self.owner }
    pub fn sample(&mut self, owner: u32, counts: u64) {
        if owner != self.owner { self.owner = owner; self.x = 0; self.y = 0; }
        else if owner != 0 {
            self.x = self.x.saturating_add((counts as u32).wrapping_sub(self.last as u32) as i32 as i64);
            self.y = self.y.saturating_add(((counts >> 32) as u32).wrapping_sub((self.last >> 32) as u32) as i32 as i64);
        }
        self.last = counts;
    }
    pub fn motion(&self) -> (i16, i16) { (self.x.clamp(i16::MIN as i64, i16::MAX as i64) as i16, self.y.clamp(i16::MIN as i64, i16::MAX as i64) as i16) }
    pub fn delivered(&mut self, x: i16, y: i16) { self.x -= x as i64; self.y -= y as i64; }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_discards_old_motion_and_wraps_safely() {
        let mut c = Capture::new();
        c.sample(7, u64::MAX); c.sample(7, 0);
        assert_eq!(c.motion(), (1, 1));
        c.sample(0, 100); c.sample(9, 200);
        assert_eq!(c.motion(), (0, 0));
        c.sample(9, 210); assert_eq!(c.motion(), (10, 0));
    }
    #[test]
    fn full_mailbox_keeps_motion_until_delivered() {
        let mut c = Capture::new(); c.sample(7, 0); c.sample(7, 100_000);
        assert_eq!(c.motion(), (32767, 0));
        c.sample(7, 100_001); assert_eq!(c.motion(), (32767, 0));
        c.delivered(32767, 0); c.delivered(32767, 0); c.delivered(32767, 0);
        assert_eq!(c.motion(), (1700, 0));
    }
}
