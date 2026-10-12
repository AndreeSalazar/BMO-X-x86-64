//! **LAS BALDOSAS MAESTRAS: la leccion de UMA, sin datos de nadie.**
//!
//! Una maestra se sombrea ENTERA (`Tasa::X1`) aunque UMA quisiera otra cosa.
//! De esa baldosa completa se saca, sin dibujar nada mas, como se habria
//! visto con cada tasa gruesa: lo que hace el silicio con el sombreado
//! variable es correr UNA vez por cuadro y repetir el color en todo el
//! cuadro ([`sombrear`]). Aqui se repite el pixel de arriba a la izquierda:
//! un poco PEOR que la muestra del centro del silicio, a proposito -- el
//! error se sobrestima y la decision se equivoca hacia la calidad.
//!
//! **El error, como lo ve el ojo** ([`error_entre`]): la diferencia media
//! dividida por la luz (Weber): la misma diferencia se ve menos en lo claro
//! que en lo oscuro... salvo en lo MUY oscuro, que el `WEBER` de abajo
//! frena para que una sombra negra no pida 4x4 por dividir entre casi cero.

use crate::pacha::{Baldosa, LADO, PIX};
use crate::Tasa;

/// Las tasas gruesas, en el orden de las salidas de UMA.
pub const GRUESAS: [Tasa; 4] = [Tasa::X2x1, Tasa::X1x2, Tasa::X2x2, Tasa::X4x4];

/// El suelo de la luz en el error de Weber.
pub const WEBER: f32 = 0.04;

/// Lo que daria el silicio con la tasa `t`: un color por cuadro.
pub fn sombrear(b: &Baldosa, t: Tasa) -> Baldosa {
    let (w, h) = t.bloque();
    let mut s = [0.0; PIX];
    for y in 0..LADO {
        for x in 0..LADO {
            s[y * LADO + x] = b[(y / h * h) * LADO + x / w * w];
        }
    }
    s
}

/// El error que ve el ojo entre la baldosa buena `a` y otra `b`.
pub fn error_entre(a: &Baldosa, b: &Baldosa) -> f32 {
    let media = a.iter().sum::<f32>() / PIX as f32;
    let dif = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / PIX as f32;
    dif / (WEBER + media)
}

/// El error de la tasa `t` en la baldosa entera `b`.
pub fn error_de(b: &Baldosa, t: Tasa) -> f32 {
    match t {
        Tasa::X1 | Tasa::Cero => 0.0,
        t => error_entre(b, &sombrear(b, t)),
    }
}

/// **La leccion**: el error de cada tasa gruesa, en el orden de [`GRUESAS`].
pub fn errores(b: &Baldosa) -> [f32; 4] {
    GRUESAS.map(|t| error_de(b, t))
}
