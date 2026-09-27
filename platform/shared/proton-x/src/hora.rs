//! **La hora de Windows** (P4f2, 27-09): un FILETIME son centenas de
//! nanosegundos desde el 1 de enero de 1601, en UTC.
//!
//! ```text
//!    la fecha de la placa  -> segundos desde 1970 (dias del calendario civil)
//!    segundos + ns         -> FILETIME (116444736000000000 de 1601 a 1970)
//! ```
//!
//! Lo que no es Windows, dicho: la placa no dice su zona horaria. En una
//! maquina con Windows al lado, su reloj guarda la hora LOCAL (lo que Windows
//! hace por defecto); aqui se toma como UTC.

/// De 1601 a 1970, en centenas de ns.
pub const DE_1601_A_1970: u64 = 116_444_736_000_000_000;

/// Los segundos desde 1970 de una fecha del calendario civil (UTC).
pub fn segundos_unix(anio: u16, mes: u8, dia: u8, hora: u8, minuto: u8, segundo: u8) -> u64 {
    // Los dias desde 1970 (el algoritmo de "days from civil").
    let y = anio as i64 - (mes <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = mes as i64;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + dia as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias = era * 146_097 + doe - 719_468;
    (dias * 86_400 + hora as i64 * 3600 + minuto as i64 * 60 + segundo as i64) as u64
}

/// El FILETIME de `unix` segundos y `ns` nanosegundos despues.
pub fn filetime(unix: u64, ns: u64) -> u64 {
    DE_1601_A_1970 + unix * 10_000_000 + ns / 100
}
