//! **LA CPU ESCRIBE EN LA VRAM (L1c2)** -- por la ventana PRAMIN de BAR0: un
//! MiB de la VRAM visto en `BAR0 + 0x700000`, que se mueve con el registro
//! `0x1700` (la base >> 16). Es como nouveau escribe en la VRAM antes de tener
//! BAR1 o BAR2 (`instmem/nv50.c`, que usa tambien la GA106).
//!
//! capa: puro -- la prueba sobre `Registros`; el kernel pone BAR0 (L8)
//!
//! [eje]     CORRECCION -- la VRAM es del GSP-RM, de la pantalla y nuestra; una
//!           escritura en el sitio malo pisa algo vivo
//!
//! # Por que hace falta
//!
//! El espacio de direcciones de L1c1 es "de fuera": sus tablas de paginas las
//! construye quien hace de RM de la CPU, EN LA VRAM, y luego se le dice al RM
//! donde estan (`r535/vmm.c` de nouveau). Sin escribir en la VRAM no hay
//! tablas, ni canal, ni sombreador.
//!
//! # La prueba, y por que no pisa nada
//!
//! ```text
//!    1  guardar la ventana que habia (el registro 0x1700)
//!    2  moverla a PRUEBA y GUARDAR los 4 KiB que hubiera ahi
//!    3  escribir un patron (cada palabra distinta) y RELEERLO
//!    4  DEVOLVER los 4 KiB guardados y comprobar que quedaron
//!    5  devolver la ventana
//! ```
//!
//! Una sola direccion, fija y revisada: [`PRUEBA`], 64 MiB. En la 3060 cae
//! dentro de la region que el GSP-RM dio como usable (`0x003110000..
//! 0x2F06DFFFF`, L1a 10:35): por encima del framebuffer del GOP (lo bajo de la
//! VRAM) y muy por debajo de la WPR2 (`0x2F4000000`). El escritorio lo
//! comprueba contra las regiones antes de pedirla; el kernel no acepta otra.

use crate::Registros;

/// `NV_PBUS_BAR0_WINDOW`: la base de la ventana, en unidades de 64 KiB.
pub const VENTANA_REG: u32 = 0x0000_1700;
/// Donde se ve la ventana en BAR0, y cuanto mide.
pub const VENTANA: u32 = 0x0070_0000;
pub const VENTANA_MEDIDA: u64 = 1 << 20;
/// La unica direccion de VRAM que se prueba: 64 MiB.
pub const PRUEBA: u64 = 0x0400_0000;
/// Palabras de la prueba: una pagina.
pub const PALABRAS: usize = 1024;
/// L1c3: la pagina del DIRECTORIO de paginas (la PD3 de nuestro espacio), 1 MiB
/// por encima de la prueba: dentro de lo usable, y de nadie mas.
pub const DIRECTORIO: u64 = 0x0410_0000;

// -- L1d1: EL TRAMO MAPEADO ------------------------------------------------
//
// El MiB de 65 MiB es de las TABLAS (la raiz y, detras, una de cada nivel); el
// de 66 MiB, de las PAGINAS que la GPU vera por direccion virtual (sus colas,
// sus datos). Todo dentro de lo que el GSP-RM dio como usable, y de nadie mas.

/// PD2, PD1, PD0 y PT del tramo, detras de la raiz.
pub const TABLAS: [u64; 4] = [DIRECTORIO + 0x1000, DIRECTORIO + 0x2000, DIRECTORIO + 0x3000, DIRECTORIO + 0x4000];
/// Las paginas del tramo, en VRAM.
pub const TRAMO: u64 = 0x0420_0000;
/// Donde las ve la GPU: 8 GiB (lejos de los 4 GiB que el RM se reserva).
pub const TRAMO_VA: u64 = 0x2_0000_0000;
/// Cuantas: 64 KiB (colas, USERD, instancia y datos de L1d).
pub const TRAMO_PAGINAS: usize = 16;

/// **La ventana para `dir`**: `(valor del registro, desplazamiento en ella)`.
pub const fn ventana(dir: u64) -> (u32, u32) {
    let base = dir & !(VENTANA_MEDIDA - 1);
    ((base >> 16) as u32, (dir & (VENTANA_MEDIDA - 1)) as u32)
}

/// El patron de la palabra `k`: distinta en cada una y nunca 0 ni todo unos.
pub const fn patron(k: usize) -> u32 {
    0xB0B0_0000 ^ (k as u32).wrapping_mul(0x9E37_79B1) | 1
}

/// Lo que dijo la prueba.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Prueba {
    /// Palabras del patron que se releyeron iguales (de [`PALABRAS`]).
    pub buenas: u32,
    /// Palabras que volvieron a ser lo que habia (de [`PALABRAS`]).
    pub devueltas: u32,
    /// El registro de la ventana antes, y si se le devolvio.
    pub ventana_antes: u32,
    pub ventana_devuelta: bool,
    /// La primera palabra que no cuadro, si alguna: `(k, leida)`.
    pub primera_mal: Option<(u32, u32)>,
}

/// **La prueba**, con `guardado` como sitio para los 4 KiB de antes.
pub fn probar<R: Registros>(r: &mut R, guardado: &mut [u32; PALABRAS]) -> Prueba {
    let (base, off) = ventana(PRUEBA);
    let mut p = Prueba { ventana_antes: r.leer(VENTANA_REG), ..Prueba::default() };
    r.escribir(VENTANA_REG, base);
    let dir = |k: usize| VENTANA + off + 4 * k as u32;
    for (k, g) in guardado.iter_mut().enumerate() {
        *g = r.leer(dir(k));
    }
    for k in 0..PALABRAS {
        r.escribir(dir(k), patron(k));
    }
    for k in 0..PALABRAS {
        let v = r.leer(dir(k));
        if v == patron(k) {
            p.buenas += 1;
        } else if p.primera_mal.is_none() {
            p.primera_mal = Some((k as u32, v));
        }
    }
    for (k, &g) in guardado.iter().enumerate() {
        r.escribir(dir(k), g);
    }
    for (k, &g) in guardado.iter().enumerate() {
        if r.leer(dir(k)) == g {
            p.devueltas += 1;
        }
    }
    r.escribir(VENTANA_REG, p.ventana_antes);
    p.ventana_devuelta = r.leer(VENTANA_REG) == p.ventana_antes;
    p
}

/// **Una pagina de VRAM a cero** (L1c3: la raiz vacia, todo sin mapear), por
/// la ventana, que queda como estaba. Devuelve cuantas palabras se releyeron
/// a cero (de [`PALABRAS`]).
pub fn a_cero<R: Registros>(r: &mut R, dir: u64) -> u32 {
    let (base, off) = ventana(dir);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, base);
    let d = |k: usize| VENTANA + off + 4 * k as u32;
    for k in 0..PALABRAS {
        r.escribir(d(k), 0);
    }
    let ceros = (0..PALABRAS).filter(|&k| r.leer(d(k)) == 0).count() as u32;
    r.escribir(VENTANA_REG, antes);
    ceros
}

/// **Leer 64 bits de VRAM** por la ventana, que queda como estaba (L1d0:
/// leer la raiz y ver que escribio el RM en ella).
pub fn leer64<R: Registros>(r: &mut R, dir: u64) -> u64 {
    let (base, off) = ventana(dir);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, base);
    let lo = r.leer(VENTANA + off) as u64;
    let hi = r.leer(VENTANA + off + 4) as u64;
    r.escribir(VENTANA_REG, antes);
    lo | hi << 32
}

/// **Escribir 64 bits de VRAM** por la ventana, que queda como estaba.
pub fn escribir64<R: Registros>(r: &mut R, dir: u64, v: u64) {
    let (base, off) = ventana(dir);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, base);
    r.escribir(VENTANA + off, v as u32);
    r.escribir(VENTANA + off + 4, (v >> 32) as u32);
    r.escribir(VENTANA_REG, antes);
}

/// **L1d1: mapear el tramo** bajo la raiz: sus cuatro tablas a cero, y las
/// escrituras de `mmu::mapear_tramo` de la hoja a la raiz, RELEIDAS. Solo si
/// la entrada de la raiz esta VACIA (lo que el RM tenga ahi no se pisa).
/// Devuelve `(escrituras, releidas iguales)`, o `None` si la raiz ya tenia algo.
pub fn mapear_tramo<R: Registros>(r: &mut R) -> Option<(u32, u32)> {
    let i = crate::mmu::indices(TRAMO_VA)[0];
    if leer64(r, DIRECTORIO + 8 * i as u64) != 0 {
        return None;
    }
    for t in TABLAS {
        a_cero(r, t);
    }
    let (e, n) = crate::mmu::mapear_tramo(DIRECTORIO, TABLAS, TRAMO_VA, TRAMO, TRAMO_PAGINAS)?;
    for &(d, v) in &e[..n] {
        escribir64(r, d, v);
    }
    let bien = e[..n].iter().filter(|&&(d, v)| leer64(r, d) == v).count() as u32;
    Some((n as u32, bien))
}

/// La prueba cabe en un `u64` para el escritorio: `buenas | devueltas << 16 |
/// ventana devuelta << 31 | ventana de antes << 32`.
pub const fn empaquetar(p: &Prueba) -> u64 {
    p.buenas as u64 | (p.devueltas as u64) << 16 | (p.ventana_devuelta as u64) << 31 | (p.ventana_antes as u64) << 32
}

pub const fn desempaquetar(v: u64) -> (u32, u32, bool, u32) {
    ((v & 0xFFFF) as u32, ((v >> 16) & 0x7FFF) as u32, v >> 31 & 1 != 0, (v >> 32) as u32)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una 3060 de mentira: BAR0 con su ventana PRAMIN sobre una pagina de
    /// VRAM que se repite (basta: la prueba toca una).
    struct Falsa {
        ventana: u32,
        vram: [u32; PALABRAS],
        /// Palabras de VRAM que no guardan lo que se les escribe.
        rotas: usize,
    }

    impl Registros for Falsa {
        fn leer(&mut self, reg: u32) -> u32 {
            if reg == VENTANA_REG {
                return self.ventana;
            }
            let i = ((self.ventana as u64) << 16) as usize / 4 + (reg - VENTANA) as usize / 4;
            self.vram[i % self.vram.len()]
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            if reg == VENTANA_REG {
                self.ventana = v;
                return;
            }
            let i = ((self.ventana as u64) << 16) as usize / 4 + (reg - VENTANA) as usize / 4;
            if i % 1024 >= 1024 - self.rotas {
                return;
            }
            let n = self.vram.len();
            self.vram[i % n] = v;
        }
    }

    #[test]
    fn la_ventana_de_64_mib() {
        assert_eq!(ventana(PRUEBA), (0x400, 0));
        assert_eq!(ventana(0x2F06_DFFF0), (0x2F060, 0xDFFF0));
        assert_ne!(patron(0), patron(1));
        assert!((0..PALABRAS).all(|k| patron(k) != 0 && patron(k) != u32::MAX));
    }

    #[test]
    fn prueba_sana_devuelve_todo() {
        let mut f = Falsa { ventana: 0x77, vram: [0x1234_5678; PALABRAS], rotas: 0 };
        let mut g = [0u32; PALABRAS];
        let p = probar(&mut f, &mut g);
        assert_eq!((p.buenas, p.devueltas, p.ventana_antes, p.ventana_devuelta, p.primera_mal), (1024, 1024, 0x77, true, None));
        assert_eq!(f.ventana, 0x77, "la ventana, como estaba");
        assert!(f.vram.iter().all(|&w| w == 0x1234_5678), "la VRAM, como estaba");
        assert_eq!(desempaquetar(empaquetar(&p)), (1024, 1024, true, 0x77));
    }

    #[test]
    fn vram_que_no_guarda_se_dice() {
        let mut f = Falsa { ventana: 0, vram: [0; PALABRAS], rotas: 4 };
        let mut g = [0u32; PALABRAS];
        let p = probar(&mut f, &mut g);
        assert_eq!(p.buenas, 1020);
        assert_eq!(p.primera_mal, Some((1020, 0)));
    }

    #[test]
    fn la_raiz_a_cero_y_la_ventana_como_estaba() {
        let mut f = Falsa { ventana: 0x33, vram: [0xFFFF_FFFF; PALABRAS], rotas: 0 };
        assert_eq!(a_cero(&mut f, DIRECTORIO), 1024);
        assert!(f.vram.iter().all(|&w| w == 0));
        assert_eq!(f.ventana, 0x33);
        assert_eq!(ventana(DIRECTORIO), (0x410, 0));
    }

    /// Una VRAM de mentira mas grande: 2 MiB desde `DIRECTORIO` (las tablas y
    /// el tramo), por la ventana.
    struct Grande {
        ventana: u32,
        vram: [u32; 1 << 19],
    }

    impl Registros for Grande {
        fn leer(&mut self, reg: u32) -> u32 {
            if reg == VENTANA_REG {
                return self.ventana;
            }
            let dir = ((self.ventana as u64) << 16) + (reg - VENTANA) as u64;
            self.vram[((dir - DIRECTORIO) / 4) as usize]
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            if reg == VENTANA_REG {
                self.ventana = v;
                return;
            }
            let dir = ((self.ventana as u64) << 16) + (reg - VENTANA) as u64;
            self.vram[((dir - DIRECTORIO) / 4) as usize] = v;
        }
    }

    #[test]
    fn el_tramo_se_mapea_y_se_relee() {
        extern crate std;
        let mut g = std::boxed::Box::new(Grande { ventana: 0xFFF0, vram: [0; 1 << 19] });
        let (n, bien) = mapear_tramo(&mut *g).unwrap();
        assert_eq!((n, bien), (20, 20));
        assert_eq!(g.ventana, 0xFFF0, "la ventana, como estaba");
        // La raiz apunta a la PD2 del tramo, en VRAM.
        assert_eq!(leer64(&mut *g, DIRECTORIO), crate::mmu::pde_vram(TABLAS[0]));
        // Con la raiz ya ocupada, no se pisa.
        assert_eq!(mapear_tramo(&mut *g), None);
    }
}
