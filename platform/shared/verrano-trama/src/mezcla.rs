//! **La mezcla de D3D12** (03-10, N5.11): lo que el pixel nuevo hace con el
//! que ya esta en el render target (`D3D12_RENDER_TARGET_BLEND_DESC`).
//!
//! [carril]  VERDE     cuentas sobre cuatro floats; no toca la maquina
//! [cuesta]  DATO      un factor cambiado oscurece o quema la imagen entera
//! [riesgo]  ESPEJO    las reglas son las de D3D12 (los factores, las cinco
//!                     operaciones, la mascara por canal); el banco las
//!                     prueba con las mezclas que usa un juego
//! [consumo] NADA      solo cuando un PSO mezcla
//!
//! Hasta el 03-10 un Draw con mezcla se decia y NO se pintaba: la luz que se
//! suma, las particulas, el humo, el cristal y la interfaz de Cyberpunk.
//!
//! ```text
//!    color   d.rgb = o.rgb * F(origen) <op> d.rgb * F(destino)
//!    alfa    d.a   = o.a   * F(origen_a) <op_a> d.a * F(destino_a)
//!    op      sumar, restar (o - d), restar al reves (d - o), min, max
//!            (min y max no miran los factores)
//!    mascara el canal que no esta en `RenderTargetWriteMask` no cambia
//! ```
//!
//! Lo que no se sabe todavia se dice al crear el PSO: la operacion logica
//! (`LogicOpEnable`) y los factores de DOS fuentes (`SRC1_*`, que piden una
//! segunda salida del sombreador). Un render target sRGB se mezcla aqui en
//! sus bytes, no en lineal (la casa guarda 8 bits por canal).

/// Los factores (`D3D12_BLEND`).
pub const CERO: u8 = 1;
pub const UNO: u8 = 2;
pub const ORIGEN_COLOR: u8 = 3;
pub const INV_ORIGEN_COLOR: u8 = 4;
pub const ORIGEN_ALFA: u8 = 5;
pub const INV_ORIGEN_ALFA: u8 = 6;
pub const DESTINO_ALFA: u8 = 7;
pub const INV_DESTINO_ALFA: u8 = 8;
pub const DESTINO_COLOR: u8 = 9;
pub const INV_DESTINO_COLOR: u8 = 10;
pub const ORIGEN_ALFA_SAT: u8 = 11;
pub const FACTOR: u8 = 14;
pub const INV_FACTOR: u8 = 15;
pub const FACTOR_ALFA: u8 = 20;
pub const INV_FACTOR_ALFA: u8 = 21;

/// Las operaciones (`D3D12_BLEND_OP`).
pub const SUMAR: u8 = 1;
pub const RESTAR: u8 = 2;
pub const RESTAR_AL_REVES: u8 = 3;
pub const MIN: u8 = 4;
pub const MAX: u8 = 5;

/// **La mezcla de un render target.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mezcla {
    pub encendida: bool,
    pub origen: u8,
    pub destino: u8,
    pub op: u8,
    pub origen_a: u8,
    pub destino_a: u8,
    pub op_a: u8,
    /// R 1, G 2, B 4, A 8.
    pub mascara: u8,
}

impl Mezcla {
    /// Sin mezcla y los cuatro canales: el pixel nuevo, tal cual.
    pub const NINGUNA: Mezcla = Mezcla { encendida: false, origen: UNO, destino: CERO, op: SUMAR, origen_a: UNO, destino_a: CERO, op_a: SUMAR, mascara: 0xF };

    /// **De su descripcion de C** (`D3D12_RENDER_TARGET_BLEND_DESC`, 40
    /// bytes): BlendEnable +0, LogicOpEnable +4, SrcBlend +8, DestBlend +12,
    /// BlendOp +16, SrcBlendAlpha +20, DestBlendAlpha +24, BlendOpAlpha
    /// +28, LogicOp +32, RenderTargetWriteMask +36.
    pub fn de_desc(d: &[u8]) -> Result<Mezcla, &'static str> {
        let u = |o: usize| d.get(o..o + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if u(4) != 0 {
            return Err("una mezcla con operacion logica (LogicOpEnable): todavia no");
        }
        let m = Mezcla {
            encendida: u(0) != 0,
            origen: u(8) as u8,
            destino: u(12) as u8,
            op: u(16) as u8,
            origen_a: u(20) as u8,
            destino_a: u(24) as u8,
            op_a: u(28) as u8,
            mascara: d.get(36).copied().unwrap_or(0xF) & 0xF,
        };
        if m.encendida {
            for f in [m.origen, m.destino, m.origen_a, m.destino_a] {
                if !matches!(f, CERO..=ORIGEN_ALFA_SAT | FACTOR | INV_FACTOR | FACTOR_ALFA | INV_FACTOR_ALFA) {
                    return Err("una mezcla con factores de dos fuentes (SRC1) o que no existen: todavia no");
                }
            }
            for op in [m.op, m.op_a] {
                if !(SUMAR..=MAX).contains(&op) {
                    return Err("una mezcla con una operacion que no existe");
                }
            }
        }
        Ok(m)
    }

    /// Si no cambia nada: el pixel nuevo se escribe tal cual.
    pub fn trivial(&self) -> bool {
        !self.encendida && self.mascara == 0xF
    }

    /// **El color que queda**: el nuevo `o` sobre el que estaba `d`, con el
    /// factor de mezcla `k` (`OMSetBlendFactor`).
    pub fn aplicar(&self, o: [f32; 4], d: [f32; 4], k: [f32; 4]) -> [f32; 4] {
        let mezclado = if self.encendida {
            core::array::from_fn(|c| {
                let (fo, fd, op) = if c < 3 { (self.origen, self.destino, self.op) } else { (self.origen_a, self.destino_a, self.op_a) };
                let (a, b) = (o[c] * factor(fo, c, o, d, k), d[c] * factor(fd, c, o, d, k));
                match op {
                    RESTAR => a - b,
                    RESTAR_AL_REVES => b - a,
                    MIN => o[c].min(d[c]),
                    MAX => o[c].max(d[c]),
                    _ => a + b,
                }
            })
        } else {
            o
        };
        core::array::from_fn(|c| if self.mascara & (1 << c) != 0 { mezclado[c] } else { d[c] })
    }
}

/// El factor `f` para el canal `c` (3 es el alfa).
fn factor(f: u8, c: usize, o: [f32; 4], d: [f32; 4], k: [f32; 4]) -> f32 {
    match f {
        CERO => 0.0,
        ORIGEN_COLOR => o[c],
        INV_ORIGEN_COLOR => 1.0 - o[c],
        ORIGEN_ALFA => o[3],
        INV_ORIGEN_ALFA => 1.0 - o[3],
        DESTINO_ALFA => d[3],
        INV_DESTINO_ALFA => 1.0 - d[3],
        DESTINO_COLOR => d[c],
        INV_DESTINO_COLOR => 1.0 - d[c],
        // min(o.a, 1 - d.a) en el color; 1 en el alfa.
        ORIGEN_ALFA_SAT if c < 3 => o[3].min(1.0 - d[3]),
        FACTOR => k[c],
        INV_FACTOR => 1.0 - k[c],
        FACTOR_ALFA => k[3],
        INV_FACTOR_ALFA => 1.0 - k[3],
        _ => 1.0,
    }
}

/// **Las mezclas de un dibujo**: la de cada render target (el SV_Target
/// `i`, la `rt[i]`) y el factor de `OMSetBlendFactor`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mezclas {
    pub rt: [Mezcla; 8],
    pub factor: [f32; 4],
}

impl Mezclas {
    pub const NINGUNA: Mezclas = Mezclas { rt: [Mezcla::NINGUNA; 8], factor: [1.0; 4] };

    /// Si ningun render target mezcla ni enmascara.
    pub fn trivial(&self) -> bool {
        self.rt.iter().all(Mezcla::trivial)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn desc(enc: bool, o: u8, d: u8, op: u8, oa: u8, da: u8, opa: u8, mascara: u8) -> [u8; 40] {
        let mut x = [0u8; 40];
        for (k, v) in [enc as u32, 0, o as u32, d as u32, op as u32, oa as u32, da as u32, opa as u32].into_iter().enumerate() {
            x[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
        }
        x[36] = mascara;
        x
    }

    const O: [f32; 4] = [1.0, 0.5, 0.25, 0.5];
    const D: [f32; 4] = [0.0, 0.5, 1.0, 1.0];
    const K: [f32; 4] = [1.0; 4];

    /// La transparencia de siempre (SRC_ALPHA, INV_SRC_ALPHA): mitad y mitad.
    #[test]
    fn la_transparencia_mezcla_por_el_alfa() {
        let m = Mezcla::de_desc(&desc(true, ORIGEN_ALFA, INV_ORIGEN_ALFA, SUMAR, UNO, INV_ORIGEN_ALFA, SUMAR, 0xF)).unwrap();
        assert_eq!(m.aplicar(O, D, K), [0.5, 0.5, 0.625, 1.0]);
    }

    /// La luz que se SUMA (ONE, ONE), y las otras operaciones.
    #[test]
    fn sumar_restar_min_y_max() {
        let con = |op| Mezcla::de_desc(&desc(true, UNO, UNO, op, UNO, UNO, op, 0xF)).unwrap().aplicar(O, D, K);
        assert_eq!(con(SUMAR), [1.0, 1.0, 1.25, 1.5], "sin saturar aqui: lo satura quien lo guarda");
        assert_eq!(con(RESTAR), [1.0, 0.0, -0.75, -0.5]);
        assert_eq!(con(RESTAR_AL_REVES), [-1.0, 0.0, 0.75, 0.5]);
        assert_eq!(con(MIN), [0.0, 0.5, 0.25, 0.5]);
        assert_eq!(con(MAX), [1.0, 0.5, 1.0, 1.0]);
    }

    /// El factor de mezcla, el del destino y el que satura.
    #[test]
    fn los_factores_que_usan_los_juegos() {
        let k = [0.5, 0.25, 0.0, 0.75];
        let m = Mezcla::de_desc(&desc(true, FACTOR, INV_FACTOR_ALFA, SUMAR, ORIGEN_ALFA_SAT, CERO, SUMAR, 0xF)).unwrap();
        assert_eq!(m.aplicar(O, D, k), [0.5, 0.25, 0.25, 0.5], "o * k + d * (1 - k.a); en el alfa SAT es 1");
        let m = Mezcla::de_desc(&desc(true, DESTINO_COLOR, CERO, SUMAR, UNO, CERO, SUMAR, 0xF)).unwrap();
        assert_eq!(m.aplicar(O, D, K), [0.0, 0.25, 0.25, 0.5], "multiplicar (la sombra, la suciedad)");
        let m = Mezcla::de_desc(&desc(true, ORIGEN_ALFA_SAT, UNO, SUMAR, UNO, CERO, SUMAR, 0xF)).unwrap();
        assert_eq!(m.aplicar(O, D, K), [0.0, 0.5, 1.0, 0.5], "min(o.a, 1 - d.a) = 0");
    }

    /// La mascara: lo que no esta, no cambia (con o sin mezcla).
    #[test]
    fn la_mascara_deja_los_canales_que_no_estan() {
        let m = Mezcla::de_desc(&desc(false, UNO, CERO, SUMAR, UNO, CERO, SUMAR, 0b0101)).unwrap();
        assert!(!m.trivial());
        assert_eq!(m.aplicar(O, D, K), [1.0, 0.5, 0.25, 1.0]);
        assert!(Mezcla::de_desc(&desc(false, 0, 0, 0, 0, 0, 0, 0xF)).unwrap().trivial(), "apagada y RGBA: nada que hacer");
    }

    #[test]
    fn lo_que_no_se_sabe_se_dice() {
        let mut d = desc(true, UNO, UNO, SUMAR, UNO, UNO, SUMAR, 0xF);
        d[4] = 1;
        assert!(Mezcla::de_desc(&d).is_err(), "LogicOpEnable");
        assert!(Mezcla::de_desc(&desc(true, 16, UNO, SUMAR, UNO, UNO, SUMAR, 0xF)).is_err(), "SRC1_COLOR");
        assert!(Mezcla::de_desc(&desc(true, UNO, UNO, 9, UNO, UNO, SUMAR, 0xF)).is_err());
    }
}
