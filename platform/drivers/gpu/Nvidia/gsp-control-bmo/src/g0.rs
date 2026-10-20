//! G0: lo que el propietario eligio probar contra el 0x15 (GCB2c, 17-10).
//! Una palabra por experimento en `informe/estado/g0.txt`; el kernel lo lee al
//! arrancar y el DIRECTOR lo pregunta. Solo palabras conocidas: lo demas se
//! cuenta como desconocido y no activa nada.
//! [carril] VERDE
//! [consumo] NADA

/// E3: el GSP como OpenRM 570.144 -- reset al nucleo RISC-V y los buzones,
/// sin arrancar el falcon vacio.
pub const RISCV: u64 = 1 << 0;
/// E4: el booter en la misma llamada que `despertar`, sin hueco.
pub const SEGUIDO: u64 = 1 << 1;
/// E5: el RESET de los falcons como OpenRM, diez lecturas por flanco.
pub const RESET: u64 = 1 << 3;
/// E6: el reloj de la 3060 (PTIMER) en hora antes de FWSEC, como OpenRM.
pub const PTIMER: u64 = 1 << 4;
/// E7 (20-10): FWSEC-FRTS dentro de `despertar`, seguido del GSP y del
/// booter, como `kgspBootstrap_TU102` -- sin los ~522 ms ni el `save` de en
/// medio. Pide `seguido`.
pub const JUNTO: u64 = 1 << 5;
/// **El arranque de OpenRM entero** (PLAN_GPU_COMO_LINUX 4b, 20-10, el
/// propietario: *"inspirar por completo el OpenRM de ULTRA"*): lo que corre
/// SIN `g0.txt`. `gpu g0 normal` vuelve al camino de antes. PTIMER no entra:
/// E6 tiene un fallo sin informe.
pub const OPENRM: u64 = RISCV | SEGUIDO | RESET | JUNTO;
/// Lo que de verdad corrio, puesto por el kernel: un experimento pedido y
/// no usado (el GSP no llego a despertarse) no cuenta como probado.
pub const RISCV_USADO: u64 = RISCV << 8;
pub const SEGUIDO_USADO: u64 = SEGUIDO << 8;
pub const RESET_USADO: u64 = RESET << 8;
pub const PTIMER_USADO: u64 = PTIMER << 8;
pub const JUNTO_USADO: u64 = JUNTO << 8;
/// Palabras que no son ninguna de las conocidas (bits 16..23, saturado a 255).
pub const DESCONOCIDAS_SHIFT: u64 = 16;
/// El fichero existia y se leyo entero.
pub const FICHERO: u64 = 1 << 62;
/// El kernel ya miro (exista o no el fichero).
pub const MIRADO: u64 = 1 << 63;

/// Las palabras, en el orden en que se escriben. `sin-d` (E2, bit 2) se
/// retiro el 17-10 tras probarse: hoy cuenta como desconocida.
pub const PALABRAS: [(&[u8], u64); 5] =
    [(b"riscv", RISCV), (b"seguido", SEGUIDO), (b"reset", RESET), (b"ptimer", PTIMER), (b"junto", JUNTO)];

/// Las palabras del fichero: separadas por espacios, comas o saltos de linea,
/// sin distinguir mayusculas. Una linea que empieza por `#` es un comentario.
pub fn leer(texto: &[u8]) -> u64 {
    let mut modo = 0;
    let mut desconocidas: u64 = 0;
    for linea in texto.split(|&c| c == b'\n') {
        let linea = match linea.iter().position(|&c| c == b'#') {
            Some(i) => &linea[..i],
            None => linea,
        };
        for palabra in linea.split(|&c| c == b' ' || c == b',' || c == b'\t' || c == b'\r') {
            if palabra.is_empty() || palabra.eq_ignore_ascii_case(b"normal") {
                continue;
            }
            if palabra.eq_ignore_ascii_case(b"openrm") {
                modo |= OPENRM;
                continue;
            }
            match PALABRAS.iter().find(|(p, _)| palabra.eq_ignore_ascii_case(p)) {
                Some((_, bit)) => modo |= bit,
                None => desconocidas = (desconocidas + 1).min(255),
            }
        }
    }
    modo | desconocidas << DESCONOCIDAS_SHIFT
}

/// Cuantas palabras no se entendieron.
pub const fn desconocidas(modo: u64) -> u64 {
    (modo >> DESCONOCIDAS_SHIFT) & 0xFF
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn openrm_es_las_cuatro_y_normal_ninguna() {
        assert_eq!(leer(b"openrm"), RISCV | SEGUIDO | RESET | JUNTO);
        assert_eq!(leer(b"riscv seguido reset junto"), OPENRM);
        assert_eq!(leer(b"normal"), 0);
        assert_eq!(leer(b"# comentario\nopenrm ptimer"), OPENRM | PTIMER);
        assert_eq!(desconocidas(leer(b"sin-d")), 1);
    }

    #[test]
    fn los_bits_no_se_pisan() {
        let todos = [RISCV, SEGUIDO, RESET, PTIMER, JUNTO];
        for (i, a) in todos.iter().enumerate() {
            assert_eq!(a & (0xFF << DESCONOCIDAS_SHIFT), 0);
            assert_eq!((a << 8) & 0xFF, 0, "el USADO cae en 8..15");
            for b in &todos[i + 1..] {
                assert_eq!(a & b, 0);
            }
        }
    }
}
