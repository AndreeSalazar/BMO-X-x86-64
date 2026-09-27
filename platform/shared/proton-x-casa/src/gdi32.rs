//! **`gdi32.dll` de la casa** (P2, 27-09): poner pixeles de la CPU en una
//! ventana.
//!
//! `StretchDIBits` es como pinta un programa de Windows que dibuja con la CPU
//! en un bufer suyo (los emuladores, los juegos de software, muchos ejemplos):
//! un DIB de 32 bits y un `SRCCOPY` a la ventana. Aqui cae en la superficie
//! del escritorio de BMO-X con `bmo_proton_x::ventanas::copiar_dib`, que es
//! puro y lo prueba su banco.
//!
//! Lo que todavia no sabe --estirar, un trozo del DIB, menos de 32 bits, otra
//! operacion que `SRCCOPY`-- contesta 0 (el fallo de GDI) y lo dice con
//! [`aviso`]: nunca "0 filas" callado ni un dibujo a medias.

use bmo_proton_x::ventanas::{copiar_dib, leer_dib, NoPinta, Rect};

use crate::user32::{superficie_de, BIT_HDC};
use crate::{aviso, dir};

const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;

#[allow(clippy::too_many_arguments)]
extern "win64" fn stretch_di_bits(
    hdc: u64,
    xd: i32,
    yd: i32,
    wd: i32,
    hd: i32,
    xs: i32,
    ys: i32,
    ws: i32,
    hs: i32,
    bits: *const u8,
    bmi: *const u8,
    uso: u32,
    rop: u32,
) -> i32 {
    if hdc & BIT_HDC == 0 || bits.is_null() || bmi.is_null() {
        return 0;
    }
    if uso != DIB_RGB_COLORS || rop != SRCCOPY {
        aviso("StretchDIBits: solo DIB_RGB_COLORS y SRCCOPY, todavia");
        return 0;
    }
    let Some(sup) = superficie_de(hdc & !BIT_HDC) else { return 0 };
    // SAFETY: un BITMAPINFOHEADER suyo: 40 bytes legibles.
    let cab = unsafe { core::slice::from_raw_parts(bmi, 40) };
    let dib = match leer_dib(cab) {
        Ok(d) => d,
        Err(e) => {
            aviso(motivo(e));
            return 0;
        }
    };
    // SAFETY: el DIB promete `ancho * alto` pixeles de 4 bytes en `bits`.
    let fuente = unsafe { core::slice::from_raw_parts(bits, dib.ancho as usize * dib.alto as usize * 4) };
    // SAFETY: la superficie mide `stride * alto` pixeles y es de este proceso.
    let destino = unsafe { core::slice::from_raw_parts_mut(sup.pixeles, sup.stride as usize * sup.alto as usize) };
    let dst = Rect { x: xd, y: yd, ancho: wd, alto: hd };
    let origen = Rect { x: xs, y: ys, ancho: ws, alto: hs };
    match copiar_dib(destino, sup.ancho, sup.alto, sup.stride, dst, origen, fuente, &dib) {
        Ok(filas) => filas as i32,
        Err(e) => {
            aviso(motivo(e));
            0
        }
    }
}

fn motivo(e: NoPinta) -> &'static str {
    match e {
        NoPinta::Cabecera => "StretchDIBits: la cabecera no es un BITMAPINFOHEADER",
        NoPinta::Formato { .. } => "StretchDIBits: solo 32 bits por pixel sin comprimir, todavia",
        NoPinta::Escala => "StretchDIBits: todavia no ESTIRA (origen y destino del mismo medida)",
        NoPinta::Trozo => "StretchDIBits: solo el DIB entero, todavia",
        NoPinta::Corto => "StretchDIBits: los pixeles no llegan a lo que dice la cabecera",
    }
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "StretchDIBits" => dir!(stretch_di_bits),
        _ => return None,
    })
}
