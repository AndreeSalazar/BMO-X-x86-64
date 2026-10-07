//! **LA LIBRETA DE LA 3060, en el PEGAMENTO** (9d, 06-10) -- la mitad del
//! kernel de lo que PROTON-X empezo en `bmo-proton-x-sm86::libreta`: el
//! cuerpo de una app deja un TERMOMETRO en un registro (NaN = algo raro: una
//! salida NaN o infinita), y el pegamento que pone el KERNEL lo apunta.
//!
//! capa: puro -- instrucciones y una direccion; quien pone la palabra a cero
//! y la lee es el kernel (`gpu_trabajo/libreta.rs`)
//!
//! [eje]     SEGURIDAD -- AISLADA del todo (pedido del propietario: "AISLAR
//!           por completo"):
//!
//! ```text
//!    la direccion   UNA palabra fija del kernel, `LIBRETA` (en su pagina de
//!                   semaforos): la ponen dos MOV del pegamento, nunca la app
//!    lo que escribe una constante, `APUNTE`: la app no elige el valor
//!    la app         solo dice EN QUE REGISTRO esta su termometro (de SU
//!                   cuerpo: `receta::leer` lo mira) -- lo peor que puede
//!                   hacer es apuntarse raro a si misma
//!    otra app       nunca la ve: el kernel la pone a 0 antes de CADA dibujo
//!                   y la lee despues, y el resultado va en el `Ok` de la
//!                   receta de quien la mando (`cubo::LIBRETA_RARO`)
//!    otra GPU       nada de esto: una AMD tendria su libreta en su driver
//! ```
//!
//! # Lo que pega (tras el cuerpo; en el de pixel, antes de su EXIT)
//!
//! ```text
//!    FSETP.NEU P0, PT, Rt, Rt, PT     P0 = el termometro es NaN
//!    MOV  A, A+1 <- LIBRETA           la direccion, del kernel
//!    MOV  V <- APUNTE
//!    @P0 STG.E [A.64], V              lectura 1 (la espera el EXIT)
//! ```
//!
//! Los registros A y V son los del pegamento (`pegamento::propios`): el
//! cuerpo ya acabo y sus cargas llegaron, asi que se reusan. Solo
//! instrucciones de la tabla del juez: FSETP (la de E6), MOV, y el STG de
//! `sombreador` (el que ya corrio en el metal), con su guarda. El juez
//! (R9) pide 13 ciclos entre el FSETP y su guarda: los dan los tres MOV.

use crate::cubo::{con_control, mov, ALU};
use crate::lienzo::sombreador_va;
use crate::sombreador::SEMAFOROS;

/// **La palabra de la libreta**: en la pagina de semaforos, detras de la
/// tabla de VERRANO (`tuberia::TABLA`, +0x600).
pub const LIBRETA: u64 = SEMAFOROS + 0x700;

/// Lo que apunta la 3060 cuando algo sale raro (no 1: una palabra que se
/// reconoce en un volcado).
pub const APUNTE: u32 = 0x3060_09D0;

/// El predicado que usa (el cuerpo ya acabo: nadie lo lee despues).
const P: u64 = 0;

/// La barrera de LECTURA del STG: la 1, la de los AST (la espera el EXIT).
pub const B_LECTURA: u64 = 1;

/// `FSETP.NEU.AND P0, PT, Rt, Rt, PT`: P0 = `Rt` es NaN (desordenado
/// consigo mismo). La de `bmo_sm86::codifica::fsetp` (lo dice la prueba).
pub const fn fsetp_nan(t: u64, control: u64) -> (u64, u64) {
    (0x720B | t << 24 | t << 32, con_control(13 << 12 | P << 17 | 7 << 20 | 7 << 23, control))
}

/// El STG de `sombreador` (STG.E [R2.64], R5: el que corrio en el metal).
const STG_ORO: (u64, u64) = (0x0000_0005_0200_7986, 0x000F_E200_0C10_1904);

/// `@P0 STG.E [Ra.64], Rb`.
pub const fn stg_si(ra: u64, rb: u64, control: u64) -> (u64, u64) {
    let lo = STG_ORO.0 & !(0xF << 12) & !(0xFF << 24) & !(0xFF << 32);
    (lo | P << 12 | ra << 24 | rb << 32, con_control(STG_ORO.1, control))
}

/// El control del STG: 1 ciclo, el bit 4, sin barrera de escritura, la de
/// LECTURA `B_LECTURA` (como el AST).
const STG_CONTROL: u64 = 1 | 1 << 4 | 7 << 5 | B_LECTURA << 8;

/// Cuantas instrucciones pega.
pub const INSTRUCCIONES: usize = 5;

/// **Las instrucciones del apunte**, con el termometro en `t` y los
/// registros del pegamento `a` (par: `a`, `a + 1`) y `v`.
pub const fn apunte(t: u64, a: u64, v: u64) -> [(u64, u64); INSTRUCCIONES] {
    let va = sombreador_va(LIBRETA);
    [fsetp_nan(t, ALU), mov(a, va as u32), mov(a + 1, (va >> 32) as u32), mov(v, APUNTE), stg_si(a, v, STG_CONTROL)]
}

/// Si lo que se leyo de la palabra es un apunte.
pub const fn apuntado(palabra: u32) -> bool {
    palabra == APUNTE
}

/// **Leer la libreta** tras un dibujo pagado (el kernel, con el cerrojo
/// del GR aun tomado): si la 3060 apunto.
pub fn leer<R: crate::Registros>(r: &mut R) -> bool {
    apuntado(crate::copia::leer32(r, LIBRETA))
}

const _: () = assert!(LIBRETA >= crate::tuberia::TABLA + 8 && LIBRETA + 4 <= SEMAFOROS + 0x1000 && LIBRETA % 16 == 0);

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_sm86::codifica::{self as c, Cmp};

    /// El FSETP es el del codificador de la casa (E6), con su control.
    #[test]
    fn el_fsetp_es_el_del_codificador() {
        for t in [0u8, 4, 17, 60] {
            assert_eq!(fsetp_nan(t as u64, ALU), c::fsetp(0, Cmp::Neu, c::r(t), c::r(t), ALU));
        }
    }

    /// *** NO: el apunte sin los tres MOV en medio (el STG justo detras de
    /// su FSETP) lee el guarda antes de llegar: el juez lo dice (R9).
    #[test]
    fn un_guarda_sin_esperar_no_pasa() {
        use crate::sass::juez::{juzgar_drenado, Contexto, Regla, RESERVADOS};
        let a = apunte(4, 8, 10);
        let bien = [c::mov(4, c::Fuente::Imm(0x7FC0_0000), ALU), a[1], a[2], a[3], a[0], a[1], a[2], a[3], a[4], crate::cubo::EXIT_TRAS_AST];
        assert!(juzgar_drenado(&bien, &Contexto { registros: 12 + RESERVADOS, sph: None }).is_ok());
        let mal = [c::mov(4, c::Fuente::Imm(0x7FC0_0000), ALU), a[1], a[2], a[3], a[0], a[4], crate::cubo::EXIT_TRAS_AST];
        assert_eq!(juzgar_drenado(&mal, &Contexto { registros: 12 + RESERVADOS, sph: None }).unwrap_err().regla, Regla::R9PredicadoAntesDeLlegar);
    }

    /// La palabra: solo el APUNTE cuenta (ni 0, ni 1, ni otra cosa).
    #[test]
    fn solo_el_apunte_cuenta() {
        struct Vram(u32);
        impl crate::Registros for Vram {
            fn leer(&mut self, reg: u32) -> u32 {
                if reg == crate::vram::VENTANA_REG { 0 } else { self.0 }
            }
            fn escribir(&mut self, _: u32, _: u32) {}
        }
        assert!(leer(&mut Vram(APUNTE)));
        for x in [0, 1, APUNTE ^ 1, u32::MAX] {
            assert!(!leer(&mut Vram(x)), "{x:#x}");
        }
    }

    /// El STG: el de oro con otros registros, el guarda P0 y su control; y
    /// con PT y sus registros, el de oro tal cual (salvo el control).
    #[test]
    fn el_stg_es_el_de_oro() {
        let (lo, hi) = stg_si(2, 5, STG_CONTROL);
        assert_eq!(lo & 0xFFF, 0x986);
        assert_eq!((lo >> 12 & 0xF, lo >> 24 & 0xFF, lo >> 32 & 0xFF, lo >> 40), (0, 2, 5, 0), "@P0, [R2.64 + 0], R5");
        assert_eq!(hi & ((1 << 41) - 1), STG_ORO.1 & ((1 << 41) - 1), "el resto (.E, 32 bits) del de oro");
        assert_eq!(lo | 7 << 12, STG_ORO.0);
        let a = apunte(9, 40, 44);
        assert_eq!(a[1].0 >> 32, sombreador_va(LIBRETA) & 0xFFFF_FFFF);
        assert_eq!(a[2].0 >> 32, sombreador_va(LIBRETA) >> 32);
        assert_eq!(a[3].0 >> 32, APUNTE as u64);
    }
}
