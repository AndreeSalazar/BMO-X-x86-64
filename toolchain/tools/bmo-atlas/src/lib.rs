//! **bmo-atlas** -- EL ATLAS DE UNA LETRA (LE1 de PL18, `docs/plan/PLAN_ECCE.md`).
//!
//! El propietario (16-10): *"ECCE puede tener letras elegantes? [...] que
//! todo el INTERFAZ se vea mucho mas elegante"*. Las apps de Rust ya escriben
//! con LA LETRA DE LA CASA (`platform/shared/bmo-letra`: proporcional, de
//! borde suave, trazos propios). Las de TITAN++ no: `director.texto` pinta la
//! de 8 x 16 del kernel, sin grises. Esto es el puente: un ATLAS -- cada
//! letra de Latin-1 (de la 32 a la 255: el castellano entero) ya dibujada,
//! con su alfa -- que una app lee y pinta sin saber nada de curvas.
//!
//! De donde sale la letra:
//!
//! ```text
//!    la casa      `bmo-letra` (por defecto): sin un contorno de nadie, la
//!                 regla del propietario ("no integrar objetos terceros,
//!                 sino tomar la inspiracion", 03-10)
//!    un .ttf      un TrueType que el propietario ELIJA (`ttf`, `pinta`): se
//!                 lee y se dibuja aqui, curvas cuadraticas y glifos
//!                 compuestos. El fichero no entra al arbol: es suyo
//! ```
//!
//! ```text
//!    BLET  el atlas (todo en little-endian)
//!       0  "BLET"
//!       4  version (1), medida en pixeles del em (u8), 0, 0
//!       8  ascenso (i16, px), descenso (i16, px, negativo), linea (u16, px)
//!      14  primero (u16, 32), cuantos (u16, 224)
//!      18  por glifo, 16 bytes:
//!             avance   u16   en 1/64 de pixel
//!             x        i16   de la pluma al borde izquierdo del dibujo
//!             y        i16   de la linea base a su borde de ARRIBA (+ sube)
//!             ancho    u16
//!             alto     u16
//!             (cero)   u16
//!             donde    u32   su primer byte en el alfa, desde el final de
//!                            la tabla
//!       .  el alfa: ancho x alto bytes por glifo, fila a fila (0 a 255)
//! ```
//!
//! Vive en el ANFITRION, como `bmo-icono`: la letra se dibuja al construir,
//! una vez por medida, y la app lleva el atlas. Lo que no hace aun con un
//! `.ttf`: el hinting (a medidas chicas se ve mas suave que en Windows), el
//! kerning y las ligaduras (LE5).

pub mod pinta;
pub mod ttf;

pub const PRIMERO: u16 = 32;
pub const CUANTOS: u16 = 224;

/// Un glifo dibujado.
#[derive(Debug)]
pub struct Glifo {
    /// En 1/64 de pixel.
    pub avance: u16,
    pub x: i16,
    pub y: i16,
    pub ancho: u16,
    pub alto: u16,
    pub alfa: Vec<u8>,
}

#[derive(Debug)]
pub struct Atlas {
    pub medida: u8,
    pub ascenso: i16,
    pub descenso: i16,
    pub linea: u16,
    pub glifos: Vec<Glifo>,
}

/// **El glifo del caracter `c`** de la letra a `medida` pixeles por em.
pub fn glifo(t: &ttf::Ttf, c: u32, medida: f64) -> Result<Glifo, String> {
    let mut g = t.glifo(c)?;
    if g == 0 && c != 32 {
        // La letra no lo tiene: el "?" (no un cuadro vacio sin nombre).
        g = t.glifo('?' as u32)?;
    }
    let s = medida / t.unidades as f64;
    let avance = (t.avance(g)? as f64 * s * 64.0).round() as u16;
    let tramos = pinta::aplanar(&t.contornos(g)?, |x, y| (x * s, -y * s));
    if tramos.is_empty() {
        return Ok(Glifo { avance, x: 0, y: 0, ancho: 0, alto: 0, alfa: Vec::new() });
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(a, b) in &tramos {
        for p in [a, b] {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
    }
    let (x0, y0) = (x0.floor(), y0.floor());
    let (ancho, alto) = ((x1.ceil() - x0) as usize, (y1.ceil() - y0) as usize);
    let alfa = pinta::cubrir(&tramos, x0, y0, ancho, alto);
    Ok(Glifo { avance, x: x0 as i16, y: -y0 as i16, ancho: ancho as u16, alto: alto as u16, alfa })
}

/// **El atlas de Latin-1 de la LETRA DE LA CASA** a `medida` pixeles (la
/// `font-size`), con su `peso`. Las medidas de la linea, las de
/// `bmo-letra`: 1,025 em sobre la base y 0,275 debajo.
pub fn atlas_de_casa(medida: u8, peso: bmo_letra::Peso) -> Result<Atlas, String> {
    use bmo_letra::Fuente;
    if !(6..=128).contains(&medida) {
        return Err(format!("la medida {medida} no va de 6 a 128 pixeles"));
    }
    let mut letra = bmo_letra::Letra::nueva();
    let mut glifos = Vec::with_capacity(CUANTOS as usize);
    for c in PRIMERO..PRIMERO + CUANTOS {
        let c = if (127..160).contains(&c) { 32 } else { c as u8 };
        let c = if letra.glifo(c, medida, peso, 0).is_some() { c } else { b'?' };
        glifos.push(match letra.glifo(c, medida, peso, 0) {
            Some(g) => Glifo {
                avance: g.avance64.clamp(0, u16::MAX as i32) as u16,
                x: g.x as i16,
                y: -g.y as i16,
                ancho: g.w as u16,
                alto: g.h as u16,
                alfa: g.alfa.to_vec(),
            },
            None => Glifo { avance: 0, x: 0, y: 0, ancho: 0, alto: 0, alfa: Vec::new() },
        });
    }
    let px = medida as i32;
    Ok(Atlas {
        medida,
        ascenso: ((1025 * px + 500) / 1000) as i16,
        descenso: -((275 * px + 500) / 1000) as i16,
        linea: bmo_letra::alto_normal(medida) as u16,
        glifos,
    })
}

/// **El atlas de Latin-1** de la letra TrueType `fuente` a `medida` pixeles
/// por em.
pub fn atlas(fuente: &[u8], medida: u8) -> Result<Atlas, String> {
    if !(6..=128).contains(&medida) {
        return Err(format!("la medida {medida} no va de 6 a 128 pixeles"));
    }
    let t = ttf::Ttf::leer(fuente)?;
    let s = medida as f64 / t.unidades as f64;
    let ascenso = (t.ascenso as f64 * s).round() as i16;
    let descenso = (t.descenso as f64 * s).round() as i16;
    let linea = ((t.ascenso as f64 - t.descenso as f64 + t.hueco as f64) * s).round() as u16;
    let mut glifos = Vec::with_capacity(CUANTOS as usize);
    for c in PRIMERO as u32..PRIMERO as u32 + CUANTOS as u32 {
        // 127..=159 no son letras en Latin-1: van vacias, con su avance.
        if (127..160).contains(&c) {
            let g = glifo(&t, 32, medida as f64)?;
            glifos.push(g);
        } else {
            glifos.push(glifo(&t, c, medida as f64)?);
        }
    }
    Ok(Atlas { medida, ascenso, descenso, linea, glifos })
}

impl Atlas {
    /// El fichero BLET.
    pub fn bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"BLET");
        v.extend_from_slice(&[1, self.medida, 0, 0]);
        v.extend_from_slice(&self.ascenso.to_le_bytes());
        v.extend_from_slice(&self.descenso.to_le_bytes());
        v.extend_from_slice(&self.linea.to_le_bytes());
        v.extend_from_slice(&PRIMERO.to_le_bytes());
        v.extend_from_slice(&CUANTOS.to_le_bytes());
        let mut donde = 0u32;
        for g in &self.glifos {
            v.extend_from_slice(&g.avance.to_le_bytes());
            v.extend_from_slice(&g.x.to_le_bytes());
            v.extend_from_slice(&g.y.to_le_bytes());
            v.extend_from_slice(&g.ancho.to_le_bytes());
            v.extend_from_slice(&g.alto.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&donde.to_le_bytes());
            donde += g.alfa.len() as u32;
        }
        for g in &self.glifos {
            v.extend_from_slice(&g.alfa);
        }
        v
    }

    fn de(&self, c: u8) -> &Glifo {
        let i = (c as u16).saturating_sub(PRIMERO).min(CUANTOS - 1);
        &self.glifos[i as usize]
    }

    /// **Una linea de texto (Latin-1) en gris**, para VERLA: `(ancho, alto,
    /// pixeles)`, tinta negra sobre blanco. La pluma avanza en 1/64.
    pub fn muestra(&self, texto: &[u8]) -> (usize, usize, Vec<u8>) {
        let total: u32 = texto.iter().map(|&c| self.de(c).avance as u32).sum();
        let ancho = (total / 64) as usize + 2 * self.medida as usize;
        let alto = self.linea as usize + self.medida as usize / 2;
        let mut px = vec![255u8; ancho * alto];
        let base = self.ascenso as i64 + self.medida as i64 / 4;
        let mut pluma = (self.medida as u32) * 64;
        for &c in texto {
            let g = self.de(c);
            let gx = (pluma / 64) as i64 + g.x as i64;
            let gy = base - g.y as i64;
            for fy in 0..g.alto as i64 {
                for fx in 0..g.ancho as i64 {
                    let (x, y) = (gx + fx, gy + fy);
                    if x >= 0 && y >= 0 && (x as usize) < ancho && (y as usize) < alto {
                        let a = g.alfa[(fy * g.ancho as i64 + fx) as usize] as u32;
                        let p = &mut px[y as usize * ancho + x as usize];
                        *p = (*p as u32 * (255 - a) / 255) as u8;
                    }
                }
            }
            pluma += g.avance as u32;
        }
        (ancho, alto, px)
    }
}
