//! **LO FINO** -- las piezas suaves de MAQUETA 2 sobre la pantalla.
//!
//! El propietario (04-10): *"que MAQUETA mejore con ultra nitidez matematica,
//! como SVG, y mejorar mi escritorio en Ring 3"*. El pintor ya existe y es
//! uno solo -- `platform/shared/bmo-pinta`, el mismo que hace la foto de la
//! maqueta en el anfitrion --; aqui solo vive el ADAPTADOR: la pantalla
//! contestando las dos preguntas que el pintor le hace (un rectangulo macizo,
//! un color mezclado sobre un pixel).
//!
//! ```text
//!    p.pieza(&bmo::Pieza::Caja { .. }, ox, oy, None)        lo que emite MAQUETA 2
//!    p.letra(x, y, b"Hola", color, bmo::Estilo::media(14))  la letra de la casa
//!    p.medir(b"Hola", estilo)                               su ancho, para centrar
//! ```
//!
//! Vive DENTRO del carril VERDE (`verde.rs` lo declara): es `punto` encima
//! de `punto`, como las letras de 8 x 16. Si falla, se ve -- y nada mas.
//!
//! ## La contabilidad, que es donde esto podia salir caro
//!
//! El pintor pide muchos `rect(x, y, 1, 1)` (las esquinas, el degradado). Si
//! cada uno fuera `Pantalla::rect`, cada uno MARCARIA -- y `marcar` copia 272
//! bytes por llamada (la cabecera de `punto_ya_marcado` tiene la cuenta). Asi
//! que se marca UNA vez, la caja entera de la pieza ya recortada, y los
//! pixeles van por `punto_ya_marcado`. Para la letra la caja se MIDE (no se
//! toma la de `bmo_pinta::caja_de`, que no sabe cuanto mide el texto y dice
//! 4096 de ancho): una caja sucia del ancho de la pantalla volcaria de mas.
//!
//! ## La letra: una sola, sin monton y sin `unsafe`
//!
//! El escritorio no tiene `alloc`, asi que la cache de glifos es una
//! [`bmo_letra::LetraFija`] estatica: 64 KiB de tinta y 512 ranuras, en el
//! `.bss`. Cuando se llena se vacia entera (un fotograma algo mas caro; el
//! contador `vaciados` lo dice) y nunca escribe fuera.
//!
//! [!] La pila de Ring 3 es de 64 KiB y dibujar un glifo nuevo usa ~13 KiB de
//! ella (los trozos y las lineas del trazo, en tablas fijas). Los glifos ya
//! hechos no usan nada: salen de la cache.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use bmo_dibujo::Recorte;
use bmo_letra::{Fuente, LetraFija};
use bmo_pinta::{Color, Lienzo};

pub use bmo_letra::{Estilo, Peso};
pub use bmo_pinta::{avance, entre, entre_color, entre_i, Pieza};

use crate::pantalla::Pantalla;

/// La cache de la letra del escritorio: 64 KiB de tinta, 512 ranuras.
type LaLetra = LetraFija<{ 64 * 1024 }, 512>;

/// **La letra, con su cerrojo.** Un `static` necesita ser `Sync`, y lo es
/// porque solo se presta con `ocupada` a `true`: dos prestamos a la vez son
/// imposibles aunque un dia haya dos hilos pintando. El que llega segundo no
/// pinta (y no espera: un compositor que se bloquea en su propia letra es
/// peor que una palabra que falta un fotograma).
struct Cerrada {
    ocupada: AtomicBool,
    letra: UnsafeCell<LaLetra>,
}

// SAFETY: `letra` solo se toca dentro de `con_letra`, con `ocupada` ganada por
// `compare_exchange`: nunca hay dos `&mut` a la vez.
unsafe impl Sync for Cerrada {}

static LETRA: Cerrada = Cerrada { ocupada: AtomicBool::new(false), letra: UnsafeCell::new(LetraFija::nueva()) };

fn con_letra<R>(f: impl FnOnce(&mut LaLetra) -> R) -> Option<R> {
    if LETRA.ocupada.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return None;
    }
    // SAFETY: el cerrojo es nuestro hasta el `store` de abajo.
    let r = f(unsafe { &mut *LETRA.letra.get() });
    LETRA.ocupada.store(false, Ordering::Release);
    Some(r)
}

/// Las veces que la cache de la letra se vacio. Si sube en cada fotograma,
/// la cache es chica para lo que se pinta.
pub fn letra_vaciados() -> u32 {
    con_letra(|l| l.vaciados).unwrap_or(0)
}

/// **La pantalla vista por el pintor**: recortada a la pantalla y a un
/// limite, con la caja ya marcada.
struct Pincel<'p> {
    p: &'p Pantalla,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Lienzo for Pincel<'_> {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        let (a, b) = (x.max(self.x0), y.max(self.y0));
        let (c2, d) = (x.saturating_add(w).min(self.x1), y.saturating_add(h).min(self.y1));
        if a >= c2 || b >= d {
            return;
        }
        if c2 - a == 1 && d - b == 1 {
            // Lo comun: un pixel de esquina o de degradado. Ya esta marcado.
            self.p.punto_ya_marcado(a as u32, b as u32, c);
        } else {
            self.p.rect(a as u32, b as u32, (c2 - a) as u32, (d - b) as u32, c);
        }
    }

    fn mezclar(&mut self, x: i32, y: i32, c: Color, alfa: u8) {
        if alfa == 0 || x < self.x0 || y < self.y0 || x >= self.x1 || y >= self.y1 {
            return;
        }
        let (x, y) = (x as u32, y as u32);
        self.p.punto_ya_marcado(x, y, bmo_pinta::sobre(self.p.read(x, y), c, alfa));
    }
}

impl Pantalla {
    /// El pincel para una caja `(x, y, w, h)` dentro de `limite`: la marca UNA
    /// vez y recorta a las dos. `None` si no queda nada que pintar.
    fn pincel(&self, x: i32, y: i32, w: i32, h: i32, limite: Option<Recorte>) -> Option<Pincel<'_>> {
        let mut r = Recorte::nuevo(x, y, w, h).interseccion(&Recorte::nuevo(0, 0, self.ancho as i32, self.alto as i32));
        if let Some(l) = limite {
            r = r.interseccion(&l);
        }
        if r.vacio() {
            return None;
        }
        self.sincronizar_lectura();
        self.marcar(r.x0 as u32, r.y0 as u32, (r.x1 - r.x0) as u32, (r.y1 - r.y0) as u32);
        Some(Pincel { p: self, x0: r.x0, y0: r.y0, x1: r.x1, y1: r.y1 })
    }

    /// **Pinta una pieza suave** (lo que emite MAQUETA 2) con su origen en
    /// `(ox, oy)`, sin salirse de `limite` si lo hay.
    pub fn pieza(&self, pz: &Pieza, ox: i32, oy: i32, limite: Option<Recorte>) {
        let (x, y, mut w, h) = bmo_pinta::caja_de(pz);
        if let Pieza::Letra { texto, px, peso, espacio, mayusculas, .. } = *pz {
            // La caja de verdad de la letra: lo que mide, mas lo que asoma.
            let e = bmo_pinta::estilo(px, peso, espacio, mayusculas);
            w = con_letra(|l| l.medir(texto, e)).unwrap_or(0) + 4;
        }
        let Some(mut pincel) = self.pincel(x + ox, y + oy, w, h, limite) else { return };
        let _ = con_letra(|l| bmo_pinta::pieza(&mut pincel, l, pz, ox, oy));
    }

    /// **Una pieza con un dato dentro** (H2): si es una `Pieza::Letra`, su
    /// texto llega al ejecutar (un hueco `{nombre}` de MAQUETA) y se corta a
    /// `max` pixeles con `...`; cualquier otra se pinta como [`Self::pieza`].
    pub fn pieza_cabe(&self, pz: &Pieza, max: i32, ox: i32, oy: i32, limite: Option<Recorte>) {
        let Pieza::Letra { x, y, alto, texto, c, px, peso, espacio, mayusculas } = *pz else {
            return self.pieza(pz, ox, oy, limite);
        };
        let e = bmo_pinta::estilo(px, peso, espacio, mayusculas);
        let (_, cy, _, h) = bmo_pinta::caja_de(pz);
        let (x, y) = (x + ox, y + oy);
        let Some(mut pincel) = self.pincel(x - 2, cy + oy, max + 4, h, limite) else { return };
        let _ = con_letra(|l| bmo_pinta::letra_cabe(&mut pincel, l, x, y, alto, texto, c, e, max));
    }

    /// **Una pieza a medio camino** entre `a` y `b` (P3b): `k` milesimas de
    /// avance (`bmo::avance`). Es lo que pinta, en cada fotograma, una
    /// transicion que genera MAQUETA -- la misma mezcla que la foto del
    /// anfitrion.
    pub fn pieza_entre(&self, a: &Pieza, b: &Pieza, k: i32, ox: i32, oy: i32) {
        self.pieza(&bmo_pinta::entre_piezas(a, b, k), ox, oy, None);
    }

    /// **La letra de la casa**, con la BASE de su linea en `y` y empezando en
    /// `x`. Devuelve el ancho pintado. Es lo que los textos del escritorio
    /// usan en vez de la de 8 x 16 cuando quieren verse como sus maquetas.
    pub fn letra(&self, x: i32, y: i32, s: &[u8], c: Color, e: Estilo) -> i32 {
        let w = self.medir(s, e);
        let px = e.px as i32;
        // Lo que puede asomar: un acento por arriba, la cola de una g por abajo.
        let Some(mut pincel) = self.pincel(x - 2, y - px - px / 2, w + 4, 2 * px + px / 2, None) else { return w };
        con_letra(|l| l.escribir(s, e, x, y, |px, py, a| pincel.mezclar(px, py, c, a))).unwrap_or(0)
    }

    /// **La letra en una caja** de `alto` pixeles (su `line-height`), como la
    /// pone el navegador y como la pinta MAQUETA. `y` es lo de arriba de la
    /// caja.
    pub fn letra_en_caja(&self, x: i32, y: i32, alto: i32, s: &[u8], c: Color, e: Estilo) -> i32 {
        self.letra(x, y + bmo_letra::base_en_caja(e.px, alto), s, c, e)
    }

    /// **Lo que mide** un texto en la letra de la casa, en pixeles.
    pub fn medir(&self, s: &[u8], e: Estilo) -> i32 {
        con_letra(|l| l.medir(s, e)).unwrap_or(0)
    }

    /// **Una caja redonda** maciza, con la curva suave. Es la pieza que mas
    /// se usa, y asi no hay que escribir la `Pieza` entera.
    pub fn caja_redonda(&self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color) {
        self.pieza(&Pieza::Caja { x, y, w, h, r, c }, 0, 0, None);
    }

    /// **Un borde redondo** de `grosor` pixeles (lo de dentro no se toca).
    pub fn borde_redondo(&self, x: i32, y: i32, w: i32, h: i32, r: i32, grosor: i32, c: Color) {
        self.pieza(&Pieza::Borde { x, y, w, h, r, grosor, c }, 0, 0, None);
    }
}

// -- Las pruebas ------------------------------------------------------------
//
// Estas SI corren (`cargo test -p bmo-userland --lib`): `Pantalla::en_memoria`
// no toca el kernel, y el pintor es aritmetica.
#[cfg(test)]
mod pruebas {
    extern crate std;
    use std::boxed::Box;
    use std::vec;
    use std::vec::Vec;

    use super::*;

    const FONDO: u32 = 0x0010_1828;

    /// La letra es UNA y el que llega segundo no pinta (es a proposito: ver
    /// `Cerrada`). Los tests corren en hilos a la vez, asi que van en fila.
    static EN_FILA: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn pantalla(w: u32, h: u32) -> Pantalla {
        let px: &'static mut [u32] = Box::leak(vec![FONDO; (w * h) as usize].into_boxed_slice());
        Pantalla::en_memoria(px, w, h).unwrap()
    }

    fn pixeles(p: &Pantalla) -> Vec<u32> {
        let mut v = Vec::new();
        for y in 0..p.alto {
            for x in 0..p.ancho {
                v.push(p.read(x, y));
            }
        }
        v
    }

    /// El lienzo de la foto del anfitrion, en chico: el mismo `sobre`.
    struct Foto {
        w: i32,
        h: i32,
        px: Vec<u32>,
    }

    impl Lienzo for Foto {
        fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
            for j in y.max(0)..(y + h).min(self.h) {
                for i in x.max(0)..(x + w).min(self.w) {
                    self.px[(j * self.w + i) as usize] = c;
                }
            }
        }
        fn mezclar(&mut self, x: i32, y: i32, c: Color, alfa: u8) {
            if x >= 0 && y >= 0 && x < self.w && y < self.h {
                let k = (y * self.w + x) as usize;
                self.px[k] = bmo_pinta::sobre(self.px[k], c, alfa);
            }
        }
    }

    /// ** La pantalla y la foto de la maqueta pintan LOS MISMOS pixeles:
    /// el escritorio y el anfitrion ejecutan el mismo pintor.
    #[test]
    fn la_pantalla_pinta_como_la_foto() {
        let _fila = EN_FILA.lock().unwrap_or_else(|e| e.into_inner());
        let p = pantalla(96, 64);
        let mut f = Foto { w: 96, h: 64, px: vec![FONDO; 96 * 64] };
        let piezas = [
            Pieza::Resplandor { x: 20, y: 14, w: 50, h: 30, r: 10, alcance: 8, argb: 0x66FF_D45E },
            Pieza::Degradado { x: 20, y: 14, w: 50, h: 30, r: 10, de: 0x0022_1A40, a: 0x0040_2A70, vertical: false },
            Pieza::Borde { x: 20, y: 14, w: 50, h: 30, r: 10, grosor: 1, c: 0x00C9_A227 },
            Pieza::Letra { x: 26, y: 20, alto: 18, texto: b"Paris", c: 0x00F4_EFE6, px: 13, peso: 500, espacio: 0, mayusculas: false },
        ];
        for pz in &piezas {
            p.pieza(pz, 0, 0, None);
            let mut letra: LaLetra = LetraFija::nueva();
            bmo_pinta::pieza(&mut f, &mut letra, pz, 0, 0);
        }
        assert_eq!(pixeles(&p), f.px);
        assert!(f.px.iter().filter(|&&c| c != FONDO).count() > 1500);
    }

    /// La letra marca la caja de lo que MIDE, no 4096 de ancho.
    #[test]
    fn la_letra_marca_solo_lo_suyo() {
        let _fila = EN_FILA.lock().unwrap_or_else(|e| e.into_inner());
        let p = pantalla(800, 60);
        let _ = p.sucio.replace(crate::sin_gpu::sucio::Sucias::nueva());
        let e = Estilo::media(14);
        let w = p.letra(10, 30, b"Bonjour", 0x00FF_FFFF, e);
        assert!(w > 30 && w < 90, "mide {w}");
        let s = p.sucio.get();
        assert_eq!(s.cajas().len(), 1);
        let (x0, _, x1, _) = s.cajas()[0];
        assert!(x0 >= 6 && x1 <= 10 + w as u32 + 4, "caja {x0}..{x1}, ancho {w}");
    }

    /// Con limite, lo de fuera no se toca (ni el resplandor que se sale).
    #[test]
    fn el_limite_se_respeta() {
        let _fila = EN_FILA.lock().unwrap_or_else(|e| e.into_inner());
        let p = pantalla(80, 40);
        let limite = Recorte::nuevo(0, 0, 40, 40);
        p.pieza(&Pieza::Resplandor { x: 30, y: 10, w: 20, h: 20, r: 6, alcance: 10, argb: 0xFFFF_0000 }, 0, 0, Some(limite));
        p.pieza(&Pieza::Caja { x: 30, y: 10, w: 20, h: 20, r: 6, c: 0x00FF_FFFF }, 0, 0, Some(limite));
        for y in 0..40 {
            for x in 40..80 {
                assert_eq!(p.read(x, y), FONDO, "({x}, {y})");
            }
        }
        assert_ne!(p.read(35, 20), FONDO);
    }

    /// Fuera de la pantalla no pasa nada: ni se pinta ni se marca.
    #[test]
    fn fuera_de_la_pantalla_no_pasa_nada() {
        let _fila = EN_FILA.lock().unwrap_or_else(|e| e.into_inner());
        let p = pantalla(40, 40);
        let _ = p.sucio.replace(crate::sin_gpu::sucio::Sucias::nueva());
        p.caja_redonda(-100, -100, 50, 50, 8, 0x00FF_FFFF);
        p.caja_redonda(500, 10, 50, 50, 8, 0x00FF_FFFF);
        assert!(p.sucio.get().vacia());
        assert!(pixeles(&p).iter().all(|&c| c == FONDO));
    }
}
