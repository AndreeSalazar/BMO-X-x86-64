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
