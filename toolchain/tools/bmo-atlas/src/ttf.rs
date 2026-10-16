//! **Leer un TrueType**: las tablas que hacen falta para DIBUJAR una letra
//! (`head`, `maxp`, `hhea`, `hmtx`, `cmap`, `loca`, `glyf`), y nada mas.
//!
//! ```text
//!    cmap     de un caracter (Unicode) a su glifo: formatos 4 y 12
//!    hmtx     cuanto avanza cada glifo
//!    glyf     sus CONTORNOS: puntos sobre la curva y puntos de control
//!             (curvas cuadraticas); o un glifo COMPUESTO de otros, movidos
//!             y escalados -- la "a" con tilde es la "a" y la tilde --
//! ```
//!
//! Todo lo que se lee se mira antes: un fichero roto da un `Err` con su
//! motivo, nunca un indice fuera de sitio. Lo que NO se lee: el hinting (las
//! instrucciones de `fpgm` y `prep` que ajustan a la rejilla), el kerning y
//! las ligaduras (OpenType, LE5), y las curvas cubicas de CFF.

/// Un punto del contorno en unidades de la letra, y si esta SOBRE la curva
/// (si no, es de control).
pub type Punto = (f64, f64, bool);

pub struct Ttf<'a> {
    d: &'a [u8],
    pub unidades: u16,
    pub ascenso: i16,
    pub descenso: i16,
    pub hueco: i16,
    largo_loca: bool,
    glifos: u16,
    metricas: u16,
    hmtx: usize,
    loca: usize,
    glyf: (usize, usize),
    cmap: (usize, u16),
}

fn u16_en(d: &[u8], o: usize) -> Result<u16, String> {
    d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]])).ok_or_else(|| format!("el fichero se acaba en el byte {o}"))
}

fn i16_en(d: &[u8], o: usize) -> Result<i16, String> {
    u16_en(d, o).map(|v| v as i16)
}

fn u32_en(d: &[u8], o: usize) -> Result<u32, String> {
    d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("el fichero se acaba en el byte {o}"))
}

impl<'a> Ttf<'a> {
    pub fn leer(d: &'a [u8]) -> Result<Ttf<'a>, String> {
        let version = u32_en(d, 0)?;
        if version == 0x4F54_544F {
            return Err("es OpenType con curvas CFF (OTTO): aun no se leen (LE5)".into());
        }
        if version != 0x0001_0000 && version != 0x7472_7565 {
            return Err("no es un TrueType: no empieza por 00010000 ni 'true'".into());
        }
        let n = u16_en(d, 4)? as usize;
        let tabla = |nombre: &[u8; 4]| -> Result<(usize, usize), String> {
            for i in 0..n {
                let o = 12 + 16 * i;
                if d.get(o..o + 4) == Some(&nombre[..]) {
                    let (ini, largo) = (u32_en(d, o + 8)? as usize, u32_en(d, o + 12)? as usize);
                    if ini.checked_add(largo).is_none_or(|f| f > d.len()) {
                        return Err(format!("la tabla {} se sale del fichero", String::from_utf8_lossy(nombre)));
                    }
                    return Ok((ini, largo));
                }
            }
            Err(format!("falta la tabla {}", String::from_utf8_lossy(nombre)))
        };
        let head = tabla(b"head")?.0;
        let maxp = tabla(b"maxp")?.0;
        let hhea = tabla(b"hhea")?.0;
        let hmtx = tabla(b"hmtx")?.0;
        let loca = tabla(b"loca")?.0;
        let glyf = tabla(b"glyf")?;
        let cmap = tabla(b"cmap")?.0;
        let unidades = u16_en(d, head + 18)?;
        if unidades == 0 {
            return Err("unitsPerEm es 0".into());
        }
        // La subtabla del cmap: Unicode entero (12) si la hay; si no, la de
        // Unicode basico (4).
        let cuantas = u16_en(d, cmap + 2)? as usize;
        let mut elegida = None;
        for i in 0..cuantas {
            let o = cmap + 4 + 8 * i;
            let (plataforma, cod) = (u16_en(d, o)?, u16_en(d, o + 2)?);
            let sub = cmap + u32_en(d, o + 4)? as usize;
            let formato = u16_en(d, sub)?;
            let unicode = plataforma == 0 || (plataforma == 3 && (cod == 1 || cod == 10));
            if unicode && (formato == 12 || (formato == 4 && elegida.is_none())) {
                elegida = Some((sub, formato));
            }
        }
        let cmap = elegida.ok_or("el cmap no tiene una subtabla Unicode de formato 4 o 12")?;
        Ok(Ttf {
            d,
            unidades,
            ascenso: i16_en(d, hhea + 4)?,
            descenso: i16_en(d, hhea + 6)?,
            hueco: i16_en(d, hhea + 8)?,
            largo_loca: i16_en(d, head + 50)? == 1,
            glifos: u16_en(d, maxp + 4)?,
            metricas: u16_en(d, hhea + 34)?,
            hmtx,
            loca,
            glyf: (glyf.0, glyf.1),
            cmap,
        })
    }

    /// El glifo del caracter `c` (0 si la letra no lo tiene: el `.notdef`).
    pub fn glifo(&self, c: u32) -> Result<u16, String> {
        let d = self.d;
        let (s, formato) = self.cmap;
        if formato == 12 {
            let n = u32_en(d, s + 12)? as usize;
            for i in 0..n {
                let o = s + 16 + 12 * i;
                let (a, b, g) = (u32_en(d, o)?, u32_en(d, o + 4)?, u32_en(d, o + 8)?);
                if (a..=b).contains(&c) {
                    return Ok((g + (c - a)) as u16);
                }
            }
            return Ok(0);
        }
        if c > 0xFFFF {
            return Ok(0);
        }
        let segs = u16_en(d, s + 6)? as usize / 2;
        let fin = s + 14;
        let ini = fin + 2 * segs + 2;
        let delta = ini + 2 * segs;
        let rango = delta + 2 * segs;
        for i in 0..segs {
            if c as u16 <= u16_en(d, fin + 2 * i)? {
                let a = u16_en(d, ini + 2 * i)?;
                if (c as u16) < a {
                    return Ok(0);
                }
                let dl = u16_en(d, delta + 2 * i)?;
                let r = u16_en(d, rango + 2 * i)? as usize;
                if r == 0 {
                    return Ok((c as u16).wrapping_add(dl));
                }
                let o = rango + 2 * i + r + 2 * (c as usize - a as usize);
                let g = u16_en(d, o)?;
                return Ok(if g == 0 { 0 } else { g.wrapping_add(dl) });
            }
        }
        Ok(0)
    }

    /// Cuanto avanza el glifo `g`, en unidades.
    pub fn avance(&self, g: u16) -> Result<u16, String> {
        let k = g.min(self.metricas.saturating_sub(1)) as usize;
        u16_en(self.d, self.hmtx + 4 * k)
    }

    fn sitio(&self, g: u16) -> Result<(usize, usize), String> {
        if g >= self.glifos {
            return Err(format!("el glifo {g} no existe: la letra tiene {}", self.glifos));
        }
        let (a, b) = if self.largo_loca {
            (u32_en(self.d, self.loca + 4 * g as usize)? as usize, u32_en(self.d, self.loca + 4 * g as usize + 4)? as usize)
        } else {
            (2 * u16_en(self.d, self.loca + 2 * g as usize)? as usize, 2 * u16_en(self.d, self.loca + 2 * g as usize + 2)? as usize)
        };
        if a > b || b > self.glyf.1 {
            return Err(format!("el glifo {g} se sale de glyf"));
        }
        Ok((self.glyf.0 + a, b - a))
    }

    /// **Los contornos del glifo `g`**, en unidades (y hacia ARRIBA, como
    /// en la letra). Un glifo compuesto trae los de sus piezas, movidas.
    pub fn contornos(&self, g: u16) -> Result<Vec<Vec<Punto>>, String> {
        let mut v = Vec::new();
        self.contornos_en(g, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0, &mut v)?;
        Ok(v)
    }

    fn contornos_en(&self, g: u16, m: [f64; 6], hondo: u32, out: &mut Vec<Vec<Punto>>) -> Result<(), String> {
        if hondo > 8 {
            return Err("un glifo compuesto de mas de ocho niveles: un ciclo".into());
        }
        let (o, largo) = self.sitio(g)?;
        if largo == 0 {
            return Ok(());
        }
        let d = self.d;
        let n = i16_en(d, o)?;
        let pon = |x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        if n >= 0 {
            let n = n as usize;
            let mut fines = Vec::with_capacity(n);
            for i in 0..n {
                fines.push(u16_en(d, o + 10 + 2 * i)? as usize);
            }
            let puntos = fines.last().map_or(0, |f| f + 1);
            let instr = u16_en(d, o + 10 + 2 * n)? as usize;
            let mut p = o + 12 + 2 * n + instr;
            let mut banderas = Vec::with_capacity(puntos);
            while banderas.len() < puntos {
                let b = *d.get(p).ok_or("las banderas se salen del glifo")?;
                p += 1;
                banderas.push(b);
                if b & 8 != 0 {
                    let r = *d.get(p).ok_or("las banderas se salen del glifo")?;
                    p += 1;
                    for _ in 0..r {
                        banderas.push(b);
                    }
                }
            }
            banderas.truncate(puntos);
            let mut eje = |corto: u8, igual: u8| -> Result<Vec<i32>, String> {
                let mut v = Vec::with_capacity(puntos);
                let mut a = 0i32;
                for &b in &banderas {
                    if b & corto != 0 {
                        let x = *d.get(p).ok_or("las coordenadas se salen del glifo")? as i32;
                        p += 1;
                        a += if b & igual != 0 { x } else { -x };
                    } else if b & igual == 0 {
                        a += i16_en(d, p)? as i32;
                        p += 2;
                    }
                    v.push(a);
                }
                Ok(v)
            };
            let xs = eje(2, 16)?;
            let ys = eje(4, 32)?;
            let mut desde = 0;
            for f in fines {
                if f < desde || f >= puntos {
                    return Err("un contorno acaba antes de empezar".into());
                }
                out.push(
                    (desde..=f)
                        .map(|i| {
                            let (x, y) = pon(xs[i] as f64, ys[i] as f64);
                            (x, y, banderas[i] & 1 != 0)
                        })
                        .collect(),
                );
                desde = f + 1;
            }
            return Ok(());
        }
        // Compuesto: pieza a pieza, cada una con su movimiento.
        let mut p = o + 10;
        loop {
            let banderas = u16_en(d, p)?;
            let pieza = u16_en(d, p + 2)?;
            p += 4;
            let (dx, dy) = if banderas & 1 != 0 {
                let r = (i16_en(d, p)? as f64, i16_en(d, p + 2)? as f64);
                p += 4;
                r
            } else {
                let r = (*d.get(p).ok_or("la pieza se sale")? as i8 as f64, *d.get(p + 1).ok_or("la pieza se sale")? as i8 as f64);
                p += 2;
                r
            };
            if banderas & 2 == 0 {
                return Err("una pieza que se pega por PUNTOS (no por x, y): aun no se lee".into());
            }
            let f2 = |o: usize| i16_en(d, o).map(|v| v as f64 / 16384.0);
            let mut e = [1.0, 0.0, 0.0, 1.0];
            if banderas & 8 != 0 {
                let s = f2(p)?;
                e = [s, 0.0, 0.0, s];
                p += 2;
            } else if banderas & 0x40 != 0 {
                e = [f2(p)?, 0.0, 0.0, f2(p + 2)?];
                p += 4;
            } else if banderas & 0x80 != 0 {
                e = [f2(p)?, f2(p + 2)?, f2(p + 4)?, f2(p + 6)?];
                p += 8;
            }
            let pm = [
                m[0] * e[0] + m[2] * e[1],
                m[1] * e[0] + m[3] * e[1],
                m[0] * e[2] + m[2] * e[3],
                m[1] * e[2] + m[3] * e[3],
                m[0] * dx + m[2] * dy + m[4],
                m[1] * dx + m[3] * dy + m[5],
            ];
            self.contornos_en(pieza, pm, hondo + 1, out)?;
            if banderas & 0x20 == 0 {
                return Ok(());
            }
        }
    }
}
