//! Contraste de la WPR meta con la copia y traducciones aportadas por el kernel.
//! capa: puro -- no recorre tablas ni dereferencia direcciones.
//! [carril] AMARILLO
//! [estado]  VOLATIL  clasificacion local; no acredita transacciones DMA ni autenticacion.
pub const PALABRAS: usize = super::WPR_META / 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Estado { Coincide = 0, SinTraduccion = 1, PaginaDistinta = 2,
    SinEscritura = 3, SinCopia = 4, ContenidoDistinto = 5, BaseInvalida = 6 }

/// Las traducciones incluyen permiso de lectura por contrato del oraculo.
/// La meta debe ser una pagina propia alineada, legible y escribible.
pub fn enlace(base: u64, inicio: Option<(u64, bool)>, fin: Option<(u64, bool)>) -> Estado {
    if base == 0 || base & 4095 != 0 || base.checked_add(255).is_none() {
        return Estado::BaseInvalida;
    }
    let (Some((a, wa)), Some((b, wb))) = (inicio, fin) else { return Estado::SinTraduccion };
    if a != base || b != base + 255 { return Estado::PaginaDistinta; }
    if !wa || !wb { return Estado::SinEscritura; }
    Estado::Coincide
}

/// Difiere en cualquier bit de los 256 bytes, incluidos bootCount y verified.
/// La comparacion previa exige la copia exacta; despues solo se informa.
pub fn diferencias(esperada: &[u64; PALABRAS], observada: &[u64; PALABRAS]) -> u32 {
    esperada.iter().zip(observada).enumerate().fold(0, |m, (i, (a, b))| m | if a != b { 1 << i } else { 0 })
}

pub fn contenido(esperada: &[u64; PALABRAS], observada: Option<&[u64; PALABRAS]>) -> Estado {
    match observada {
        None => Estado::SinCopia,
        Some(v) if diferencias(esperada, v) != 0 => Estado::ContenidoDistinto,
        Some(_) => Estado::Coincide,
    }
}

/// No invoca el lector si la traduccion o permisos no coinciden. El kernel
/// aporta una base propia; nunca se lee la fisica desconocida de una tabla.
pub fn auditar(base: u64, inicio: Option<(u64, bool)>, fin: Option<(u64, bool)>,
    esperada: &[u64; PALABRAS], mut leer: impl FnMut(u64) -> u64,
) -> (Estado, Option<[u64; PALABRAS]>) {
    let estado = enlace(base, inicio, fin);
    if estado != Estado::Coincide { return (estado, None); }
    let mut actual = [0; PALABRAS];
    for (i, v) in actual.iter_mut().enumerate() { *v = leer(base + i as u64 * 8); }
    (contenido(esperada, Some(&actual)), Some(actual))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lector_solo_recibe_la_region_propia_y_no_se_llama_con_enlace_invalido() {
        let esperada = core::array::from_fn(|i| 0x8000000000000000 | i as u64);
        let (s, foto) = auditar(4096, Some((8192, true)), Some((8447, true)), &esperada,
            |_| panic!("no se lee una fisica desconocida"));
        assert_eq!((s, foto), (Estado::PaginaDistinta, None));
        let mut n = 0;
        let (s, foto) = auditar(4096, Some((4096, true)), Some((4351, true)), &esperada, |p| {
            assert_eq!(p, 4096 + n * 8);
            n += 1;
            esperada[(n - 1) as usize]
        });
        assert_eq!(n, 32);
        assert_eq!((s, foto), (Estado::Coincide, Some(esperada)));
    }
    #[test]
    fn traduccion_incompleta_erronea_o_sin_permiso_no_pasa() {
        let p = 0x12345000;
        assert_eq!(enlace(p, Some((p, true)), Some((p + 255, true))), Estado::Coincide);
        assert_eq!(enlace(p, None, Some((p + 255, true))), Estado::SinTraduccion);
        assert_eq!(enlace(p, Some((p, true)), None), Estado::SinTraduccion);
        assert_eq!(enlace(p, Some((p + 4096, true)), Some((p + 4351, true))), Estado::PaginaDistinta);
        assert_eq!(enlace(p, Some((p, true)), Some((p + 256, true))), Estado::PaginaDistinta);
        for (a, b) in [(false, true), (true, false), (false, false)] {
            assert_eq!(enlace(p, Some((p, a)), Some((p + 255, b))), Estado::SinEscritura);
        }
        for p in [0, 1, u64::MAX] { assert_eq!(enlace(p, None, None), Estado::BaseInvalida); }
    }
    #[test]
    fn todos_los_bits_y_la_ultima_palabra_cuentan_y_sin_copia_no_es_igual() {
        let esperada = [0u64; PALABRAS];
        assert_eq!(contenido(&esperada, None), Estado::SinCopia);
        assert_eq!(contenido(&esperada, Some(&esperada)), Estado::Coincide);
        for i in 0..PALABRAS {
            for v in [1, 1 << 63] {
                let mut actual = esperada;
                actual[i] = v;
                assert_eq!(diferencias(&esperada, &actual), 1 << i);
                assert_eq!(contenido(&esperada, Some(&actual)), Estado::ContenidoDistinto);
            }
        }
        assert_eq!(diferencias(&esperada, &[1; PALABRAS]), u32::MAX);
    }
}
