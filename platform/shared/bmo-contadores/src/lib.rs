//! # **LOS CONTADORES DEL ZEN 3** (LB4 de `docs/plan/PLAN_LA_LENTE_BINARIA.md`,
//! C4 de `docs/plan/EL_FOCO.md`, 15-10)
//!
//! capa: puro -- numeros que entran y numeros que salen, probado en el anfitrion
//!
//! [carril]  VERDE     decide como se programa un contador y si lo contado
//!                     es lo que dice el papel
//! [cuesta]  DATO -- un contador mal puesto no rompe nada; un numero mal
//!           LEIDO burla: por eso cada evento lleva su FUENTE y su CONFIANZA, y
//!           el metal lo confirma antes de que nadie lo crea
//!
//! El Zen 3 cuenta lo que hace: instrucciones retiradas, saltos mal
//! predichos, cargas que fallan la L1. Cada contador se programa con un
//! `PERF_CTL` (que evento, que mascara, en que anillo) y se lee de su
//! `PERF_CTR` (48 bits). Esto es TODO lo que se decide; el kernel
//! (`cpu_vendor/ryzen_5_5600x/contadores.rs`) solo escribe y lee los MSR.
//!
//! ```text
//!    PERF_CTL   bits  7:0   evento[7:0]       35:32 evento[11:8]
//!                    15:8   mascara (umask)   16 USR  17 OS
//!                       22  EN                41:40 solo anfitrion/invitado
//!    MSR        extendidos (CPUID 0x80000001 ECX[23], PerfCtrExtCore):
//!               PERF_CTL k = 0xC001_0200 + 2k, PERF_CTR k = 0xC001_0201 + 2k,
//!               seis; sin ellos, los cuatro de siempre en 0xC001_0000/4
//! ```
//!
//! (AMD64 Architecture Programmer's Manual, vol. 2, 13.2 "Hardware
//! Performance Monitoring and Control".)
//!
//! ** LA REGLA DEL PLAN: primero MEDIR que cuenta cada contador en ESTE Ryzen
//! (el PPR dice, el metal confirma), como se hizo con las caches. Por eso cada
//! [`Evento`] dice de donde sale y si es [`Confianza::Firme`] o
//! [`Confianza::PorConfirmar`], y [`juzgar`] compara lo contado en un bucle
//! que se sabe de memoria con lo que tendria que salir.

#![cfg_attr(not(test), no_std)]

/// Los contadores extendidos (el Zen 3 tiene seis).
pub const PERF_CTL_EXT: u32 = 0xC001_0200;
pub const PERF_CTR_EXT: u32 = 0xC001_0201;
/// Los cuatro de siempre (sin `PerfCtrExtCore`).
pub const PERF_CTL_VIEJO: u32 = 0xC001_0000;
pub const PERF_CTR_VIEJO: u32 = 0xC001_0004;
/// CPUID 0x80000001 ECX[23]: hay contadores extendidos.
pub const BIT_EXTENDIDOS: u32 = 23;
/// Lo que mide un contador: 48 bits.
pub const MASCARA_CUENTA: u64 = (1 << 48) - 1;

/// Donde estan el control y la cuenta del contador `k`.
pub const fn msr(k: u32, extendidos: bool) -> (u32, u32) {
    if extendidos {
        (PERF_CTL_EXT + 2 * k, PERF_CTR_EXT + 2 * k)
    } else {
        (PERF_CTL_VIEJO + k, PERF_CTR_VIEJO + k)
    }
}

/// Cuantos contadores hay.
pub const fn cuantos(extendidos: bool) -> u32 {
    if extendidos { 6 } else { 4 }
}

/// En que anillo cuenta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anillo {
    /// Solo Ring 3 (un programa).
    Usuario,
    /// Solo Ring 0 (la calibracion del kernel).
    Kernel,
    Ambos,
}

/// **Un `PERF_CTL`** (el de un contador).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    /// 12 bits.
    pub evento: u16,
    pub mascara: u8,
    pub anillo: Anillo,
    pub encendido: bool,
}

impl Control {
    pub const fn de(e: &Evento, anillo: Anillo) -> Self {
        Control { evento: e.evento, mascara: e.mascara, anillo, encendido: true }
    }

    /// Lo que se escribe en el MSR.
    pub const fn palabra(&self) -> u64 {
        let (usr, os) = match self.anillo {
            Anillo::Usuario => (1, 0),
            Anillo::Kernel => (0, 1),
            Anillo::Ambos => (1, 1),
        };
        (self.evento as u64 & 0xFF)
            | (self.mascara as u64) << 8
            | usr << 16
            | os << 17
            | (self.encendido as u64) << 22
            | ((self.evento as u64 >> 8) & 0xF) << 32
    }

    /// Lo de [`Control::palabra`], al reves (lo que se lee del MSR).
    pub const fn leer(w: u64) -> Self {
        let anillo = match (w >> 16 & 1, w >> 17 & 1) {
            (1, 0) => Anillo::Usuario,
            (0, 1) => Anillo::Kernel,
            _ => Anillo::Ambos,
        };
        Control { evento: (w & 0xFF) as u16 | ((w >> 32 & 0xF) as u16) << 8, mascara: (w >> 8) as u8, anillo, encendido: w >> 22 & 1 != 0 }
    }

    /// El mismo, apagado (para dejar de contar sin perder la cuenta).
    pub const fn apagado(self) -> Self {
        Control { encendido: false, ..self }
    }
}

/// Cuanto se cree lo que dice el papel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confianza {
    /// El mismo numero en todas las familias Zen y en el `perf` de Linux
    /// (`cycles`, `instructions`, `branches`, `branch-misses`).
    Firme,
    /// Del PPR de la familia 19h y del `perf` de Linux, pero un umask mal
    /// leido cuenta otra cosa: hasta que el metal lo confirme, NO se cree.
    PorConfirmar,
}

/// **Un evento del Zen 3.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Evento {
    pub nombre: &'static str,
    pub evento: u16,
    pub mascara: u8,
    /// Que cuenta, en castellano.
    pub que: &'static str,
    pub fuente: &'static str,
    pub confianza: Confianza,
}

const fn ev(nombre: &'static str, evento: u16, mascara: u8, que: &'static str, fuente: &'static str, confianza: Confianza) -> Evento {
    Evento { nombre, evento, mascara, que, fuente, confianza }
}

/// **El catalogo**, en el orden de su indice. Uno que se retira no se reusa.
pub const EVENTOS: [Evento; 6] = [
    ev("ciclos", 0x076, 0x00, "ciclos de reloj sin estar parado (HLT)", "PPR 19h PMCx076 LsNotHaltedCyc; perf `cycles`", Confianza::Firme),
    ev("instrucciones", 0x0C0, 0x00, "instrucciones retiradas", "PPR 19h PMCx0C0 ExRetInstr; perf `instructions`", Confianza::Firme),
    ev("saltos", 0x0C2, 0x00, "saltos retirados", "PPR 19h PMCx0C2 ExRetBrn; perf `branches`", Confianza::Firme),
    ev("saltos fallados", 0x0C3, 0x00, "saltos retirados mal predichos", "PPR 19h PMCx0C3 ExRetBrnMisp; perf `branch-misses`", Confianza::Firme),
    ev("accesos L1D", 0x040, 0x00, "accesos a la cache de datos L1", "PPR 19h PMCx040 LsDcAccesses; perf `L1-dcache-loads` (0x0040)", Confianza::PorConfirmar),
    ev("fallos L1D", 0x060, 0xC8, "fallos de la L1 de datos que piden a la L2", "PPR 19h PMCx060 L2RequestG1 umask 0xC8; perf `L1-dcache-load-misses` (0xC860)", Confianza::PorConfirmar),
];

/// El evento de este nombre.
pub fn por_nombre(nombre: &str) -> Option<(usize, &'static Evento)> {
    EVENTOS.iter().enumerate().find(|(_, e)| e.nombre == nombre)
}

/// **Las pruebas de la calibracion**: bucles que se saben de memoria. El
/// kernel corre cada una con `n` vueltas y cuenta; [`juzgar`] dice si lo
/// contado es lo que tendria que salir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prueba {
    /// `1: add; dec; jnz 1b` -- 3 instrucciones y 1 salto por vuelta, el
    /// salto casi siempre bien predicho.
    Recta,
    /// Un salto que depende de un bit al azar (xorshift): la mitad fallados.
    Azar,
    /// Saltar por una permutacion de lineas de 64 B en 256 KiB (mas que la
    /// L1D de 32 KiB, menos que la L2 de 512): cada carga, casi siempre un
    /// fallo de la L1 que va a la L2. El prefetcher no adivina una
    /// permutacion.
    Persecucion,
}

/// Lo que se mide en cada prueba: `(prueba, evento)`.
pub const CALIBRACION: [(Prueba, usize); 8] = [
    (Prueba::Recta, 0),
    (Prueba::Recta, 1),
    (Prueba::Recta, 2),
    (Prueba::Recta, 3),
    (Prueba::Azar, 2),
    (Prueba::Azar, 3),
    (Prueba::Persecucion, 4),
    (Prueba::Persecucion, 5),
];

/// Lo que tendria que contar `evento` en `prueba` de `n` vueltas:
/// `(minimo, maximo)`, con holgura para lo que rodea al bucle (la entrada y
/// la salida: unas decenas) y para lo que el silicio hace de mas. `None`:
/// esa prueba no dice nada de ese evento.
pub fn esperado(prueba: Prueba, evento: usize, n: u64) -> Option<(u64, u64)> {
    let holgura = 1000;
    Some(match (prueba, EVENTOS.get(evento)?.nombre) {
        // Un ciclo por vuelta como poco (la cadena de `add`), y no mas de 8.
        (Prueba::Recta, "ciclos") => (n / 2, 8 * n + holgura),
        (Prueba::Recta, "instrucciones") => (3 * n, 3 * n + holgura),
        (Prueba::Recta, "saltos") => (n, n + holgura),
        (Prueba::Recta, "saltos fallados") => (0, n / 1000 + holgura),
        // Dos saltos por vuelta (el del azar y el del bucle).
        (Prueba::Azar, "saltos") => (2 * n, 2 * n + holgura),
        // El del azar falla la mitad: entre un cuarto y tres cuartos.
        (Prueba::Azar, "saltos fallados") => (n / 4, 3 * n / 4 + holgura),
        (Prueba::Persecucion, "accesos L1D") => (n, 4 * n + holgura),
        (Prueba::Persecucion, "fallos L1D") => (n / 2, 2 * n + holgura),
        _ => return None,
    })
}

/// **El veredicto** de una fila de la calibracion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Veredicto {
    /// Cuenta lo que dice el papel.
    Cuenta,
    /// Cuenta, pero fuera de lo esperado: NO se cree.
    Otra { minimo: u64, maximo: u64 },
    /// No conto nada: el evento no existe aqui, o no se encendio.
    Nada,
}

/// **Juzgar** lo contado.
pub fn juzgar(prueba: Prueba, evento: usize, n: u64, contado: u64) -> Option<Veredicto> {
    let (minimo, maximo) = esperado(prueba, evento, n)?;
    Some(if contado == 0 && minimo > 0 {
        Veredicto::Nada
    } else if (minimo..=maximo).contains(&contado) {
        Veredicto::Cuenta
    } else {
        Veredicto::Otra { minimo, maximo }
    })
}

/// Una fila, para viajar en una palabra: la cuenta (48 bits), la prueba
/// (bits 48..50), el evento (52..56) y el veredicto (60..62: 1 cuenta, 2
/// otra, 3 nada; 0 sin medir).
pub const fn empaquetar(prueba: Prueba, evento: usize, contado: u64, v: Option<Veredicto>) -> u64 {
    let p = match prueba {
        Prueba::Recta => 0,
        Prueba::Azar => 1,
        Prueba::Persecucion => 2,
    };
    let vv = match v {
        None => 0,
        Some(Veredicto::Cuenta) => 1,
        Some(Veredicto::Otra { .. }) => 2,
        Some(Veredicto::Nada) => 3,
    };
    (contado & MASCARA_CUENTA) | (p as u64) << 48 | ((evento as u64) & 0xF) << 52 | vv << 60
}

/// Lo de [`empaquetar`], al reves: `(prueba, evento, contado, veredicto)`
/// con el veredicto como numero (0 sin medir, 1 cuenta, 2 otra, 3 nada).
pub const fn desempaquetar(w: u64) -> (Prueba, usize, u64, u8) {
    let p = match w >> 48 & 3 {
        0 => Prueba::Recta,
        1 => Prueba::Azar,
        _ => Prueba::Persecucion,
    };
    (p, (w >> 52 & 0xF) as usize, w & MASCARA_CUENTA, (w >> 60 & 3) as u8)
}

/// Las vueltas de cada prueba de la calibracion: ~1 ms cada una en el Zen 3.
pub const VUELTAS: u64 = 1_000_000;

/// Las de cada prueba: la persecucion va a la L2 en cada carga (~15 ns), y
/// con las interrupciones apagadas no se quiere pasar de unos ms.
pub const fn vueltas(p: Prueba) -> u64 {
    match p {
        Prueba::Recta | Prueba::Azar => VUELTAS,
        Prueba::Persecucion => VUELTAS / 10,
    }
}
/// La persecucion: lineas de 64 B en el bloque (256 KiB).
pub const LINEAS: usize = 4096;

/// **La permutacion de la persecucion**: `siguiente[i]` es la linea que sigue
/// a la `i`, un ciclo UNICO por las [`LINEAS`] (Sattolo, con un xorshift de
/// semilla fija: la misma en el anfitrion y en el metal).
pub fn permutacion(siguiente: &mut [u32; LINEAS]) {
    for (i, s) in siguiente.iter_mut().enumerate() {
        *s = i as u32;
    }
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut i = LINEAS - 1;
    while i > 0 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let j = (x % i as u64) as usize;
        siguiente.swap(i, j);
        i -= 1;
    }
}

/// **En castellano**: "1 de cada N" de `parte` sobre `todo` (la frase de la
/// LENTE: "este bucle falla la L1 una de cada 3"). `None` si no hay `todo`.
pub fn una_de_cada(parte: u64, todo: u64) -> Option<u64> {
    (todo > 0 && parte > 0).then(|| (todo + parte / 2) / parte)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_control_ida_y_vuelta_y_sus_bits() {
        let c = Control::de(&EVENTOS[5], Anillo::Usuario);
        let w = c.palabra();
        assert_eq!(w & 0xFF, 0x60);
        assert_eq!(w >> 8 & 0xFF, 0xC8);
        assert_eq!((w >> 16 & 1, w >> 17 & 1, w >> 22 & 1), (1, 0, 1), "USR, no OS, EN");
        assert_eq!(Control::leer(w), c);
        // Un evento de 12 bits va partido: 7:0 abajo, 11:8 en 35:32.
        let alto = Control { evento: 0x1A3, mascara: 0, anillo: Anillo::Kernel, encendido: false };
        assert_eq!(alto.palabra(), 0xA3 | 1 << 17 | 1 << 32);
        assert_eq!(Control::leer(alto.palabra()), alto);
        assert_eq!(c.apagado().palabra() >> 22 & 1, 0);
    }

    #[test]
    fn los_msr_de_cada_contador() {
        assert_eq!(msr(0, true), (0xC001_0200, 0xC001_0201));
        assert_eq!(msr(5, true), (0xC001_020A, 0xC001_020B));
        assert_eq!(msr(3, false), (0xC001_0003, 0xC001_0007));
        assert_eq!((cuantos(true), cuantos(false)), (6, 4));
    }

    #[test]
    fn cada_fila_de_la_calibracion_tiene_su_esperado() {
        for &(p, e) in CALIBRACION.iter() {
            let (a, b) = esperado(p, e, vueltas(p)).expect("toda fila de la calibracion dice algo");
            assert!(a <= b && b < 10 * VUELTAS, "{p:?} {}", EVENTOS[e].nombre);
        }
        assert_eq!(esperado(Prueba::Recta, 4, VUELTAS), None, "la recta no dice nada de la L1");
    }

    /// El juez: lo que tendria que salir, lo que no, y el cero.
    #[test]
    fn el_juez_dice_cuenta_otra_o_nada() {
        let n = VUELTAS;
        assert_eq!(juzgar(Prueba::Recta, 1, n, 3 * n + 40), Some(Veredicto::Cuenta));
        assert!(matches!(juzgar(Prueba::Recta, 1, n, 2 * n), Some(Veredicto::Otra { .. })), "dos por vuelta: otro evento");
        assert_eq!(juzgar(Prueba::Recta, 1, n, 0), Some(Veredicto::Nada));
        assert_eq!(juzgar(Prueba::Recta, 3, n, 0), Some(Veredicto::Cuenta), "ningun fallo en la recta tambien vale");
        assert_eq!(juzgar(Prueba::Azar, 3, n, n / 2), Some(Veredicto::Cuenta));
        assert!(matches!(juzgar(Prueba::Azar, 3, n, 10), Some(Veredicto::Otra { .. })), "un azar sin fallos no es azar");
    }

    #[test]
    fn la_fila_viaja_en_una_palabra() {
        let w = empaquetar(Prueba::Persecucion, 5, 1_234_567, Some(Veredicto::Cuenta));
        assert_eq!(desempaquetar(w), (Prueba::Persecucion, 5, 1_234_567, 1));
        let w = empaquetar(Prueba::Azar, 3, MASCARA_CUENTA + 7, Some(Veredicto::Nada));
        assert_eq!(desempaquetar(w), (Prueba::Azar, 3, 6, 3), "la cuenta, en sus 48 bits");
        assert_eq!(desempaquetar(empaquetar(Prueba::Recta, 0, 9, None)).3, 0);
    }

    /// La permutacion es UN ciclo por todas las lineas: la persecucion las
    /// visita todas antes de repetir (si no, cabria en la L1 y no fallaria).
    #[test]
    fn la_persecucion_es_un_ciclo_entero() {
        let mut s = [0u32; LINEAS];
        permutacion(&mut s);
        let mut visto = [false; LINEAS];
        let mut i = 0usize;
        for _ in 0..LINEAS {
            assert!(!visto[i], "repite antes de acabar");
            visto[i] = true;
            i = s[i] as usize;
        }
        assert_eq!(i, 0, "vuelve al principio");
        assert!(visto.iter().all(|&v| v));
        let mut t = [0u32; LINEAS];
        permutacion(&mut t);
        assert_eq!(s, t, "la misma en el anfitrion y en el metal");
    }

    #[test]
    fn una_de_cada() {
        assert_eq!(super::una_de_cada(333, 1000), Some(3));
        assert_eq!(super::una_de_cada(0, 1000), None);
        assert_eq!(super::una_de_cada(5, 0), None);
    }

    #[test]
    fn los_firmes_son_los_cuatro_de_siempre() {
        let firmes: [&str; 4] = ["ciclos", "instrucciones", "saltos", "saltos fallados"];
        for e in EVENTOS.iter() {
            assert_eq!(e.confianza == Confianza::Firme, firmes.contains(&e.nombre), "{}", e.nombre);
        }
        assert_eq!(por_nombre("fallos L1D").map(|(k, _)| k), Some(5));
    }
}
