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
/// E2: el disco Personal (D:) sin montar en este arranque.
pub const SIN_D: u64 = 1 << 2;
/// Lo que de verdad corrio, puesto por el kernel: un experimento pedido y
/// no usado (el GSP no llego a despertarse) no cuenta como probado.
pub const RISCV_USADO: u64 = RISCV << 8;
pub const SEGUIDO_USADO: u64 = SEGUIDO << 8;
pub const SIN_D_USADO: u64 = SIN_D << 8;
/// Palabras que no son ninguna de las tres (bits 16..23, saturado a 255).
pub const DESCONOCIDAS_SHIFT: u64 = 16;
/// El fichero existia y se leyo entero.
pub const FICHERO: u64 = 1 << 62;
/// El kernel ya miro (exista o no el fichero).
pub const MIRADO: u64 = 1 << 63;

/// Las tres palabras, en el orden en que se escriben.
pub const PALABRAS: [(&[u8], u64); 3] = [(b"sin-d", SIN_D), (b"riscv", RISCV), (b"seguido", SEGUIDO)];

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
