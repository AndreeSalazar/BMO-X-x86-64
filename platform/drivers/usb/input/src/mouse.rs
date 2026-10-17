//! Mouse state tracking.

use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};

static MOUSE_X: AtomicI32 = AtomicI32::new(960);
static MOUSE_Y: AtomicI32 = AtomicI32::new(540);
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);

/// Accumulate mouse movement. Clamped to screen bounds.
pub fn accumulate_move(dx: i16, dy: i16, max_x: i32, max_y: i32) {
    let old_x = MOUSE_X.load(Ordering::Relaxed);
    let old_y = MOUSE_Y.load(Ordering::Relaxed);
    MOUSE_X.store((old_x + dx as i32).clamp(0, max_x), Ordering::Relaxed);
    MOUSE_Y.store((old_y - dy as i32).clamp(0, max_y), Ordering::Relaxed); // PS/2 Y is inverted
}

/// Set absolute mouse position.
pub fn set_position(x: i32, y: i32) {
    MOUSE_X.store(x, Ordering::Relaxed);
    MOUSE_Y.store(y, Ordering::Relaxed);
}

/// Update button state.
pub fn set_buttons(btns: u8) {
    MOUSE_BUTTONS.store(btns, Ordering::Relaxed);
}

/// Get current mouse position.
pub fn position() -> (i32, i32) {
    (MOUSE_X.load(Ordering::Relaxed), MOUSE_Y.load(Ordering::Relaxed))
}

/// Get current button state.
pub fn buttons() -> u8 {
    MOUSE_BUTTONS.load(Ordering::Relaxed)
}

/// Check if a button was pressed (for edge detection).
pub fn button_changed(old: u8, new: u8, mask: u8) -> bool {
    (old & mask) != (new & mask)
}

/// Contadores relativos independientes del cursor, con vuelta de u32.
pub const fn accumulate_relative(old: u64, dx: i16, dy: i16) -> u64 {
    let x = (old as u32).wrapping_add(dx as i32 as u32);
    let y = ((old >> 32) as u32).wrapping_add(dy as i32 as u32);
    x as u64 | (y as u64) << 32
}

#[cfg(test)]
mod relative_tests {
    #[test]
    fn motion_does_not_stop_at_a_screen_edge() {
        let mut count = 0;
        for _ in 0..10000 { count = super::accumulate_relative(count, 100, -100); }
        assert_eq!(count as u32, 1_000_000);
        assert_eq!((count >> 32) as u32 as i32, -1_000_000);
        assert_eq!(super::accumulate_relative(u64::MAX, 1, 1), 0);
    }
}
