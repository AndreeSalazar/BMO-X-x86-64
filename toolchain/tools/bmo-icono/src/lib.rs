//! **bmo-icono** -- la CARA de una app desde un PNG, un JPG o un SVG.
//!
//! El propietario (15-10): *"que el icono en escritorio se vea elegante [...]
//! que tengan iconos con imagen png o jpg o svg"*. Hasta hoy la cara de una
//! app era una rejilla de 16 x 16 escrita a mano con letras de una paleta
//! (`catalogo.ps1`) y pintada al doble: cuadros de 2 x 2 y bordes en
//! escalera.
//!
//! Esto la saca de un fichero de verdad y la deja en un `BICO` de
//! [`LADO`] x [`LADO`] -- la medida a la que el escritorio la PINTA, uno a
//! uno, sin agrandar -- con el ALFA ENTERO (256 tonos de borde, no un bit):
//!
//! ```text
//!    SVG        se pinta a la medida justa con su forma (`svg`, `ruta`,
//!               `raster`): curvas, arcos, trazos y degradados, suavizado
//!               a 64 muestras por pixel
//!    PNG, JPG   los decodificadores de la casa (`bmo-imagen`, el mismo
//!               del visor) y se ENCAJA por promedio de area
//! ```
//!
//! Vive en el ANFITRION: la cara se hace al construir, una vez, y dentro del
//! `.bex` viaja ya hecha. El escritorio no decodifica nada al arrancar.

pub mod raster;
pub mod ruta;
pub mod svg;

/// El lado del icono que se guarda: el mismo al que lo pinta el escritorio
/// (`launcher.rs`, `ICON_PX`).
pub const LADO: u32 = 32;

/// **De un fichero (por su contenido, no su nombre) a un BICO de `lado`.**
/// Devuelve tambien lo que el SVG traia y no se supo pintar.
pub fn icono(bytes: &[u8], lado: u32) -> Result<(Vec<u8>, Vec<String>), String> {
    if lado == 0 || lado > 256 {
        return Err(format!("un icono de {lado} de lado no tiene sentido (1 a 256)"));
    }
    if es_svg(bytes) {
        let texto = std::str::from_utf8(bytes).map_err(|_| "el SVG no es UTF-8".to_string())?;
        let d = svg::leer(texto)?;
        if d.formas.is_empty() {
            return Err("el SVG no tiene ninguna forma que se sepa pintar".into());
        }
        return Ok((raster::pintar(&d, lado).bico(), d.saltado));
    }
    let m = bmo_imagen::medir(bytes).map_err(|e| format!("no es una imagen que se sepa leer: {e:?}"))?;
    let mut px = vec![0u32; (m.ancho * m.alto) as usize];
    match m.formato {
        bmo_imagen::Formato::Png => {
            let mut taller = vec![0u8; bmo_imagen::TALLER];
            bmo_imagen::png::decodificar_alfa(bytes, &m, &mut px, &mut taller, true)
        }
        _ => bmo_imagen::decodificar(bytes, &mut px).map(|_| ()),
    }
    .map_err(|e| format!("la imagen no se pudo decodificar: {e:?}"))?;
    Ok((raster::encajar(&px, m.ancho, m.alto, lado).bico(), Vec::new()))
}

/// Un SVG empieza (tras espacios y la cabecera XML o un comentario) por `<`
/// y dice `<svg` en sus primeros KiB.
pub fn es_svg(bytes: &[u8]) -> bool {
    let inicio = &bytes[..bytes.len().min(4096)];
    let t = String::from_utf8_lossy(inicio);
    t.trim_start().starts_with('<') && t.contains("<svg")
}
