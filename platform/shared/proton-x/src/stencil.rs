//! **El STENCIL de D3D12** (05-10): la prueba y las operaciones que cada
//! pixel hace con su byte del plano de stencil (`D3D12_DEPTH_STENCIL_DESC`).
//!
//! [carril]  VERDE     cuentas sobre un byte; no toca la maquina
//! [cuesta]  DATO      una regla cambiada recorta de mas o de menos (la luz
//!                     de un juego que marca con stencil donde va cada una)
//! [riesgo]  ESPEJO    las reglas son las de D3D12; el juez es `stencil.exe`
//!                     (`prueba/`), que dice lo mismo en Windows
//! [consumo] NADA      solo cuando el PSO lo enciende y el DSV lo tiene
//!
//! Hasta el 05-10 el PSO lo apuntaba y no lo usaba: un juego que recorta con
//! stencil (Cyberpunk: las mascaras de luz y las calcomanias) pintaba de mas.
//!
//! ```text
//!    la cara     la de delante o la de detras, por el giro del triangulo
//!    la prueba   (referencia & lectura) FUNC (guardado & lectura): la
//!                referencia a la IZQUIERDA (LESS pasa si ref < guardado)
//!    la op       stencil NO pasa                 StencilFailOp
//!                pasa y la profundidad NO        StencilDepthFailOp
//!                pasan las dos                   StencilPassOp
//!    escribir    (guardado & !escritura) | (nuevo & escritura)
//!    REPLACE     la referencia ENTERA (sin la mascara de lectura)
//! ```
//!
//! Un pixel que el sombreador TIRA (`discard`) no llega a la salida: no
//! cambia el stencil, ni con la operacion de fallo (la trama corre el de
//! pixeles para saberlo cuando la operacion cambiaria algo).

/// Las operaciones (`D3D12_STENCIL_OP`).
pub const KEEP: u8 = 1;
pub const ZERO: u8 = 2;
pub const REPLACE: u8 = 3;
pub const INCR_SAT: u8 = 4;
pub const DECR_SAT: u8 = 5;
pub const INVERT: u8 = 6;
pub const INCR: u8 = 7;
pub const DECR: u8 = 8;

/// **Una cara** (`D3D12_DEPTH_STENCILOP_DESC`), con las mascaras y la
/// referencia que le tocan (en D3D12 son de las dos; `OMSetFrontAndBackStencilRef`
/// da una referencia a cada una).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cara {
    pub falla: u8,
    pub falla_z: u8,
    pub pasa: u8,
    /// `D3D12_COMPARISON_FUNC` (1 nunca .. 8 siempre), como la profundidad.
    pub funcion: u8,
    pub lectura: u8,
    pub escritura: u8,
    pub referencia: u8,
}

/// **El stencil de un dibujo**: sus dos caras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stencil {
    pub delante: Cara,
    pub detras: Cara,
}

impl Cara {
    /// Si el pixel pasa contra `guardado` (su byte del plano).
    pub fn prueba(&self, guardado: u8) -> bool {
        let (r, g) = (self.referencia & self.lectura, guardado & self.lectura);
        match self.funcion {
            1 => false,
            2 => r < g,
            3 => r == g,
            4 => r <= g,
            5 => r > g,
            6 => r != g,
            7 => r >= g,
            _ => true,
        }
    }

    /// El byte que queda tras la operacion `op`, con la mascara de escritura.
    pub fn aplicar(&self, op: u8, guardado: u8) -> u8 {
        let nuevo = match op {
            ZERO => 0,
            REPLACE => self.referencia,
            INCR_SAT => guardado.saturating_add(1),
            DECR_SAT => guardado.saturating_sub(1),
            INVERT => !guardado,
            INCR => guardado.wrapping_add(1),
            DECR => guardado.wrapping_sub(1),
            _ => guardado,
        };
        (guardado & !self.escritura) | (nuevo & self.escritura)
    }

    /// Si la operacion `op` puede cambiar el byte (para saber si hay que
    /// correr el de pixeles de un pixel que no pasa: ver la cabecera).
    pub fn cambia(&self, op: u8) -> bool {
        op != KEEP && self.escritura != 0
    }
}

impl Stencil {
    /// **De un `D3D12_DEPTH_STENCIL_DESC`** (52 B): StencilEnable +12,
    /// StencilReadMask +16, StencilWriteMask +17, FrontFace +20 y BackFace
    /// +36 (StencilFailOp, StencilDepthFailOp, StencilPassOp, StencilFunc:
    /// un u32 cada uno). `Ok(None)` si esta apagado; la referencia, 0 (la
    /// pone la lista: `OMSetStencilRef`).
    pub fn de_desc(d: &[u8]) -> Result<Option<Stencil>, &'static str> {
        let u = |o: usize| d.get(o..o + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if u(12) == 0 {
            return Ok(None);
        }
        let (lectura, escritura) = (d.get(16).copied().unwrap_or(0), d.get(17).copied().unwrap_or(0));
        let cara = |o: usize| {
            let [falla, falla_z, pasa, funcion] = [u(o), u(o + 4), u(o + 8), u(o + 12)];
            if ![falla, falla_z, pasa].iter().all(|x| (1..=8).contains(x)) || !(1..=8).contains(&funcion) {
                return Err("CreateGraphicsPipelineState con una operacion o una funcion de stencil fuera de D3D12: en Windows es E_INVALIDARG");
            }
            Ok(Cara { falla: falla as u8, falla_z: falla_z as u8, pasa: pasa as u8, funcion: funcion as u8, lectura, escritura, referencia: 0 })
        };
        Ok(Some(Stencil { delante: cara(20)?, detras: cara(36)? }))
    }

    /// Con las referencias de la lista (delante, detras).
    pub fn con_referencia(mut self, r: [u8; 2]) -> Stencil {
        self.delante.referencia = r[0];
        self.detras.referencia = r[1];
        self
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cara(funcion: u8, lectura: u8, escritura: u8, referencia: u8) -> Cara {
        Cara { falla: KEEP, falla_z: KEEP, pasa: KEEP, funcion, lectura, escritura, referencia }
    }

    #[test]
    fn la_prueba_pone_la_referencia_a_la_izquierda_y_enmascara_las_dos() {
        // LESS: pasa si ref < guardado (no al reves).
        assert!(cara(2, 0xFF, 0xFF, 1).prueba(2));
        assert!(!cara(2, 0xFF, 0xFF, 2).prueba(1));
        // EQUAL con la mascara de lectura: 0x13 & 0x0F == 0x23 & 0x0F.
        assert!(cara(3, 0x0F, 0xFF, 0x13).prueba(0x23));
        assert!(!cara(3, 0xFF, 0xFF, 0x13).prueba(0x23));
        assert!(!cara(1, 0xFF, 0xFF, 0).prueba(0) && cara(8, 0xFF, 0xFF, 9).prueba(0));
    }

    #[test]
    fn las_ocho_operaciones_y_la_mascara_de_escritura() {
        let c = cara(8, 0xFF, 0xFF, 0x5A);
        let hecho: [u8; 8] = core::array::from_fn(|k| c.aplicar(k as u8 + 1, 0xFF));
        assert_eq!(hecho, [0xFF, 0, 0x5A, 0xFF, 0xFE, 0, 0, 0xFE]);
        assert_eq!((c.aplicar(INCR_SAT, 0), c.aplicar(DECR_SAT, 0), c.aplicar(DECR, 0)), (1, 0, 0xFF));
        // Escritura 0x0F: lo de arriba no cambia; REPLACE da la referencia
        // entera, sin la mascara de lectura.
        let m = cara(8, 0x01, 0x0F, 0x5A);
        assert_eq!(m.aplicar(REPLACE, 0xF0), 0xFA);
        assert_eq!(m.aplicar(INVERT, 0x35), 0x3A);
        assert!(!m.cambia(KEEP) && m.cambia(ZERO) && !cara(8, 0xFF, 0, 0).cambia(ZERO));
    }

    #[test]
    fn se_lee_del_desc_de_d3d12() {
        let mut d = [0u8; 52];
        let mut u = |o: usize, v: u32| d[o..o + 4].copy_from_slice(&v.to_le_bytes());
        u(12, 1);
        for (o, v) in [(20, 2), (24, 6), (28, 3), (32, 3), (36, 4), (40, 5), (44, 7), (48, 8)] {
            u(o, v);
        }
        d[16] = 0xF0;
        d[17] = 0x0F;
        let s = Stencil::de_desc(&d).unwrap().unwrap().con_referencia([7, 9]);
        assert_eq!(s.delante, Cara { falla: ZERO, falla_z: INVERT, pasa: REPLACE, funcion: 3, lectura: 0xF0, escritura: 0x0F, referencia: 7 });
        assert_eq!((s.detras.falla, s.detras.falla_z, s.detras.pasa, s.detras.funcion, s.detras.referencia), (INCR_SAT, DECR_SAT, INCR, 8, 9));
        d[12] = 0;
        assert_eq!(Stencil::de_desc(&d), Ok(None));
        d[12] = 1;
        d[20] = 9;
        assert!(Stencil::de_desc(&d).is_err());
    }

    use crate::trama;
    use alloc::vec::Vec;

    /// Un triangulo que cubre los 8x8 a z = 0.5, en el giro que se pida.
    fn dibujar(st: Stencil, giro_horario: bool, z: Option<&mut [u32]>, plano: &mut [u8], px: &mut [u32], queda: bool) -> trama::Cuenta {
        let mut esquinas = [[-1.0f32, -1.0], [-1.0, 3.0], [3.0, -1.0]];
        if !giro_horario {
            esquinas.swap(1, 2);
        }
        let v: Vec<trama::Sombreado> = esquinas.iter().map(|&[x, y]| trama::Sombreado { pos: [x, y, 0.5, 1.0], atributos: Vec::new() }).collect();
        let profundidad = z.is_some().then_some(trama::Profundidad { funcion: 2, escribir: false });
        let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte: 1, antihorario: false, profundidad, mezcla: crate::mezcla::Mezclas::NINGUNA, z_del_sombreador: false, stencil: Some(st) };
        let mut d = trama::Destino { pixeles: px, ancho: 8, alto: 8, bgra: false, z, cadena: false, otros: &mut [], flotante: None, stencil: Some(plano) };
        trama::dibujar(&reglas, &v, &[[0, 1, 2]], &mut d, None, |_, c| {
            c[0] = [1.0; 4];
            queda
        })
    }

    /// *** En la trama: la cara de DELANTE y la de DETRAS hacen lo suyo (el
    /// mismo triangulo, girado, INVIERTE lo que el otro REEMPLAZO).
    #[test]
    fn la_trama_elige_la_cara_por_el_giro() {
        let delante = Cara { falla: KEEP, falla_z: KEEP, pasa: REPLACE, funcion: 8, lectura: 0xFF, escritura: 0xFF, referencia: 0 };
        let st = Stencil { delante, detras: Cara { pasa: INVERT, ..delante } }.con_referencia([1, 5]);
        let (mut plano, mut px) = ([0u8; 64], [0u32; 64]);
        dibujar(st, true, None, &mut plano, &mut px, true);
        assert_eq!(plano, [1; 64], "delante: REPLACE con la referencia de delante");
        dibujar(st, false, None, &mut plano, &mut px, true);
        assert_eq!(plano, [0xFE; 64], "detras: INVERT de 1");
    }

    /// *** Las tres operaciones por resultado, y lo TIRADO no cambia nada:
    /// el plano con 1 en las 4 columnas de la izquierda, la Z de las 4 filas
    /// de arriba DELANTE (0.25: LESS no pasa) y el resto detras (1.0); EQUAL
    /// a 1. Arriba a la izquierda falla la Z (INVERT: 0xFE), abajo pasan las
    /// dos (ZERO, y se pinta), a la derecha falla el stencil (INCR_SAT: 1).
    #[test]
    fn la_trama_aplica_la_operacion_de_cada_resultado() {
        let c = Cara { falla: INCR_SAT, falla_z: INVERT, pasa: ZERO, funcion: 3, lectura: 0xFF, escritura: 0xFF, referencia: 1 };
        let st = Stencil { delante: c, detras: c };
        let inicio: [u8; 64] = core::array::from_fn(|k| (k % 8 < 4) as u8);
        let z0: Vec<u32> = (0..64).map(|k| if k / 8 < 4 { 0.25f32 } else { 1.0 }.to_bits()).collect();
        let (mut plano, mut px, mut z) = (inicio, [0u32; 64], z0.clone());
        let cuenta = dibujar(st, true, Some(&mut z), &mut plano, &mut px, true);
        for k in 0..64 {
            let (x, y) = (k % 8, k / 8);
            let quiero = match (x < 4, y < 4) {
                (true, true) => (0xFE, 0),
                (true, false) => (0, 0xFFFF_FFFF),
                _ => (1, 0),
            };
            assert_eq!((plano[k], px[k]), quiero, "({x}, {y})");
        }
        assert_eq!((cuenta.pasan, cuenta.tapados), (16, 48));
        // El de pixeles lo tira todo: ni color ni stencil (ni las de fallo).
        let (mut plano, mut px, mut z) = (inicio, [0u32; 64], z0);
        dibujar(st, true, Some(&mut z), &mut plano, &mut px, false);
        assert_eq!((plano, px), (inicio, [0; 64]));
    }
}
