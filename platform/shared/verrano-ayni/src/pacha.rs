//! **PACHA: lo que UMA mira de cada baldosa** (el espacio y el tiempo).
//!
//! Todo sale del fotograma ANTERIOR (ya mostrado) y del de antes de ese: la
//! decision es para el SIGUIENTE. Eso es la "4D" del propietario dicha en
//! numeros: la CPU ve la serie de cortes, la GPU dibuja el corte.
//!
//! Las entradas van todas en `[0, 1]`, y en este orden:
//!
//! ```text
//!    0 LUZ          la luminancia media (el ojo ve menos en la sombra)
//!    1 CONTRASTE    la desviacion media de la luz, x2
//!    2 BORDE        el salto mas grande entre dos vecinos
//!    3 DETALLE      el salto MEDIO entre vecinos (textura, no un solo borde)
//!    4 CAMBIO       lo que cambio desde el fotograma de antes (la 4D)
//!    5 SALTO_Z      el salto de profundidad (de quien dibuja, si lo da)
//!    6 MOVIMIENTO   cuanto se mueve (vectores, si los da)
//!    7 TRIANGULOS   cuantos triangulos la cruzan (PROTON-X lo sabe)
//! ```

/// El lado de una baldosa: 16, como la imagen de tasas de AMPERE_B.
pub const LADO: usize = 16;
pub const PIX: usize = LADO * LADO;
/// Las entradas de UMA.
pub const ENTRADAS: usize = 8;

pub const LUZ: usize = 0;
pub const CONTRASTE: usize = 1;
pub const BORDE: usize = 2;
pub const DETALLE: usize = 3;
pub const CAMBIO: usize = 4;
pub const SALTO_Z: usize = 5;
pub const MOVIMIENTO: usize = 6;
pub const TRIANGULOS: usize = 7;

/// Una baldosa de luminancia, fila a fila.
pub type Baldosa = [f32; PIX];

/// **Una imagen de luminancia** en `[0, 1]`, fila a fila. Quien la da elige
/// como la saca del color (la 3060: una pasada; la CPU: al vuelo).
#[derive(Clone, Copy)]
pub struct Imagen<'a> {
    pub l: &'a [f32],
    pub ancho: usize,
    pub alto: usize,
}

impl<'a> Imagen<'a> {
    pub fn columnas(&self) -> usize {
        self.ancho.div_ceil(LADO)
    }

    pub fn filas(&self) -> usize {
        self.alto.div_ceil(LADO)
    }

    /// La baldosa `(bx, by)`. En el borde derecho o de abajo, la ultima fila
    /// o columna se repite: una baldosa partida no inventa contraste.
    pub fn baldosa(&self, bx: usize, by: usize) -> Baldosa {
        let mut b = [0.0; PIX];
        for y in 0..LADO {
            let fy = (by * LADO + y).min(self.alto - 1);
            for x in 0..LADO {
                let fx = (bx * LADO + x).min(self.ancho - 1);
                b[y * LADO + x] = self.l[fy * self.ancho + fx];
            }
        }
        b
    }
}

/// **Lo que solo sabe quien dibuja**, por baldosa. Todo opcional: sin
/// PROTON-X, AYNI mira solo los pixeles.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Extra {
    pub salto_z: f32,
    pub movimiento: f32,
    pub triangulos: f32,
    /// Un lote de INTERFAZ (texto, HUD) la toca.
    pub interfaz: bool,
    /// Quien dibuja GARANTIZA que ningun lote la toco distinto (A2).
    pub sin_cambios: bool,
    /// Un borde de geometria la cruza (una arista de silueta).
    pub borde_geometria: bool,
}

/// Los rasgos de una baldosa.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rasgos {
    pub entrada: [f32; ENTRADAS],
    /// La luz cambia MAS a lo ancho que a lo alto: el borde es vertical y se
    /// puede engrosar a lo alto (`X1x2`), no a lo ancho.
    pub cambia_a_lo_ancho: bool,
}

fn uno(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// **Los rasgos de una baldosa**, con la de antes si la hay.
pub fn rasgos(b: &Baldosa, antes: Option<&Baldosa>, e: &Extra) -> Rasgos {
    let media = b.iter().sum::<f32>() / PIX as f32;
    let contraste = b.iter().map(|v| (v - media).abs()).sum::<f32>() / PIX as f32;
    let (mut dx, mut dy, mut borde) = (0.0f32, 0.0f32, 0.0f32);
    for y in 0..LADO {
        for x in 0..LADO {
            let v = b[y * LADO + x];
            if x + 1 < LADO {
                let d = (b[y * LADO + x + 1] - v).abs();
                dx += d;
                borde = borde.max(d);
            }
            if y + 1 < LADO {
                let d = (b[(y + 1) * LADO + x] - v).abs();
                dy += d;
                borde = borde.max(d);
            }
        }
    }
    let saltos = (2 * LADO * (LADO - 1)) as f32;
    let mut r = Rasgos { entrada: [0.0; ENTRADAS], cambia_a_lo_ancho: dx >= dy };
    r.entrada[LUZ] = uno(media);
    r.entrada[CONTRASTE] = uno(contraste * 2.0);
    r.entrada[BORDE] = uno(borde);
    r.entrada[DETALLE] = uno((dx + dy) / saltos);
    r.entrada[CAMBIO] = antes.map_or(0.0, |a| uno(cambio(b, a)));
    r.entrada[SALTO_Z] = uno(e.salto_z);
    r.entrada[MOVIMIENTO] = uno(e.movimiento);
    r.entrada[TRIANGULOS] = uno(e.triangulos);
    r
}

/// Lo que cambio entre dos baldosas: la diferencia media.
pub fn cambio(b: &Baldosa, antes: &Baldosa) -> f32 {
    b.iter().zip(antes).map(|(x, y)| (x - y).abs()).sum::<f32>() / PIX as f32
}
