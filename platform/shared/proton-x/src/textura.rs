//! **LAS TEXTURAS** (camino de `HelloTexture`, 29-09) -- lo que lee un
//! sombreador con `Sample`: una imagen de 8 bits por canal y un
//! MUESTREADOR (como se filtra y que pasa fuera de 0..1), con las reglas de
//! D3D11/12 (la especificacion funcional de D3D11, seccion 7.18).
//!
//! capa: puro -- ni un aparato, ni `std`: soft-float en Ring 3
//!
//! ```text
//!    PUNTO     el texel de floor(u * ancho), floor(v * alto)
//!    LINEAL    los cuatro de alrededor de (u * ancho - 0.5, v * alto - 0.5),
//!              con la fraccion en 8 bits (lo que la especificacion pide y
//!              lo que hacen las tarjetas; la 3060 lo dira en el metal)
//!    fuera     REPETIR, ESPEJO, SUJETAR, BORDE (su color) y ESPEJO UNA VEZ
//! ```
//!
//! Sin mipmaps (un nivel, el 0), sin anisotropia, sin comparacion: se dicen
//! al leer el muestreador y el lote va igual, con lo de aqui.

/// `DXGI_FORMAT_R8G8B8A8_UNORM` y `B8G8R8A8_UNORM`.
pub const R8G8B8A8_UNORM: u32 = 28;
pub const B8G8R8A8_UNORM: u32 = 87;

/// **Una textura**: sus texeles como estan en memoria (4 bytes cada uno).
#[derive(Clone, Copy, Debug)]
pub struct Textura<'a> {
    pub texeles: &'a [u32],
    pub ancho: u32,
    pub alto: u32,
    /// `B8G8R8A8` (si no, `R8G8B8A8`).
    pub bgra: bool,
}

/// Como se filtra (`D3D12_FILTER`, reducido a lo que no son mipmaps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filtro {
    Punto,
    Lineal,
}

/// Que pasa fuera de 0..1 (`D3D12_TEXTURE_ADDRESS_MODE`: 1..5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direccion {
    Repetir,
    Espejo,
    Sujetar,
    Borde,
    EspejoUnaVez,
}

/// **Un muestreador.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muestreador {
    pub filtro: Filtro,
    pub u: Direccion,
    pub v: Direccion,
    /// El color del borde (R, G, B, A), para [`Direccion::Borde`].
    pub borde: [f32; 4],
}

impl Muestreador {
    /// **De un `D3D12_STATIC_SAMPLER_DESC`** (13 palabras): Filter,
    /// AddressU, V, W, MipLODBias, MaxAnisotropy, ComparisonFunc,
    /// BorderColor, MinLOD, MaxLOD, ShaderRegister, RegisterSpace,
    /// ShaderVisibility. `Err` = lo que no se sabe (se dice, no se inventa).
    pub fn de_estatico(p: &[u32; 13]) -> Result<Self, &'static str> {
        let filtro = filtro(p[0])?;
        let borde = match p[7] {
            0 => [0.0, 0.0, 0.0, 0.0],
            1 => [0.0, 0.0, 0.0, 1.0],
            2 => [1.0, 1.0, 1.0, 1.0],
            _ => return Err("un color de borde que no es de D3D12"),
        };
        Ok(Muestreador { filtro, u: direccion(p[1])?, v: direccion(p[2])?, borde })
    }

    /// **De un `D3D12_SAMPLER_DESC`** (CreateSampler: 13 palabras tambien --
    /// el borde, cuatro floats en 8..11, y MinLOD/MaxLOD en 12..13 van
    /// aparte: aqui llegan Filter, AddressU, V y los cuatro del borde).
    pub fn de_descriptor(filtro_d3d: u32, u: u32, v: u32, borde: [f32; 4]) -> Result<Self, &'static str> {
        Ok(Muestreador { filtro: filtro(filtro_d3d)?, u: direccion(u)?, v: direccion(v)?, borde })
    }
}

/// `D3D12_FILTER`: los bits de MIN, MAG y MIP (0x01 MIP, 0x04 MAG, 0x10
/// MIN) lineales o no; 0x80 comparacion; 0x40/0x55 anisotropico.
fn filtro(f: u32) -> Result<Filtro, &'static str> {
    if f & 0x80 != 0 {
        return Err("un muestreador de COMPARACION: todavia no");
    }
    if f & 0x40 != 0 {
        return Err("un muestreador ANISOTROPICO: todavia no");
    }
    // Sin mipmaps, el de MIP da igual; MIN y MAG tienen que coincidir.
    match (f & 0x10 != 0, f & 0x04 != 0) {
        (false, false) => Ok(Filtro::Punto),
        (true, true) => Ok(Filtro::Lineal),
        _ => Err("un filtro con MIN y MAG distintos: todavia no"),
    }
}

fn direccion(d: u32) -> Result<Direccion, &'static str> {
    Ok(match d {
        1 => Direccion::Repetir,
        2 => Direccion::Espejo,
        3 => Direccion::Sujetar,
        4 => Direccion::Borde,
        5 => Direccion::EspejoUnaVez,
        _ => return Err("un modo de direccion que no es de D3D12"),
    })
}

/// `floor` sin `libm` (soft-float en Ring 3): por la conversion a entero.
pub fn suelo(x: f32) -> f32 {
    if !(x.abs() < 8_388_608.0) {
        return x; // ya es entero (o NaN, o infinito)
    }
    let t = x as i32 as f32;
    if t > x {
        t - 1.0
    } else {
        t
    }
}

/// El texel `i` de `n` segun la direccion; `None` = el borde.
fn dentro(i: i64, n: i64, d: Direccion) -> Option<i64> {
    match d {
        Direccion::Repetir => Some(i.rem_euclid(n)),
        Direccion::Sujetar => Some(i.clamp(0, n - 1)),
        Direccion::Borde => (0..n).contains(&i).then_some(i),
        Direccion::Espejo => {
            let m = i.rem_euclid(2 * n);
            Some(if m < n { m } else { 2 * n - 1 - m })
        }
        Direccion::EspejoUnaVez => {
            let a = if i < 0 { -i - 1 } else { i };
            Some(a.min(n - 1))
        }
    }
}

/// Un canal de 8 bits a float (`UNORM`: `b / 255`).
fn canal(b: u32) -> f32 {
    (b & 0xFF) as f32 / 255.0
}

impl Textura<'_> {
    /// El texel `(x, y)` ya dentro, como (R, G, B, A).
    fn texel(&self, x: i64, y: i64) -> [f32; 4] {
        let p = self.texeles.get((y * self.ancho as i64 + x) as usize).copied().unwrap_or(0);
        let (r, g, b, a) = if self.bgra { (p >> 16, p >> 8, p, p >> 24) } else { (p, p >> 8, p >> 16, p >> 24) };
        [canal(r), canal(g), canal(b), canal(a)]
    }

    /// El texel `(i, j)` con las direcciones del muestreador (o su borde).
    fn leer(&self, m: &Muestreador, i: i64, j: i64) -> [f32; 4] {
        match (dentro(i, self.ancho as i64, m.u), dentro(j, self.alto as i64, m.v)) {
            (Some(x), Some(y)) => self.texel(x, y),
            _ => m.borde,
        }
    }

    /// **`Sample(s, (u, v))`** en el nivel 0.
    pub fn muestrear(&self, m: &Muestreador, u: f32, v: f32) -> [f32; 4] {
        if self.ancho == 0 || self.alto == 0 || u.is_nan() || v.is_nan() {
            return [0.0; 4];
        }
        let (su, sv) = (u * self.ancho as f32, v * self.alto as f32);
        match m.filtro {
            Filtro::Punto => self.leer(m, suelo(su) as i64, suelo(sv) as i64),
            Filtro::Lineal => {
                let (tu, tv) = (su - 0.5, sv - 0.5);
                let (i0, j0) = (suelo(tu), suelo(tv));
                // La fraccion, en 8 bits (1/256).
                let fu = suelo((tu - i0) * 256.0) / 256.0;
                let fv = suelo((tv - j0) * 256.0) / 256.0;
                let (i0, j0) = (i0 as i64, j0 as i64);
                let (a, b, c, d) = (self.leer(m, i0, j0), self.leer(m, i0 + 1, j0), self.leer(m, i0, j0 + 1), self.leer(m, i0 + 1, j0 + 1));
                core::array::from_fn(|k| {
                    let arriba = a[k] + (b[k] - a[k]) * fu;
                    let abajo = c[k] + (d[k] - c[k]) * fu;
                    arriba + (abajo - arriba) * fv
                })
            }
        }
    }
}

/// **Lo que un dibujo le da a sus sombreadores**: las texturas por su
/// registro (tN) y los muestreadores por el suyo (sN).
#[derive(Clone, Copy, Debug)]
pub struct Recursos<'a> {
    pub texturas: &'a [Option<Textura<'a>>],
    pub muestreadores: &'a [Option<Muestreador>],
}

impl Recursos<'_> {
    pub const NINGUNO: Recursos<'static> = Recursos { texturas: &[], muestreadores: &[] };

    /// `Sample(tN, sM, (u, v))`; sin textura o sin muestreador, ceros.
    pub fn muestrear(&self, t: u8, s: u8, u: f32, v: f32) -> [f32; 4] {
        match (self.texturas.get(t as usize), self.muestreadores.get(s as usize)) {
            (Some(Some(tx)), Some(Some(m))) => tx.muestrear(m, u, v),
            _ => [0.0; 4],
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const PUNTO_BORDE: Muestreador = Muestreador { filtro: Filtro::Punto, u: Direccion::Borde, v: Direccion::Borde, borde: [0.0, 0.0, 0.0, 0.0] };

    #[test]
    fn suelo_como_floor() {
        for x in [-2.5f32, -2.0, -0.5, -0.0, 0.0, 0.25, 0.999, 1.0, 7.5, 1e9, -1e9] {
            assert_eq!(suelo(x), x.floor(), "{x}");
        }
    }

    #[test]
    fn punto_el_texel_de_floor_y_el_borde_fuera() {
        // 2x2 RGBA: rojo, verde / azul, blanco.
        let t = [0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000, 0xFFFF_FFFF];
        let tx = Textura { texeles: &t, ancho: 2, alto: 2, bgra: false };
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.25, 0.25), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.75, 0.25), [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.25, 0.75), [0.0, 0.0, 1.0, 1.0]);
        // El centro exacto (0.5) es del texel 1.
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 0.5, 0.5), [1.0, 1.0, 1.0, 1.0]);
        // Fuera: el borde (negro transparente).
        assert_eq!(tx.muestrear(&PUNTO_BORDE, 1.25, 0.25), [0.0; 4]);
        assert_eq!(tx.muestrear(&PUNTO_BORDE, -0.01, 0.25), [0.0; 4]);
        // Repetir y sujetar.
        let rep = Muestreador { u: Direccion::Repetir, v: Direccion::Repetir, ..PUNTO_BORDE };
        assert_eq!(tx.muestrear(&rep, 1.25, 0.25), [1.0, 0.0, 0.0, 1.0]);
        let suj = Muestreador { u: Direccion::Sujetar, v: Direccion::Sujetar, ..PUNTO_BORDE };
        assert_eq!(tx.muestrear(&suj, 7.0, -3.0), [0.0, 1.0, 0.0, 1.0]);
        // BGRA: el mismo pixel en memoria dice otro color.
        let bg = Textura { bgra: true, ..tx };
        assert_eq!(bg.muestrear(&PUNTO_BORDE, 0.25, 0.25), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn lineal_mezcla_los_cuatro() {
        let t = [0xFF00_0000, 0xFFFF_FFFF, 0xFF00_0000, 0xFFFF_FFFF];
        let tx = Textura { texeles: &t, ancho: 2, alto: 2, bgra: false };
        let m = Muestreador { filtro: Filtro::Lineal, u: Direccion::Sujetar, v: Direccion::Sujetar, borde: [0.0; 4] };
        // Justo entre los dos centros: la mitad.
        assert_eq!(tx.muestrear(&m, 0.5, 0.5)[0], 0.5);
        // En el centro de un texel: el texel.
        assert_eq!(tx.muestrear(&m, 0.25, 0.25)[0], 0.0);
        assert_eq!(tx.muestrear(&m, 0.75, 0.75)[0], 1.0);
    }

    #[test]
    fn el_muestreador_de_hellotexture() {
        // D3D12_FILTER_MIN_MAG_MIP_POINT, BORDER x3, TRANSPARENT_BLACK.
        let m = Muestreador::de_estatico(&[0, 4, 4, 4, 0, 0, 1, 0, 0, 0x7F7F_FFFF, 0, 0, 5]).unwrap();
        assert_eq!(m, PUNTO_BORDE);
        assert_eq!(Muestreador::de_estatico(&[0x15, 3, 3, 3, 0, 0, 1, 2, 0, 0, 0, 0, 0]).unwrap().filtro, Filtro::Lineal);
        assert!(Muestreador::de_estatico(&[0x55, 1, 1, 1, 0, 16, 1, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(Muestreador::de_estatico(&[0x80, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
