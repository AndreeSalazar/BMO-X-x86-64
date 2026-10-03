//! **EL SAVE VIVO** (03-10): lo que `save siempre` deja en el disco cada
//! poco mientras corre un programa, para que una maquina CONGELADA no se
//! lleve la prueba.
//!
//! Pedido por el propietario: *"automatizar la escritura en save mode"*. En
//! el metal, Cyberpunk se quedo girando sin ventana, y lo unico que habia era
//! el `^C` y un `save` tecleado DESPUES. Si la maquina se congela entera, no
//! hay despues.
//!
//! ```text
//!    la forma      una cabecera (`# `), lo que escribio el programa (cada
//!                  linea distinta una vez, con su (xN)), y las ultimas filas
//!                  de CABINA (una racha igual, una vez con su (xN))
//!    el final      la linea [`MARCA_FIN`] solo la pone el save del FINAL,
//!                  cuando el programa acabo. Un vivo SIN ella es una maquina
//!                  que se quedo colgada con el programa corriendo: el
//!                  siguiente arranque lo ve ([`acabo`]) y lo dice
//! ```
//!
//! Va directo al fichero y no por la pantalla, al reves que el `save`
//! entero: pintar un informe de 400 filas cada 30 s enterraria en un anillo
//! de 200 lo que el programa esta diciendo, que es justo lo que se guarda.

use crate::{decimal, Registro};

/// La ultima linea de un vivo cuyo programa ACABO.
pub const MARCA_FIN: &[u8] = b"# ACABO";

/// Lo que va en la cabecera.
pub struct Cabecera<'a> {
    /// `AAAA-MM-DD HH:MM` de la placa, o vacio si no la da.
    pub fecha: &'a [u8],
    /// Lo que se lanzo, como se tecleo.
    pub programa: &'a [u8],
    /// Cuanto lleva corriendo.
    pub segundos: u64,
    /// El numero de este save vivo desde que se lanzo (1 el primero).
    pub vuelta: u32,
}

/// **Componer el vivo**: la cabecera, el registro y las filas de la sesion
/// (en orden, la mas vieja primero), con [`MARCA_FIN`] al final si `fin`.
/// Cada trozo sale por `sal`, con finales `\r\n` (se lee en Windows).
pub fn componer<const N: usize>(c: &Cabecera, r: &Registro<N>, sesion: &mut dyn Iterator<Item = &[u8]>, fin: bool, sal: &mut dyn FnMut(&[u8])) {
    let mut d = [0u8; 20];
    let mut num = |v: u64, sal: &mut dyn FnMut(&[u8])| {
        let k = decimal(v, &mut d);
        sal(&d[..k]);
    };
    sal(b"# SAVE VIVO de CABINA (`save siempre`): se reescribe cada poco mientras corre un programa\r\n");
    sal(b"# programa: ");
    sal(if c.programa.is_empty() { b"(sin nombre)" } else { c.programa });
    sal(b"\r\n# hora: ");
    sal(if c.fecha.is_empty() { b"(la placa no la da)" } else { c.fecha });
    sal(b"\r\n# corriendo: ");
    num(c.segundos, sal);
    sal(b" s; save vivo numero ");
    num(c.vuelta as u64, sal);
    sal(b"\r\n# si este fichero NO acaba en \"");
    sal(MARCA_FIN);
    sal(b"\", la maquina se quedo colgada con el programa corriendo\r\n\r\n");

    sal(b"== lo que escribio el programa, cada linea una vez: ");
    num(r.lineas, sal);
    sal(b" lineas, ");
    num(r.distintas() as u64, sal);
    sal(b" distintas ==\r\n");
    let mut linea = [0u8; crate::LARGO + 32];
    for e in (0..r.distintas()).filter_map(|i| r.entrada(i)) {
        let k = Registro::<N>::con_veces(e, &mut linea);
        sal(&linea[..k]);
        sal(b"\r\n");
    }
    if r.sin_sitio > 0 {
        sal(b"(y ");
        num(r.sin_sitio, sal);
        sal(b" lineas distintas mas que no cupieron)\r\n");
    }

    sal(b"\r\n== las ultimas filas de CABINA ==\r\n");
    let mut antes: Option<&[u8]> = None;
    let mut veces = 0u64;
    let soltar = |l: &[u8], veces: u64, sal: &mut dyn FnMut(&[u8])| {
        sal(l);
        if veces > 1 {
            sal(b"  (x");
            let mut d = [0u8; 20];
            let k = decimal(veces, &mut d);
            sal(&d[..k]);
            sal(b")");
        }
        sal(b"\r\n");
    };
    for l in sesion {
        match antes {
            Some(a) if a == l => veces += 1,
            _ => {
                if let Some(a) = antes {
                    soltar(a, veces, sal);
                }
                antes = Some(l);
                veces = 1;
            }
        }
    }
    if let Some(a) = antes {
        soltar(a, veces, sal);
    }
    if fin {
        sal(b"\r\n");
        sal(MARCA_FIN);
        sal(b"\r\n");
    }
}

/// **Si el programa del vivo ACABO**: su ultima linea con texto es
/// [`MARCA_FIN`]. Un vivo vacio o cortado no acabo.
pub fn acabo(vivo: &[u8]) -> bool {
    vivo.split(|&b| b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)).filter(|l| !l.is_empty()).last() == Some(MARCA_FIN)
}

/// **El programa de un vivo**, de su cabecera.
pub fn programa(vivo: &[u8]) -> Option<&[u8]> {
    vivo.split(|&b| b == b'\n').find_map(|l| l.strip_prefix(b"# programa: ")).map(|l| l.strip_suffix(b"\r").unwrap_or(l))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vivo(fin: bool, sesion: &[&[u8]]) -> Vec<u8> {
        let mut r: Registro<8> = Registro::nuevo();
        r.escribir(b"GetCurrentDirectoryW: D:\\x\nGetCurrentDirectoryW: D:\\x\nCreateDevice: bien\n");
        let c = Cabecera { fecha: b"2026-10-03 14:05", programa: b"personal diario Cyberpunk2077.exe", segundos: 95, vuelta: 3 };
        let mut v = Vec::new();
        componer(&c, &r, &mut sesion.iter().copied(), fin, &mut |b| v.extend_from_slice(b));
        v
    }

    #[test]
    fn dice_lo_que_escribio_sin_repetir_y_la_sesion() {
        let v = vivo(false, &[b"> personal diario Cyberpunk2077.exe", b"  lanzado", b"  lanzado", b"  lanzado"]);
        let t = String::from_utf8(v).unwrap();
        assert!(t.contains("# programa: personal diario Cyberpunk2077.exe\r\n"), "{t}");
        assert!(t.contains("# corriendo: 95 s; save vivo numero 3\r\n"), "{t}");
        assert!(t.contains("3 lineas, 2 distintas"), "{t}");
        assert!(t.contains("GetCurrentDirectoryW: D:\\x  (x2)\r\nCreateDevice: bien\r\n"), "{t}");
        assert!(t.contains("> personal diario Cyberpunk2077.exe\r\n  lanzado  (x3)\r\n"), "{t}");
    }

    /// Sin la marca, la maquina se quedo colgada; con ella, acabo.
    #[test]
    fn la_marca_dice_si_acabo() {
        let colgado = vivo(false, &[b"x"]);
        let acabado = vivo(true, &[b"x"]);
        assert!(!acabo(&colgado));
        assert!(acabo(&acabado));
        assert!(!acabo(b""));
        // Cortado a medias (el disco no llego a escribir el final): no acabo.
        assert!(!acabo(&acabado[..acabado.len() - 4]));
        // La marca en medio (una linea del programa que la copia) no cuenta.
        assert!(!acabo(b"# ACABO\r\nmas cosas\r\n"));
        assert_eq!(programa(&colgado), Some(&b"personal diario Cyberpunk2077.exe"[..]));
        assert_eq!(programa(b"nada"), None);
    }

    #[test]
    fn sin_fecha_ni_nombre_lo_dice() {
        let r: Registro<4> = Registro::nuevo();
        let c = Cabecera { fecha: b"", programa: b"", segundos: 0, vuelta: 1 };
        let mut v = Vec::new();
        componer(&c, &r, &mut core::iter::empty(), false, &mut |b| v.extend_from_slice(b));
        let t = String::from_utf8(v).unwrap();
        assert!(t.contains("(sin nombre)") && t.contains("(la placa no la da)"), "{t}");
        assert!(t.contains("0 lineas, 0 distintas"), "{t}");
    }
}
