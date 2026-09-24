//! **LO QUE LA GRAFICA AMPERE CONTESTA** -- su identidad, el modo que barre y
//! el VBLANK, leidos por MMIO y SIN firmware.
//!
//! capa: puro -- recibe numeros que el kernel ya leyo y devuelve lo que significan; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- aqui no se optimiza nada, se lee bien
//!
//! # Por que existe (2026-09-23)
//!
//! `docs/maestro/GPU_NVIDIA_MAESTRO.md` dejo una pregunta en su seccion 6: se
//! puede mover la pantalla de la RTX 3060 (GA106) SIN el firmware cerrado del
//! GSP? El codigo publicado de nouveau dice que si (seccion 6b), y lo que hoy le
//! falta al compositor es UNA cosa: saber CUANDO la tarjeta acaba de barrer la
//! pantalla, para dejar de pintar a ciegas. Eso es el VBLANK.
//!
//! Antes de construir nada, se PREGUNTA (LEY 24). Este crate es la pregunta:
//!
//! ```text
//!    BOOT_0          quien es el chip          GA106 = chipset 0x176
//!    0x610060        que cabezas existen       una mascara
//!    0x682064..70    el modo que barre         totales, y donde empieza y
//!                                              acaba el borrado vertical
//!    0x68200c        el reloj de pixel         en Hz: da el refresco DICHO
//!    0x616330        la linea que barre AHORA  la que da el refresco MEDIDO
//! ```
//!
//! Las direcciones son las de `nvkm/engine/disp/gv100.c` de nouveau (Ampere
//! hereda la pantalla de Volta): `gv100_head_state` y `gv100_head_rgpos`.
//!
//! # ** Dos numeros para el refresco, y el que manda es el MEDIDO
//!
//! El reloj de pixel entre los totales da el refresco que el modo DICE. La
//! linea que barre, cronometrada al dar dos vueltas, da el que la tarjeta HACE.
//! Si no coinciden, el que manda es el medido -- y la diferencia se dice.

#![no_std]
#![forbid(unsafe_code)]

/// E2: el VBLANK por interrupcion -- que registros y en que orden (2026-09-24).
pub mod vblank;
/// M0d3: el DMA de un falcon, la prueba de fuego de la traduccion (2026-09-24).
pub mod falcon;
/// L0a: la VBIOS leida -- donde esta FWSEC y que firma pide (2026-09-24).
pub mod vbios;
/// L0b: FWSEC-FRTS preparado -- la orden y la firma, sobre bytes (2026-09-24).
pub mod fwsec;
/// L0c1: el booter y el bootloader RISC-V, leidos sobre bytes (2026-09-24).
pub mod booter;
/// L0c1: las secciones del GSP-RM, sin traerse sus 63 MB (2026-09-24).
pub mod elf;
/// L0c1: el reparto de la VRAM y la `GspFwWprMeta` (2026-09-24).
pub mod wpr;
/// L0c3a: los argumentos de LIBOS, los logs, `rmargs` y las colas (2026-09-24).
pub mod libos;
/// L0c4a: los mensajes del GSP -- su cabecera, su suma y su nombre (2026-09-24).
pub mod rpc;
/// L0c4b2a: lo que la CPU le escribe al GSP -- SetSystemInfo y SetRegistry (2026-09-24).
pub mod orden;
/// L0c4b2b: las ordenes del secuenciador que pide el GSP (2026-09-24).
pub mod secuenciador;
/// L0c4b2c: correr el secuenciador, por tramos y solo en el falcon del GSP (2026-09-24).
pub mod correr;
/// L1a: GET_GSP_STATIC_INFO -- lo que el GSP-RM dice de la 3060 (2026-09-24).
pub mod estatica;
/// L1b: GSP_RM_ALLOC -- nuestro cliente, dispositivo y subdispositivo (2026-09-24).
pub mod objeto;
/// L1b: GSP_RM_CONTROL -- preguntas de control a nuestro subdispositivo (2026-09-24).
pub mod control;
/// La lista CERRADA de lo que sale hacia el GSP-RM, y el NO de lo demas (2026-09-24).
pub mod contrato;
/// La temperatura y el enlace PCIe de la 3060, en solo lectura (2026-09-24).
pub mod salud;
/// L1c2: la CPU escribe en la VRAM por la ventana PRAMIN, sin pisar nada (2026-09-24).
pub mod vram;
/// L1d: el formato de la MMU (PDE, PTE, los cinco niveles) y mapear una pagina (2026-09-24).
pub mod mmu;
/// L1d2b: el canal AMPERE_CHANNEL_GPFIFO_A, sus 368 B exactos (2026-09-24).
pub mod canal;
/// L1d2d y L1d3: el copiador y la primera copia VRAM a VRAM (2026-09-24).
pub mod copia;
/// M5 G0: los buferes de contexto que pide el motor grafico (2026-09-24).
pub mod gr;

pub mod computo;

pub mod sombreador;

pub mod lienzo;

pub mod blur;

pub mod fractal;

pub mod triangulo;

pub mod tresde;

pub mod raster;

pub mod color3d;

pub mod giro;

pub mod escena;

/// **Quien toca los registros.** El kernel lo implementa sobre BAR0; las
/// pruebas, sobre un banco de mentira que apunta cada escritura.
pub trait Registros {
    fn leer(&mut self, reg: u32) -> u32;
    fn escribir(&mut self, reg: u32, v: u32);
}

// -- Los registros (BAR0) ---------------------------------------------------

/// `NV_PMC_BOOT_0`: la identidad del chip. Siempre se puede leer.
pub const BOOT_0: u32 = 0x0000_0000;
/// Las cabezas que EXISTEN: mascara en los bits 0..7 (`gv100_disp_init`).
pub const CABEZAS: u32 = 0x0061_0060;
/// De una cabeza a la siguiente, en los registros de modo y de barrido.
pub const PASO_CABEZA: u32 = 0x800;

/// La linea que barre la cabeza AHORA, bits 0..15 (`gv100_head_rgpos`).
pub const fn linea(cabeza: u32) -> u32 {
    0x0061_6330 + cabeza * PASO_CABEZA
}
/// El reloj de pixel del modo, en Hz.
pub const fn reloj(cabeza: u32) -> u32 {
    0x0068_200C + cabeza * PASO_CABEZA
}
/// Totales: vertical en la mitad alta, horizontal en la baja.
pub const fn totales(cabeza: u32) -> u32 {
    0x0068_2064 + cabeza * PASO_CABEZA
}
/// Donde ACABA el borrado: vertical arriba, horizontal abajo.
pub const fn fin_borrado(cabeza: u32) -> u32 {
    0x0068_206C + cabeza * PASO_CABEZA
}
/// Donde EMPIEZA el borrado: vertical arriba, horizontal abajo.
pub const fn inicio_borrado(cabeza: u32) -> u32 {
    0x0068_2070 + cabeza * PASO_CABEZA
}

/// **Es esto un error del anillo PRIV y no un dato?** Todo unos (nadie
/// contesta) o `0xBAD.....` (el anillo contesto por el registro: no se pudo
/// leer). FastOS los leyo como numeros durante semanas.
pub const fn es_error_pri(v: u32) -> bool {
    v == 0xFFFF_FFFF || (v >> 20) == 0xBAD
}

// -- El chip -----------------------------------------------------------------

/// La identidad que da `BOOT_0`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Chip(pub u32);

impl Chip {
    /// El `chipset` de nouveau: bits 20..28 de `BOOT_0`.
    pub const fn chipset(self) -> u16 {
        ((self.0 >> 20) & 0x1FF) as u16
    }
    /// La generacion: el chipset sin la variante.
    pub const fn arquitectura(self) -> u16 {
        self.chipset() & 0x1F0
    }
    /// La revision del silicio (`0xA1` = A1).
    pub const fn revision(self) -> u8 {
        (self.0 & 0xFF) as u8
    }
    /// Ampere, que es lo unico que este crate sabe leer.
    pub const fn es_ampere(self) -> bool {
        self.arquitectura() == 0x170 && !es_error_pri(self.0)
    }
    /// El nombre, si se sabe. Ninguno se supone: lo que no esta en la tabla
    /// sale por su numero.
    pub const fn nombre(self) -> &'static str {
        match self.chipset() {
            0x170 => "GA100",
            0x172 => "GA102",
            0x173 => "GA103",
            0x174 => "GA104",
            0x176 => "GA106",
            0x177 => "GA107",
            _ => "",
        }
    }
}

// -- El modo -------------------------------------------------------------------

/// **El modo que barre una cabeza**, en lineas y pixeles del reloj de pixel.
///
/// Los contadores van de 0 a `total - 1`. Lo visible va de `fin + 1` a
/// `inicio`, los dos INCLUIDOS: `fin` es la ultima linea borrada del principio
/// del cuadro e `inicio` la ULTIMA VISIBLE -- el borrado empieza despues.
///
/// ** Lo dijo el Ryzen (23-09, 15:22) y no nouveau: con `(inicio - fin - 1)`,
/// que era la lectura de su `calc()`, la sonda contesto `1919 x 1079` y 46
/// lineas de borrado en una pantalla de 1920 x 1080. Con los registros de
/// verdad (`vtotal 1125, fin 40, inicio 1120`; `htotal 2200, fin 191, inicio
/// 2111`) solo cuadra asi, y cuadra EXACTO con el 1080p de CEA-861: 5 de
/// sincronia + 36 de porche trasero = 41 lineas (0..40), 1080 visibles
/// (41..1120) y 4 de porche delantero (1121..1124).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Modo {
    pub htotal: u16,
    pub vtotal: u16,
    pub hinicio: u16,
    pub hfin: u16,
    pub vinicio: u16,
    pub vfin: u16,
    /// El reloj de pixel, en Hz.
    pub reloj_hz: u32,
}

impl Modo {
    /// Los cuatro registros en crudo. `None` si alguno no es un dato (error
    /// del anillo) o el modo no tiene forma: una cabeza apagada da ceros.
    pub fn de_registros(totales: u32, fin: u32, inicio: u32, reloj_hz: u32) -> Option<Modo> {
        if es_error_pri(totales) || es_error_pri(fin) || es_error_pri(inicio) || es_error_pri(reloj_hz) {
            return None;
        }
        let m = Modo {
            htotal: totales as u16,
            vtotal: (totales >> 16) as u16,
            hfin: fin as u16,
            vfin: (fin >> 16) as u16,
            hinicio: inicio as u16,
            vinicio: (inicio >> 16) as u16,
            reloj_hz,
        };
        let cabe = |a: u16, t: u16| a < t;
        if m.htotal == 0 || m.vtotal == 0 {
            return None;
        }
        if !cabe(m.vfin, m.vtotal) || !cabe(m.vinicio, m.vtotal) || !cabe(m.hfin, m.htotal) || !cabe(m.hinicio, m.htotal) {
            return None;
        }
        if m.lineas_visibles() == 0 || m.pixeles_visibles() == 0 {
            return None;
        }
        Some(m)
    }

    /// Las lineas que se VEN.
    pub fn lineas_visibles(&self) -> u16 {
        visibles(self.vinicio, self.vfin, self.vtotal)
    }

    /// Los pixeles que se VEN por linea.
    pub fn pixeles_visibles(&self) -> u16 {
        visibles(self.hinicio, self.hfin, self.htotal)
    }

    /// Las lineas del VBLANK: las que se barren sin pintar.
    pub fn lineas_vblank(&self) -> u16 {
        self.vtotal - self.lineas_visibles()
    }

    /// **Esta la cabeza en el VBLANK en la linea `l`?** Es la ventana en la que
    /// cambiar lo que se ve no parte el cuadro por la mitad.
    pub fn en_vblank(&self, l: u16) -> bool {
        if self.vfin < self.vinicio {
            // Lo normal: borrado al principio (0..=fin) y al final (inicio+1..).
            l <= self.vfin || l > self.vinicio
        } else {
            l > self.vinicio && l <= self.vfin
        }
    }

    /// **En que fila de lo VISIBLE va el rayo.** `0` es la primera fila que
    /// se ve; negativo, que esta en el VBLANK y le faltan esas lineas para
    /// llegar a la fila 0. Solo para el modo normal (borrado al principio y al
    /// final): el otro no lo ha dado ninguna tarjeta y no se supone.
    pub fn fila_del_rayo(&self, l: u16) -> Option<i32> {
        if self.vfin >= self.vinicio || l >= self.vtotal {
            return None;
        }
        let arriba = self.vfin as i32 + 1;
        let li = l as i32;
        if !self.en_vblank(l) {
            return Some(li - arriba);
        }
        let falta = if li <= self.vfin as i32 { arriba - li } else { self.vtotal as i32 - li + arriba };
        Some(-falta)
    }

    /// **Cuantas lineas esperar antes de copiar las filas `[y0, y1)` de lo
    /// visible, para que el rayo no las barra a medio copiar.**
    ///
    /// `copia` es lo que dura la copia ENTERA, en lineas del rayo, y quien
    /// copia lo hace DE ARRIBA ABAJO. Eso es lo que decide la cuenta: la copia
    /// no tiene que acabar antes de que el rayo LLEGUE a `y0` -- tiene que ir
    /// siempre POR DELANTE de el. La fila `d` de la caja esta copiada en
    /// `(d + 1) * copia / h` lineas y el rayo llega a ella en `falta + d`; con
    /// la copia mas rapida que el rayo (`copia < h`) basta con que el rayo no
    /// este ya encima, y con la copia mas lenta la ventaja tiene que cubrir la
    /// diferencia (`copia - h`). Mas [`GUARDA`] lineas:
    ///
    /// ```text
    ///    el rayo por ENCIMA con ventaja    ya: la copia le gana la carrera
    ///    dentro, o sin ventaja             esperar a que pase y1: desde ahi
    ///                                      tiene `total - h` de ventaja
    ///    ni asi                            no se espera: esperar no compra
    ///                                      nada, y se DICE (`Espera::cabe`)
    /// ```
    ///
    /// ** Corregido el 23-09 con el metal. La primera cuenta pedia que la
    /// copia ACABARA antes de que el rayo llegara a `y0`, y con eso la
    /// pantalla entera (7-12 ms de copia medidos, contra 666 us de VBLANK)
    /// no cabia nunca. Pero una fila de 1920 se copia en 6,5-10,9 us y el rayo
    /// barre una linea en 14,8: la copia es MAS RAPIDA que el rayo, asi que
    /// empezando detras del VBLANK llega abajo antes que el. Cabe, y no
    /// espera casi nunca.
    pub fn espera(&self, l: u16, y0: u16, y1: u16, copia: u32) -> Option<Espera> {
        let b = self.fila_del_rayo(l)?;
        if y1 <= y0 {
            return Some(Espera { lineas: 0, cabe: true });
        }
        let (y0, y1, total) = (y0 as i64, y1 as i64, self.vtotal as i64);
        let (b, copia) = (b as i64, copia as i64);
        let h = y1 - y0;
        // La ventaja que el rayo tiene que dejar para que la copia no lo pise.
        let ventaja = (copia - h).max(0) + 1 + GUARDA;
        let dentro = b >= y0 && b < y1;
        let falta = if b < y0 { y0 - b } else if b >= y1 { total - b + y0 } else { -1 };
        if !dentro && falta >= ventaja {
            return Some(Espera { lineas: 0, cabe: true });
        }
        // Esperar a que el rayo pase la caja: desde ahi le falta casi un cuadro.
        let tras = total - h;
        if tras < ventaja {
            return Some(Espera { lineas: 0, cabe: false });
        }
        // ** Cuanto falta para que el rayo SALGA de la caja (2026-09-24). Si
        // esta encima o por encima, hasta `y1`. Si ya la paso y le falta poco
        // para dar la vuelta, hasta `y1` DEL CUADRO SIGUIENTE: `falta + h`.
        // Hasta hoy los dos casos eran `y1 - b`, y el segundo da NEGATIVO:
        // como `u32` eran ~4.000 millones de lineas, el kernel lo topaba en
        // 2^32 ns y el escritorio se dormia 4,29 s. El Ryzen lo dijo
        // (`la peor 4294967 us`) y el propietario lo sintio como tirones.
        let lineas = if b >= y1 { falta + h } else { y1 - b };
        debug_assert!(lineas > 0 && lineas <= total + h);
        Some(Espera { lineas: lineas.clamp(0, total) as u32, cabe: true })
    }

    /// El refresco que el modo DICE, en milesimas de Hz. `None` sin reloj.
    pub fn refresco_dicho_mhz(&self) -> Option<u64> {
        let pixeles = self.htotal as u64 * self.vtotal as u64;
        if self.reloj_hz == 0 || pixeles == 0 {
            return None;
        }
        Some(self.reloj_hz as u64 * 1000 / pixeles)
    }
}

fn visibles(inicio: u16, fin: u16, total: u16) -> u16 {
    if fin < inicio {
        inicio - fin
    } else {
        // Borrado en medio: lo visible es lo de antes mas lo de despues.
        total - (fin - inicio)
    }
}

/// Lineas de mas que se le exigen al rayo de ventaja en [`Modo::espera`]:
/// ~118 us a 1080p. Cubren lo que pasa entre leer la linea y copiar la primera
/// fila (la vuelta de la syscall) y un cerrojo con las interrupciones
/// cerradas (el peor del `save`, 75 us). Lo que NO cubren: que el
/// planificador le quite el CPU a quien copia a media caja.
pub const GUARDA: i64 = 8;

/// Lo que contesta [`Modo::espera`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Espera {
    /// Lineas del rayo que esperar. `0` = ya.
    pub lineas: u32,
    /// `false`: la copia dura mas que lo que el rayo tarda en volver a la caja
    /// incluso esperando. No se espera, y quien copia lo cuenta: esa caja se
    /// puede partir, y solo el page flip lo arregla.
    pub cabe: bool,
}

// -- El medidor ------------------------------------------------------------------

/// **Cronometra el cuadro con la linea que barre.** Se le dan muestras
/// `(linea, tiempo)`; cada vez que la linea BAJA, la cabeza dio la vuelta.
/// Entre la primera vuelta y la ultima, el periodo de un cuadro MEDIDO.
///
/// El tiempo va en lo que quiera quien lo llame (el kernel usa el TSC); el
/// medidor no convierte nada.
#[derive(Clone, Copy, Default, Debug)]
pub struct Medidor {
    previa: Option<u16>,
    /// Vueltas vistas: veces que la linea bajo.
    pub vueltas: u32,
    primera: u64,
    ultima: u64,
    /// Muestras recibidas, para decir si la linea se movio de verdad.
    pub muestras: u32,
    /// Veces que la linea cambio entre dos muestras.
    pub cambios: u32,
}

impl Medidor {
    pub fn muestra(&mut self, l: u16, t: u64) {
        self.muestras = self.muestras.saturating_add(1);
        if let Some(p) = self.previa {
            if l != p {
                self.cambios = self.cambios.saturating_add(1);
            }
            if l < p {
                self.vueltas = self.vueltas.saturating_add(1);
                if self.vueltas == 1 {
                    self.primera = t;
                }
                self.ultima = t;
            }
        }
        self.previa = Some(l);
    }

    /// Lo que dura un cuadro, en las unidades del tiempo que se le dio. Hacen
    /// falta dos vueltas: una sola no tiene con que compararse.
    pub fn periodo(&self) -> Option<u64> {
        if self.vueltas < 2 {
            return None;
        }
        Some((self.ultima - self.primera) / (self.vueltas as u64 - 1))
    }
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El BOOT_0 que FastOS leyo de ESTA tarjeta: `0xB76000A1`.
    #[test]
    fn la_3060_de_fastos_es_un_ga106_a1() {
        let c = Chip(0xB760_00A1);
        assert_eq!(c.chipset(), 0x176);
        assert!(c.es_ampere());
        assert_eq!(c.nombre(), "GA106");
        assert_eq!(c.revision(), 0xA1);
    }

    #[test]
    fn un_error_del_anillo_no_es_un_chip() {
        assert!(es_error_pri(0xBADF_5620));
        assert!(es_error_pri(0xBAD0_0100));
        assert!(es_error_pri(0xFFFF_FFFF));
        assert!(!es_error_pri(0xB760_00A1));
        assert!(!Chip(0xBADF_5040).es_ampere());
    }

    /// ** LOS REGISTROS QUE LEYO EL RYZEN (23-09, 15:22) de la cabeza 0:
    /// 1080p a 60 Hz (CEA-861), 2200 x 1125, reloj 148,5 MHz. El borrado
    /// acaba en la 40 y la ultima visible es la 1120.
    fn mil80() -> Modo {
        Modo::de_registros(
            (1125 << 16) | 2200,
            (40 << 16) | 191,
            (1120 << 16) | 2111,
            148_500_000,
        )
        .expect("un modo de verdad")
    }

    #[test]
    fn el_1080p_se_lee_entero() {
        let m = mil80();
        assert_eq!(m.lineas_visibles(), 1080);
        assert_eq!(m.pixeles_visibles(), 1920);
        assert_eq!(m.lineas_vblank(), 45);
        assert_eq!(m.refresco_dicho_mhz(), Some(60_000));
    }

    #[test]
    fn el_vblank_da_la_vuelta() {
        let m = mil80();
        assert!(m.en_vblank(0));
        assert!(m.en_vblank(40));
        assert!(!m.en_vblank(41), "la primera visible");
        assert!(!m.en_vblank(1120), "la ultima visible");
        assert!(m.en_vblank(1121), "el primer porche delantero");
        assert!(m.en_vblank(1124));
    }

    #[test]
    fn el_rayo_se_cuenta_en_filas_de_lo_visible() {
        let m = mil80();
        assert_eq!(m.fila_del_rayo(41), Some(0), "la primera visible");
        assert_eq!(m.fila_del_rayo(1120), Some(1079), "la ultima visible");
        assert_eq!(m.fila_del_rayo(1121), Some(-45), "empieza el VBLANK: 45 para la fila 0");
        assert_eq!(m.fila_del_rayo(40), Some(-1));
        assert_eq!(m.fila_del_rayo(1125), None, "fuera del modo");
    }

    #[test]
    fn copiar_detras_del_rayo() {
        let m = mil80();
        let e = |l, y0, y1, c| m.espera(l, y0, y1, c).expect("modo normal");
        let ya = Espera { lineas: 0, cabe: true };
        // El rayo en la fila 500 (linea 541).
        assert_eq!(e(541, 100, 200, 50), ya, "ya paso la caja: no vuelve en 725 lineas");
        assert_eq!(e(541, 700, 800, 50), ya, "va por encima con 200 de sitio");
        assert_eq!(e(541, 520, 600, 50), ya, "20 de ventaja y la copia MAS RAPIDA que el rayo: le gana");
        assert_eq!(e(541, 505, 600, 50), Espera { lineas: 100, cabe: true }, "5 de ventaja: menos que la guarda, esperar a que pase");
        assert_eq!(e(541, 400, 600, 50), Espera { lineas: 100, cabe: true }, "esta dentro: esperar a que salga");
        // Copia MAS LENTA que el rayo: 80 filas en 200 lineas; la ventaja
        // tiene que cubrir las 120 que el rayo recupera.
        assert_eq!(e(541, 700, 780, 200), ya, "200 de ventaja >= 120 + 1 + guarda");
        assert_eq!(e(541, 600, 680, 200), Espera { lineas: 180, cabe: true }, "100 de ventaja no bastan");
        // La pantalla entera, con el rayo empezando el VBLANK (45 para la fila 0).
        // El metal del 23-09: 6,5-10,9 us la fila contra 14,8 la linea =
        // copia de 475-800 lineas para 1080 filas.
        assert_eq!(e(1121, 0, 1080, 800), ya, "desde el VBLANK la copia le gana al rayo");
        assert_eq!(e(541, 0, 1080, 800), Espera { lineas: 580, cabe: true }, "a media pantalla: esperar al VBLANK");
        assert_eq!(e(1121, 0, 1080, 1200), Espera { lineas: 0, cabe: false }, "120 de ventaja contra 45 de VBLANK: ni esperando");
        assert_eq!(e(541, 300, 300, 5), ya, "caja vacia");
        // El rayo YA PASO la caja y le falta poco para volver: la fila 1079
        // (linea 1120) contra una caja arriba con copia lenta. Le faltan 46
        // lineas y hacen falta 109 de ventaja: se espera a que la cruce en el
        // cuadro siguiente, 46 + 100. Antes esto daba negativo (4,29 s).
        assert_eq!(e(1120, 0, 100, 200), Espera { lineas: 146, cabe: true }, "tras la caja: la vuelta y la caja");
        for l in 0..1125u16 {
            for &(y0, y1, c) in &[(0u16, 100u16, 200u32), (500, 700, 50), (0, 1080, 800), (1000, 1080, 300)] {
                if let Some(x) = m.espera(l, y0, y1, c) {
                    assert!(x.lineas <= 1125, "nunca mas de un cuadro: l={l} caja {y0}..{y1} copia {c} -> {}", x.lineas);
                }
            }
        }
    }

    #[test]
    fn una_cabeza_apagada_o_ilegible_no_tiene_modo() {
        assert_eq!(Modo::de_registros(0, 0, 0, 0), None);
        assert_eq!(Modo::de_registros(0xBADF_5040, 0, 0, 0), None);
        assert_eq!(Modo::de_registros((10 << 16) | 10, (20 << 16) | 1, (5 << 16) | 5, 1), None, "un fin fuera del total");
    }

    #[test]
    fn el_medidor_necesita_dos_vueltas() {
        let mut m = Medidor::default();
        let mut t = 0u64;
        // Tres cuadros de 1125 lineas, una muestra cada 5 lineas y 1 unidad
        // de tiempo por linea.
        for _ in 0..3 {
            for l in (0..1125u16).step_by(5) {
                m.muestra(l, t);
                t += 5;
            }
        }
        assert_eq!(m.vueltas, 2);
        assert_eq!(m.periodo(), Some(1125));
        assert!(m.cambios > 600);
    }

    #[test]
    fn una_linea_quieta_no_mide_nada() {
        let mut m = Medidor::default();
        for t in 0..1000 {
            m.muestra(500, t);
        }
        assert_eq!(m.vueltas, 0);
        assert_eq!(m.cambios, 0);
        assert_eq!(m.periodo(), None);
    }
}
