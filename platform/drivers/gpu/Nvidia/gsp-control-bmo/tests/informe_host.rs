//! Ejecuta el adaptador REAL de Director contra una puerta y salida sinteticas.
//! No compila Director completo ni valida syscalls o registros del hardware.
//! [carril] VERDE
//! [consumo] NADA
#![forbid(unsafe_code)]
extern crate self as bmo_userland;
extern crate self as bmo_gpu_ga10x;
use std::cell::RefCell;

// Copias del ABI para la puerta simulada. El banco comprueba que sigan
// iguales al runtime real, para no aprobar con banderas inventadas.
macro_rules! abi {
    ($($name:ident : $ty:ty = $value:expr;)*) => {
        $(pub const $name: $ty = $value;)*
        #[test]
        fn simulated_door_matches_runtime_constants() {
            let source = include_str!("../../../../../../Ultra_userspace/userland/src/lib.rs");
            $(assert!(source.contains(concat!("pub const ", stringify!($name), ": ",
                stringify!($ty), " = ", stringify!($value), ";")), stringify!($name));)*
        }
    };
}
abi! {
    IOMMU_NO_BOOTER_MAL: u32 = 44;
    BUILD_VALIDO: u64 = 1 << 63;
    INFO_GPU_INTENTO: u64 = 0xDC;
    INFO_GPU_CHIP: u64 = 0x9A;
    GPU_HALLADA: u64 = 1 << 63;
    INFO_GPU_GSP: u64 = 0xBC;
    GSP_VALIDO: u64 = 1 << 63;
    INFO_GPU_GSP_HASH: u64 = 0xBD;
    INFO_GPU_LIBOS: u64 = 0xBE;
    LIBOS_COMPROBADO: u64 = 1 << 58;
    LIBOS_VALIDO: u64 = 1 << 63;
    INFO_GPU_DESPIERTO: u64 = 0xBF;
    DESPIERTO_GSP_ARRANCADO: u64 = 1 << 1;
    DESPIERTO_SEC2_ARRANCADO: u64 = 1 << 4;
    DESPIERTO_SEC2_PARADO: u64 = 1 << 5;
    DESPIERTO_VALIDO: u64 = 1 << 15;
    DESPIERTO_MOTIVO_SHIFT: u64 = 16;
    DESPIERTO_BUZON_SHIFT: u64 = 32;
    INFO_GPU_DESPIERTO_BUZON: u64 = 0xC0;
}

#[derive(Default)]
struct Door { attempt: u64, attempt_after: u64, attempt_reads: usize,
              raw: u64, init: Option<bool> }
thread_local! { static DOOR: RefCell<Door> = RefCell::new(Door::default()); }

pub fn info(selector: u64) -> u64 {
    DOOR.with(|door| {
        let mut d = door.borrow_mut();
        match selector {
            INFO_GPU_INTENTO => {
                d.attempt_reads += 1;
                if d.attempt_reads == 1 { d.attempt } else { d.attempt_after }
            }
            INFO_GPU_DESPIERTO => d.raw,
            INFO_GPU_CHIP => GPU_HALLADA | 0x176,
            INFO_GPU_GSP => GSP_VALIDO,
            INFO_GPU_GSP_HASH => 123,
            INFO_GPU_LIBOS => LIBOS_VALIDO | LIBOS_COMPROBADO,
            x if x == INFO_GPU_DESPIERTO_BUZON | 5 << 8 => 1 << 63 | 247011 << 32,
            x if x == INFO_GPU_DESPIERTO_BUZON | 6 << 8 => 0xBADF_1002_BADF_1002,
            x if x == INFO_GPU_DESPIERTO_BUZON | 8 << 8 => 0x10_BADF_5620,
            _ => panic!("consulta no preparada: {selector:x}"),
        }
    })
}

pub fn es_error_pri(value: u32) -> bool {
    value == u32::MAX || value >> 20 == 0xBAD
}

mod scene {
    pub mod output {
        #[derive(Default)]
        pub struct Output(pub Vec<u8>);
        impl Output {
            pub fn text(&mut self, s: &[u8]) { self.0.extend_from_slice(s); }
            pub fn byte(&mut self, b: u8) { self.0.push(b); }
            pub fn hex(&mut self, v: u64, n: usize) { self.text(format!("{v:0n$X}").as_bytes()); }
            pub fn dec(&mut self, v: u64) { self.text(v.to_string().as_bytes()); }
        }
    }
}
mod tabla {
    use super::scene::output::Output;
    pub fn campo(s: &mut Output, text: &[u8]) { s.text(text); s.text(b": "); }
    pub fn section(s: &mut Output, text: &[u8]) { s.text(text); s.byte(b'\n'); }
}
mod obs_build {
    pub fn kernel() -> u64 { crate::BUILD_VALIDO | 1 }
    pub fn escribir(s: &mut crate::scene::output::Output, v: u64) { s.hex(v, 16); }
}
mod gspinit {
    pub fn observado() -> Option<bool> { super::DOOR.with(|d| d.borrow().init) }
}
mod g0 { pub fn filas(_: &mut crate::scene::output::Output) {} }

#[path = "../../../../../../Ultra_userspace/services/director/src/commands/gspcontrol.rs"]
mod actual_report;

fn report(code: u32, reason: u32, attempt: u64, attempt_after: u64, halted: bool) -> String {
    DOOR.with(|d| *d.borrow_mut() = Door {
        attempt, attempt_after, attempt_reads: 0, init: Some(false),
        raw: DESPIERTO_VALIDO | DESPIERTO_GSP_ARRANCADO | DESPIERTO_SEC2_ARRANCADO
            | if halted { DESPIERTO_SEC2_PARADO } else { 0 }
            | (code as u64) << DESPIERTO_BUZON_SHIFT | (reason as u64) << DESPIERTO_MOTIVO_SHIFT,
    });
    let mut out = scene::output::Output::default();
    actual_report::report(&mut out);
    String::from_utf8(out.0).unwrap()
}

#[test]
fn report_separates_host_reason_firmware_return_and_pri_read() {
    let text = report(0x15, 44, 1, 1, true);
    assert!(text.contains("motivo 0x2C; SEC2 MAILBOX0 0x00000015"));
    assert!(text.contains("significado interno no verificado"));
    assert!(text.contains("buzon GSP con error PRI"));
    assert!(text.contains("atribuible al intento 1"));
    assert!(text.contains("intervalo CPU observado 247011 us"));
}

#[test]
fn retries_do_not_pair_cached_stop_with_the_current_live_mailbox() {
    let text = report(0x15, 44, 2, 2, true);
    assert!(text.contains("SIN sello de intento"));
    assert!(!text.contains("; GSP MAILBOX0/1"));
}

#[test]
fn equal_live_words_do_not_hide_an_attempt_change() {
    let text = report(0x15, 44, 1, 2, true);
    assert!(text.contains("estado: Desconocido"));
    assert!(text.contains("retorno booter: consulta cambio"));
}

#[test]
fn in_flight_mailbox_is_not_a_final_error() {
    let text = report(0x15, 0, 1, 1, false);
    assert!(text.contains("no se observo SEC2 parado"));
    assert!(!text.contains("SEC2 parado con retorno 0x15"));
}

#[test]
fn failed_read_and_contradiction_do_not_announce_success() {
    let unreadable = report(0xBADF_1002, 0, 1, 1, true);
    assert!(unreadable.contains("buzon no interpretable"));
    let contradictory = report(0, 44, 1, 1, true);
    assert!(contradictory.contains("estado: Fallido"));
    assert!(contradictory.contains("hechos no compatibles"));
}
