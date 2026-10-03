//! **EL HUESPED**: lo que el codigo de pintar de una app pide de
//! `bmo-userland`, en el anfitrion. La letra de 8 x 16 es la MISMA tabla
//! (`include!` del fichero generado por `fontgen`), no una copia.

static FONT16: [[u8; 16]; 120] = include!("../../../../../Ultra_userspace/userland/src/font16_data.rs");
static FONT_EXTRA: [u8; 25] = include!("../../../../../Ultra_userspace/userland/src/font16_extra.rs");

pub const GLIFO_ANCHO: u32 = 8;
pub const GLIFO_ALTO: u32 = 16;

pub fn glyph_bits(c: u8) -> Option<&'static [u8; 16]> {
    if (32..=126).contains(&c) {
        return Some(&FONT16[c as usize - 32]);
    }
    FONT_EXTRA.iter().position(|&e| e == c).map(|i| &FONT16[95 + i])
}

pub const INFO_AUDIO_MEDIDOR: u64 = 0x8C;

/// Lo que contesta `info(INFO_AUDIO_MEDIDOR)`: la escena lo pone.
pub static MEDIDOR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn info(n: u64) -> u64 {
    if n == INFO_AUDIO_MEDIDOR {
        MEDIDOR.load(std::sync::atomic::Ordering::Relaxed)
    } else {
        0
    }
}
