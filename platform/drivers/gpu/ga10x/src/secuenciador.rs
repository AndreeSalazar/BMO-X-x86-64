//! **EL SECUENCIADOR QUE PIDE EL GSP (L0c4b2b)** -- las ordenes de
//! `GSP_RUN_CPU_SEQUENCER`, leidas una a una: que registro quiere que la CPU
//! toque, con que valor, que esperar, y cuando mover su propio nucleo.
//!
//! capa: puro -- lee bytes y dice que ordenes son; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- una orden mal partida es escribir en un registro
//!           el valor de la siguiente; se para en la primera que no se entiende
//!
//! # El mensaje (r570.144, `rpc_run_cpu_sequencer_v17_00`)
//!
//! ```text
//!    +0   bufferSizeDWord   lo que mide el buffer de ordenes, en u32
//!    +4   cmdIndex          cuantas PALABRAS de ordenes trae (no ordenes)
//!    +8   regSaveArea[8]    donde el GSP guarda lo de REG_STORE
//!    +40  las ordenes, pegadas: un opcode u32 y detras SOLO su carga
//! ```
//!
//! ** `cmdIndex` son PALABRAS (metal 24-09 08:48): el primer secuenciador de
//! la 3060 trajo `cmdIndex` 1564 y 420 ordenes -- 312 REG_WRITE x 3 palabras
//! + 104 REG_POLL x 6 + 4 del nucleo x 1 = 1564 exactas. Leido como ordenes
//! (asi lo nombra nova-core, `total_cmds`) sobraban 1144 y se daba por roto;
//! nova-core no lo nota porque se para antes, donde acaban los datos.
//!
//! # Las ordenes (`GSP_SEQ_BUF_OPCODE`, y lo que hace nova-core con cada una)
//!
//! ```text
//!    0 REG_WRITE    addr, val                   escribir
//!    1 REG_MODIFY   addr, mask, val             (leido & !mask) | val
//!    2 REG_POLL     addr, mask, val, timeout, error   esperar (x & mask) == val
//!                                               (timeout en us; 0 = 4 s)
//!    3 DELAY_US     val                         esperar val us
//!    4 REG_STORE    addr, index                 leer (y guardar en index)
//!    5 CORE_RESET                               resetear el falcon del GSP
//!    6 CORE_START                               arrancarlo
//!    7 CORE_WAIT_FOR_HALT                       esperar a que se pare
//!    8 CORE_RESUME                              reset del GSP, sus argumentos
//!                                               de LIBOS al buzon, arrancar el
//!                                               SEC2, esperar al GSP-RM, el OS
//!                                               y ver el RISC-V activo
//! ```

/// Lo que mide la cabecera del mensaje, antes de las ordenes.
pub const CABECERA: usize = 40;

/// Una orden del GSP.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orden {
    Escribir { reg: u32, valor: u32 },
    Modificar { reg: u32, mascara: u32, valor: u32 },
    Esperar { reg: u32, mascara: u32, valor: u32, plazo_us: u32 },
    Retraso { us: u32 },
    Guardar { reg: u32, indice: u32 },
    Resetear,
    Arrancar,
    EsperarParada,
    Reanudar,
}

impl Orden {
    /// El nombre corto, para las filas.
    pub fn nombre(&self) -> &'static [u8] {
        match self {
            Orden::Escribir { .. } => b"ESCRIBIR",
            Orden::Modificar { .. } => b"MODIFICAR",
            Orden::Esperar { .. } => b"ESPERAR",
            Orden::Retraso { .. } => b"RETRASO",
            Orden::Guardar { .. } => b"LEER",
            Orden::Resetear => b"CORE_RESET",
            Orden::Arrancar => b"CORE_START",
            Orden::EsperarParada => b"CORE_WAIT_FOR_HALT",
            Orden::Reanudar => b"CORE_RESUME",
        }
    }

    /// El registro que toca, si toca uno.
    pub fn registro(&self) -> Option<u32> {
        match *self {
            Orden::Escribir { reg, .. } | Orden::Modificar { reg, .. } | Orden::Esperar { reg, .. } | Orden::Guardar { reg, .. } => Some(reg),
            _ => None,
        }
    }

    /// El indice del tipo, para contarlas (el opcode).
    pub fn codigo(&self) -> usize {
        match self {
            Orden::Escribir { .. } => 0,
            Orden::Modificar { .. } => 1,
            Orden::Esperar { .. } => 2,
            Orden::Retraso { .. } => 3,
            Orden::Guardar { .. } => 4,
            Orden::Resetear => 5,
            Orden::Arrancar => 6,
            Orden::EsperarParada => 7,
            Orden::Reanudar => 8,
        }
    }
}

/// Por que no se sigue leyendo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoSe {
    /// Un opcode que r570.144 no tiene.
    Opcode(u32),
    /// La orden no cabe en lo que queda.
    Corta,
}

/// **Donde cae un registro**, para decirlo en las filas: las unidades que
/// se esperan en un secuenciador de arranque.
pub fn unidad(reg: u32) -> &'static [u8] {
    match reg {
        0x0000_0000..=0x0000_0FFF => b"PMC",
        0x0010_0000..=0x0010_FFFF => b"PFB",
        0x0011_0000..=0x0011_1FFF => b"falcon GSP",
        0x0011_8000..=0x0011_8FFF => b"PGC6/BSI",
        0x0084_0000..=0x0084_1FFF => b"falcon SEC2",
        _ => b"?",
    }
}

fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// La cabecera: `(bufferSizeDWord, cmdIndex)`. `None` si no llega a 40 bytes.
pub fn cabecera(datos: &[u8]) -> Option<(u32, u32)> {
    (datos.len() >= CABECERA).then(|| (u32_de(datos, 0), u32_de(datos, 4)))
}

/// Las 8 palabras de `regSaveArea`.
pub fn guardados(datos: &[u8]) -> [u32; 8] {
    let mut g = [0; 8];
    if datos.len() >= CABECERA {
        for (k, v) in g.iter_mut().enumerate() {
            *v = u32_de(datos, 8 + 4 * k);
        }
    }
    g
}

/// **Las ordenes**, en orden, hasta las `cmdIndex` palabras; se para en la
/// primera que no entiende o que no cabe (y la dice).
pub struct Ordenes<'a> {
    b: &'a [u8],
    o: usize,
    /// Los datos no llegan a las `cmdIndex` palabras.
    faltan: bool,
    hecho: bool,
}

/// Leer las ordenes de los datos del mensaje (desde su +0, cabecera incluida).
pub fn ordenes(datos: &[u8]) -> Ordenes<'_> {
    // Hasta donde acaban sus `cmdIndex` palabras. `bufferSizeDWord` es lo que
    // mide el buffer del GSP entero: se muestra, no corta.
    let (_, palabras) = cabecera(datos).unwrap_or((0, 0));
    let fin = CABECERA + palabras as usize * 4;
    let hay = fin.min(datos.len());
    Ordenes { b: &datos[..hay], o: CABECERA, faltan: fin > datos.len() && datos.len() >= CABECERA, hecho: datos.len() < CABECERA }
}

impl Ordenes<'_> {
    fn parar(&mut self, e: NoSe) -> Option<Result<Orden, NoSe>> {
        self.hecho = true;
        Some(Err(e))
    }
}

impl Iterator for Ordenes<'_> {
    type Item = Result<Orden, NoSe>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.hecho {
            return None;
        }
        if self.o >= self.b.len() {
            self.hecho = true;
            return self.faltan.then_some(Err(NoSe::Corta));
        }
        if self.o + 4 > self.b.len() {
            return self.parar(NoSe::Corta);
        }
        let op = u32_de(self.b, self.o);
        let carga = match op {
            0 | 4 => 8,
            1 => 12,
            2 => 20,
            3 => 4,
            5..=8 => 0,
            _ => return self.parar(NoSe::Opcode(op)),
        };
        let p = self.o + 4;
        if p + carga > self.b.len() {
            return self.parar(NoSe::Corta);
        }
        let w = |k: usize| u32_de(self.b, p + 4 * k);
        let orden = match op {
            0 => Orden::Escribir { reg: w(0), valor: w(1) },
            1 => Orden::Modificar { reg: w(0), mascara: w(1), valor: w(2) },
            2 => Orden::Esperar { reg: w(0), mascara: w(1), valor: w(2), plazo_us: w(3) },
            3 => Orden::Retraso { us: w(0) },
            4 => Orden::Guardar { reg: w(0), indice: w(1) },
            5 => Orden::Resetear,
            6 => Orden::Arrancar,
            7 => Orden::EsperarParada,
            _ => Orden::Reanudar,
        };
        self.o = p + carga;
        Some(Ok(orden))
    }
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn mensaje(palabras: &[u32], n: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&(palabras.len() as u32).to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&[0u8; 32]);
        for w in palabras {
            b.extend_from_slice(&w.to_le_bytes());
        }
        b
    }

    #[test]
    fn cada_orden_con_su_carga() {
        let p = [
            0, 0x110040, 0xAB, // escribir
            1, 0x840100, 0xF0, 0x10, // modificar
            2, 0x118000, 1, 1, 5000, 0, // esperar
            3, 100, // retraso
            4, 0x1180F8, 3, // leer y guardar
            5, 6, 7, 8, // el nucleo
        ];
        let b = mensaje(&p, p.len() as u32);
        let v: Vec<_> = ordenes(&b).map(|r| r.unwrap()).collect();
        assert_eq!(
            v,
            [
                Orden::Escribir { reg: 0x110040, valor: 0xAB },
                Orden::Modificar { reg: 0x840100, mascara: 0xF0, valor: 0x10 },
                Orden::Esperar { reg: 0x118000, mascara: 1, valor: 1, plazo_us: 5000 },
                Orden::Retraso { us: 100 },
                Orden::Guardar { reg: 0x1180F8, indice: 3 },
                Orden::Resetear,
                Orden::Arrancar,
                Orden::EsperarParada,
                Orden::Reanudar,
            ]
        );
        assert_eq!(v[1].registro(), Some(0x840100));
        assert_eq!(unidad(0x840100), b"falcon SEC2");
        assert_eq!(v[8].nombre(), b"CORE_RESUME");
    }

    #[test]
    fn cmd_index_son_palabras() {
        let b = mensaje(&[5, 6, 7, 8], 2);
        assert_eq!(ordenes(&b).count(), 2, "2 palabras: las dos primeras ordenes del nucleo");
        // El del metal (24-09 08:48): 312 REG_WRITE, 104 REG_POLL y 4 del
        // nucleo son 1564 palabras, y 420 ordenes sin un error.
        let mut p = Vec::new();
        for k in 0..312u32 {
            p.extend_from_slice(&[0, 0x110114, k]);
        }
        for _ in 0..104 {
            p.extend_from_slice(&[2, 0x110118, 1, 0, 0, 0]);
        }
        p.extend_from_slice(&[5, 6, 7, 8]);
        assert_eq!(p.len(), 1564);
        let b = mensaje(&p, 1564);
        let v: Vec<_> = ordenes(&b).collect();
        assert_eq!(v.len(), 420);
        assert!(v.iter().all(|x| x.is_ok()));
        assert_eq!(v[419], Ok(Orden::Reanudar));
    }

    #[test]
    fn si_faltan_palabras_se_dice() {
        let mut b = mensaje(&[5, 6], 2);
        b.truncate(CABECERA + 4);
        assert_eq!(ordenes(&b).collect::<Vec<_>>(), [Ok(Orden::Resetear), Err(NoSe::Corta)]);
    }

    #[test]
    fn se_para_en_lo_que_no_entiende() {
        let b = mensaje(&[5, 9, 6], 3);
        let v: Vec<_> = ordenes(&b).collect();
        assert_eq!(v, [Ok(Orden::Resetear), Err(NoSe::Opcode(9))], "y no sigue");
        let b = mensaje(&[2, 0x110000, 1], 3);
        assert_eq!(ordenes(&b).collect::<Vec<_>>(), [Err(NoSe::Corta)], "una espera sin su carga entera");
        assert_eq!(ordenes(&[0u8; 10]).count(), 0, "sin cabecera, nada");
    }
}
