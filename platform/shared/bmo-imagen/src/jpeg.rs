//! **JPEG** (baseline, Huffman, 8 bits) -- el formato de TODAS las fotos.
//! Eddi (20-09): *"visualizar imagenes por completo"*.
//!
//! generacion: nieto -- recibe bytes y un bufer, y devuelve medidas o un
//! motivo; no sabe donde se pinta.
//!
//! ## Lo que se lee, y lo que se rechaza con nombre
//!
//! ```text
//!    SOF0 (baseline), 8 bits, 1 componente (gris) o 3 (YCbCr)
//!    muestreo del Y de 1x1, 2x1, 1x2 y 2x2 (4:4:4, 4:2:2, 4:2:0); Cb y Cr 1x1
//!    tablas DQT de 8 bits, DHT (hasta 4 DC y 4 AC), DRI y sus RSTn
//!    un solo SOS con todos los componentes
//!
//!    Variante:  progresivo (SOF2), 12 bits, aritmetico, DQT de 16 bits, CMYK,
//!               muestreo raro, varios SOS
//! ```
//!
//! Progresivo se deja fuera a proposito: pide guardar los coeficientes de la
//! imagen ENTERA (4 MiB de i16 para 1024 de lado) y varias pasadas; es un
//! `Variante` con su frase, no una foto a medias. Las camaras y los telefonos
//! escriben baseline.
//!
//! ## Sin coma flotante y sin taller
//!
//! La IDCT es entera (cosenos en punto fijo de 13 bits, acumulado en i32):
//! el mismo fuente da los mismos pixeles en cualquier maquina, que es la regla
//! de la casa, y no hay SSE que preservar. Todo lo que hace falta --seis
//! bloques de 64 coeficientes, cuatro tablas de Huffman, cuatro de
//! cuantizacion-- cabe en 6 KiB de pila. La salida se escribe MCU a MCU en
//! `dst`, con el recorte en el borde derecho e inferior: la imagen no tiene
//! por que medir un multiplo de 8 ni de 16.
//!
//! ## Lo que un fichero hostil no consigue
//!
//! Cada segmento se recorta a lo que llego; un codigo Huffman que no esta en
//! la tabla, un indice de tabla que no existe, un run que se sale del bloque
//! o un MCU que apunta fuera del bufer son errores con nombre, no lecturas de
//! mas. El flujo de entropia que se acaba antes del ultimo MCU es `Corto`.

use crate::{lados, Error, Formato, Medidas, OPACO};

/// El orden zig-zag: la posicion natural del coeficiente `k`-esimo.
const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14,
    21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53,
    60, 61, 54, 47, 55, 62, 63,
];

/// `round(cos((2x+1) u pi / 16) * c(u) * 2^13)`, con `c(0) = 1/sqrt(2)`.
/// Escrito como tabla y no calculado: sin flotantes en ninguna maquina.
const COS: [[i32; 8]; 8] = [
    [5793, 8035, 7568, 6811, 5793, 4551, 3135, 1598],
    [5793, 6811, 3135, -1598, -5793, -8035, -7568, -4551],
    [5793, 4551, -3135, -8035, -5793, 1598, 7568, 6811],
    [5793, 1598, -7568, -4551, 5793, 6811, -3135, -8035],
    [5793, -1598, -7568, 4551, 5793, -6811, -3135, 8035],
    [5793, -4551, -3135, 8035, -5793, -1598, 7568, -6811],
    [5793, -6811, 3135, 1598, -5793, 8035, -7568, 4551],
    [5793, -8035, 7568, -6811, 5793, -4551, 3135, -1598],
];

fn be16(b: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize)
}

/// Una tabla de Huffman de JPEG, en la forma canonica de la norma (Anexo C).
#[derive(Clone, Copy)]
struct Huffman {
    /// `mincode[l]`, `maxcode[l]` (-1 si no hay) y `valptr[l]` por longitud 1..16.
    mincode: [i32; 17],
    maxcode: [i32; 18],
    valptr: [i32; 17],
    valores: [u8; 256],
    hay: bool,
}

const HUFF_VACIA: Huffman = Huffman { mincode: [0; 17], maxcode: [-1; 18], valptr: [0; 17], valores: [0; 256], hay: false };

impl Huffman {
    fn de(cuentas: &[u8; 16], valores: &[u8]) -> Result<Self, Error> {
        let mut h = HUFF_VACIA;
        if valores.len() > 256 {
            return Err(Error::Variante);
        }
        h.valores[..valores.len()].copy_from_slice(valores);
        let mut codigo: i32 = 0;
        let mut k: i32 = 0;
        for l in 1..=16 {
            let n = cuentas[l - 1] as i32;
            if n == 0 {
                h.maxcode[l] = -1;
            } else {
                h.valptr[l] = k;
                h.mincode[l] = codigo;
                codigo += n;
                k += n;
                h.maxcode[l] = codigo - 1;
            }
            codigo <<= 1;
        }
        h.maxcode[17] = i32::MAX;
        h.hay = true;
        Ok(h)
    }
}

/// El flujo de entropia: bits con el `FF 00` quitado y los RSTn vistos.
struct Bits<'a> {
    b: &'a [u8],
    pos: usize,
    acum: u32,
    cuantos: u32,
    /// Se topo con un marcador: no se leen mas datos hasta `reinicia`.
    marcador: bool,
}

impl Bits<'_> {
    fn bit(&mut self) -> Result<u32, Error> {
        if self.cuantos == 0 {
            if self.marcador {
                return Err(Error::Corto);
            }
            let mut byte = *self.b.get(self.pos).ok_or(Error::Corto)?;
            self.pos += 1;
            if byte == 0xFF {
                let sig = *self.b.get(self.pos).ok_or(Error::Corto)?;
                if sig == 0x00 {
                    self.pos += 1;
                } else {
                    // Un marcador dentro del flujo: RSTn o EOI. Se deja
                    // apuntado y se alimenta con ceros, como manda la norma.
                    self.marcador = true;
                    self.pos -= 1;
                    byte = 0;
                }
            }
            self.acum = byte as u32;
            self.cuantos = 8;
        }
        self.cuantos -= 1;
        Ok((self.acum >> self.cuantos) & 1)
    }

    fn bits(&mut self, n: u32) -> Result<u32, Error> {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.bit()?;
        }
        Ok(v)
    }

    fn simbolo(&mut self, h: &Huffman) -> Result<u8, Error> {
        if !h.hay {
            return Err(Error::Variante);
        }
        let mut codigo: i32 = 0;
        for l in 1..=16 {
            codigo = (codigo << 1) | self.bit()? as i32;
            if h.maxcode[l] >= 0 && codigo <= h.maxcode[l] && codigo >= h.mincode[l] {
                let i = h.valptr[l] + codigo - h.mincode[l];
                return h.valores.get(i as usize).copied().ok_or(Error::Variante);
            }
        }
        Err(Error::Variante)
    }

    /// Tras un intervalo de reinicio: se salta el RSTn y se vuelve a byte.
    fn reinicia(&mut self) -> Result<(), Error> {
        self.cuantos = 0;
        self.marcador = false;
        // Puede haber bytes de relleno FF antes del marcador.
        while self.b.get(self.pos) == Some(&0xFF) && matches!(self.b.get(self.pos + 1), Some(0xFF)) {
            self.pos += 1;
        }
        if self.b.get(self.pos) == Some(&0xFF) && matches!(self.b.get(self.pos + 1), Some(0xD0..=0xD7)) {
            self.pos += 2;
            Ok(())
        } else {
            Err(Error::Variante)
        }
    }
}

/// El valor con signo de `n` bits de una categoria (F.2.2.1).
fn extiende(v: u32, n: u32) -> i32 {
    if n == 0 {
        0
    } else if v < (1 << (n - 1)) {
        v as i32 - (1 << n) + 1
    } else {
        v as i32
    }
}

struct Componente {
    id: u8,
    h: usize,
    v: usize,
    cuant: usize,
    dc: usize,
    ac: usize,
    pred: i32,
}

/// Que es y cuanto mide, del SOF0. Lo que no se decodifica se dice aqui.
pub fn medir(b: &[u8]) -> Result<Medidas, Error> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return Err(Error::NoEsImagen);
    }
    let mut o = 2usize;
    loop {
        // Los marcadores pueden llevar FF de relleno delante.
        while b.get(o) == Some(&0xFF) {
            o += 1;
        }
        let m = *b.get(o).ok_or(Error::Corto)?;
        o += 1;
        match m {
            0xC0 => {
                let len = be16(b, o).ok_or(Error::Corto)?;
                if len < 8 || b.get(o + 2) != Some(&8) {
                    return Err(Error::Variante);
                }
                let h = be16(b, o + 3).ok_or(Error::Corto)? as u32;
                let w = be16(b, o + 5).ok_or(Error::Corto)? as u32;
                lados(w, h)?;
                return Ok(Medidas { ancho: w, alto: h, formato: Formato::Jpeg });
            }
            // Progresivo, sin perdida, aritmetico, jerarquico: no.
            0xC1..=0xCF if m != 0xC4 && m != 0xC8 && m != 0xCC => return Err(Error::Variante),
            0xD8 | 0x01 | 0xD0..=0xD7 => {}
            0xD9 | 0xDA => return Err(Error::Variante),
            _ => {
                let len = be16(b, o).ok_or(Error::Corto)?;
                o = o.checked_add(len).ok_or(Error::Corto)?;
            }
        }
    }
}

/// **Decodifica un JPEG ya medido en `dst`.**
pub fn decodificar(b: &[u8], m: &Medidas, dst: &mut [u32]) -> Result<(), Error> {
    let mut cuant = [[0u16; 64]; 4];
    let mut dc = [HUFF_VACIA; 4];
    let mut ac = [HUFF_VACIA; 4];
    let mut comps: [Componente; 3] = [
        Componente { id: 0, h: 1, v: 1, cuant: 0, dc: 0, ac: 0, pred: 0 },
        Componente { id: 0, h: 1, v: 1, cuant: 0, dc: 0, ac: 0, pred: 0 },
        Componente { id: 0, h: 1, v: 1, cuant: 0, dc: 0, ac: 0, pred: 0 },
    ];
    let mut cuantos = 0usize;
    let mut intervalo = 0usize;
    let mut o = 2usize;

    // -- Las cabeceras, hasta el SOS -------------------------------------
    let datos = loop {
        while b.get(o) == Some(&0xFF) {
            o += 1;
        }
        let marcador = *b.get(o).ok_or(Error::Corto)?;
        o += 1;
        if matches!(marcador, 0xD8 | 0x01 | 0xD0..=0xD7) {
            continue;
        }
        let len = be16(b, o).ok_or(Error::Corto)?;
        let seg = b.get(o + 2..o + len).ok_or(Error::Corto)?;
        o += len;
        match marcador {
            0xDB => {
                let mut i = 0;
                while i < seg.len() {
                    let (precision, t) = (seg[i] >> 4, (seg[i] & 0x0F) as usize);
                    if precision != 0 || t > 3 {
                        return Err(Error::Variante);
                    }
                    let tabla = seg.get(i + 1..i + 65).ok_or(Error::Corto)?;
                    for k in 0..64 {
                        cuant[t][ZIGZAG[k] as usize] = tabla[k] as u16;
                    }
                    i += 65;
                }
            }
            0xC4 => {
                let mut i = 0;
                while i < seg.len() {
                    let (clase, t) = (seg[i] >> 4, (seg[i] & 0x0F) as usize);
                    if clase > 1 || t > 3 {
                        return Err(Error::Variante);
                    }
                    let cuentas: [u8; 16] = seg.get(i + 1..i + 17).ok_or(Error::Corto)?.try_into().unwrap();
                    let n: usize = cuentas.iter().map(|&c| c as usize).sum();
                    let valores = seg.get(i + 17..i + 17 + n).ok_or(Error::Corto)?;
                    let h = Huffman::de(&cuentas, valores)?;
                    if clase == 0 {
                        dc[t] = h;
                    } else {
                        ac[t] = h;
                    }
                    i += 17 + n;
                }
            }
            0xC0 => {
                cuantos = *seg.get(5).ok_or(Error::Corto)? as usize;
                if cuantos != 1 && cuantos != 3 {
                    return Err(Error::Variante);
                }
                for c in 0..cuantos {
                    let e = seg.get(6 + c * 3..9 + c * 3).ok_or(Error::Corto)?;
                    comps[c].id = e[0];
                    comps[c].h = (e[1] >> 4) as usize;
                    comps[c].v = (e[1] & 0x0F) as usize;
                    comps[c].cuant = e[2] as usize;
                    if comps[c].cuant > 3 {
                        return Err(Error::Variante);
                    }
                }
                // El Y puede ir a 1 o 2 en cada eje; Cb y Cr a 1.
                let ok = comps[0].h >= 1 && comps[0].h <= 2 && comps[0].v >= 1 && comps[0].v <= 2
                    && (cuantos == 1 || (comps[1].h == 1 && comps[1].v == 1 && comps[2].h == 1 && comps[2].v == 1));
                if !ok {
                    return Err(Error::Variante);
                }
            }
            0xDD => intervalo = be16(seg, 0).ok_or(Error::Corto)?,
            0xDA => {
                let n = *seg.first().ok_or(Error::Corto)? as usize;
                if n != cuantos || cuantos == 0 {
                    return Err(Error::Variante);
                }
                for k in 0..n {
                    let e = seg.get(1 + k * 2..3 + k * 2).ok_or(Error::Corto)?;
                    let c = comps[..cuantos].iter().position(|c| c.id == e[0]).ok_or(Error::Variante)?;
                    comps[c].dc = (e[1] >> 4) as usize;
                    comps[c].ac = (e[1] & 0x0F) as usize;
                    if comps[c].dc > 3 || comps[c].ac > 3 {
                        return Err(Error::Variante);
                    }
                }
                break o;
            }
            0xC1..=0xCF | 0xF7 => return Err(Error::Variante),
            0xD9 => return Err(Error::Corto),
            _ => {}
        }
    };

    // -- Los MCU ------------------------------------------------------------
    let (w, h) = (m.ancho as usize, m.alto as usize);
    let (hy, vy) = if cuantos == 1 { (1, 1) } else { (comps[0].h, comps[0].v) };
    let (mcu_w, mcu_h) = (8 * hy, 8 * vy);
    let (por_fila, filas) = ((w + mcu_w - 1) / mcu_w, (h + mcu_h - 1) / mcu_h);
    let mut bits = Bits { b, pos: datos, acum: 0, cuantos: 0, marcador: false };
    // Los bloques de un MCU: hasta 4 de Y, 1 de Cb, 1 de Cr, ya como muestras.
    let mut y_bl = [[0u8; 64]; 4];
    let mut cb_bl = [0u8; 64];
    let mut cr_bl = [0u8; 64];
    let mut coef = [0i32; 64];
    let mut desde_reinicio = 0usize;
    for my in 0..filas {
        for mx in 0..por_fila {
            if intervalo > 0 && desde_reinicio == intervalo {
                bits.reinicia()?;
                for c in comps.iter_mut() {
                    c.pred = 0;
                }
                desde_reinicio = 0;
            }
            for k in 0..hy * vy {
                bloque(&mut bits, &mut comps[0], &dc, &ac, &cuant, &mut coef, &mut y_bl[k])?;
            }
            if cuantos == 3 {
                bloque(&mut bits, &mut comps[1], &dc, &ac, &cuant, &mut coef, &mut cb_bl)?;
                bloque(&mut bits, &mut comps[2], &dc, &ac, &cuant, &mut coef, &mut cr_bl)?;
            }
            desde_reinicio += 1;
            // A pixeles, con recorte en el borde.
            for py in 0..mcu_h {
                let y = my * mcu_h + py;
                if y >= h {
                    break;
                }
                for px in 0..mcu_w {
                    let x = mx * mcu_w + px;
                    if x >= w {
                        break;
                    }
                    let k = (py / 8) * hy + px / 8;
                    let luma = y_bl[k][(py % 8) * 8 + px % 8] as i32;
                    let p = if cuantos == 1 {
                        OPACO | (luma as u32) << 16 | (luma as u32) << 8 | luma as u32
                    } else {
                        // El croma cubre `hy x vy` pixeles: se coge el mas cercano.
                        let (cx, cy) = (px / hy, py / vy);
                        let cb = cb_bl[cy * 8 + cx] as i32 - 128;
                        let cr = cr_bl[cy * 8 + cx] as i32 - 128;
                        // BT.601 en punto fijo de 16 bits.
                        let r = luma + ((91881 * cr) >> 16);
                        let g = luma - ((22554 * cb + 46802 * cr) >> 16);
                        let bb = luma + ((116130 * cb) >> 16);
                        OPACO | (recorta(r) << 16) | (recorta(g) << 8) | recorta(bb)
                    };
                    dst[y * w + x] = p;
                }
            }
        }
    }
    Ok(())
}

fn recorta(v: i32) -> u32 {
    v.clamp(0, 255) as u32
}

/// Un bloque de 8x8: Huffman, dequantizacion, IDCT y a muestras de 8 bits.
fn bloque(
    bits: &mut Bits<'_>,
    c: &mut Componente,
    dc: &[Huffman; 4],
    ac: &[Huffman; 4],
    cuant: &[[u16; 64]; 4],
    coef: &mut [i32; 64],
    salida: &mut [u8; 64],
) -> Result<(), Error> {
    for x in coef.iter_mut() {
        *x = 0;
    }
    let q = &cuant[c.cuant];
    let t = bits.simbolo(&dc[c.dc])? as u32;
    if t > 11 {
        return Err(Error::Variante);
    }
    let diff = extiende(bits.bits(t)?, t);
    c.pred += diff;
    coef[0] = c.pred * q[0] as i32;
    let mut k = 1;
    while k < 64 {
        let rs = bits.simbolo(&ac[c.ac])?;
        let (run, size) = ((rs >> 4) as usize, (rs & 0x0F) as u32);
        if size == 0 {
            if run == 15 {
                k += 16;
                continue;
            }
            break; // fin de bloque
        }
        k += run;
        if k > 63 {
            return Err(Error::Variante);
        }
        let v = extiende(bits.bits(size)?, size);
        let z = ZIGZAG[k] as usize;
        coef[z] = v * q[z] as i32;
        k += 1;
    }
    idct(coef, salida);
    Ok(())
}

/// IDCT separable en punto fijo: filas y despues columnas.
fn idct(coef: &[i32; 64], salida: &mut [u8; 64]) {
    let mut tmp = [0i32; 64];
    for y in 0..8 {
        for x in 0..8 {
            let mut s: i64 = 0;
            for u in 0..8 {
                s += COS[x][u] as i64 * coef[y * 8 + u] as i64;
            }
            // Los 13 bits del coseno: se bajan 8 aqui y 18 despues (13+13-8).
            tmp[y * 8 + x] = (s >> 8) as i32;
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            let mut s: i64 = 0;
            for v in 0..8 {
                s += COS[y][v] as i64 * tmp[v * 8 + x] as i64;
            }
            // 13 + 13 bits de los dos cosenos, menos los 8 que ya se bajaron,
            // y el 1/4 de la norma: 2^20. Y el +128 del nivel.
            let val = ((s + (1 << 19)) >> 20) as i32 + 128;
            salida[y * 8 + x] = recorta(val) as u8;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Escritos por Pillow (libjpeg) y decodificados por el mismo libjpeg a
    // `*.rgb`: la comparacion es contra OTRO decodificador, con la tolerancia
    // que separa dos IDCT distintas y dos formas de subir el croma.
    const LISO: &[u8] = include_bytes!("../pruebas/liso.jpg");
    const G420: &[u8] = include_bytes!("../pruebas/grad420.jpg");
    const G444: &[u8] = include_bytes!("../pruebas/grad444.jpg");
    const GRIS: &[u8] = include_bytes!("../pruebas/gris.jpg");
    const RESTART: &[u8] = include_bytes!("../pruebas/restart.jpg");
    const PROGRESIVO: &[u8] = include_bytes!("../pruebas/progresivo.jpg");

    fn dec(b: &[u8]) -> Result<(Medidas, Vec<u32>), Error> {
        let mut dst = vec![0u32; 64 * 48];
        let m = crate::decodificar(b, &mut dst)?;
        dst.truncate((m.ancho * m.alto) as usize);
        Ok((m, dst))
    }

    /// Error medio y maximo contra el `.rgb` de libjpeg.
    fn contra(px: &[u32], rgb: &[u8]) -> (u32, u32) {
        let mut suma = 0u32;
        let mut max = 0u32;
        for (i, p) in px.iter().enumerate() {
            for (c, canal) in [(p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF].iter().enumerate() {
                let d = (*canal as i32 - rgb[i * 3 + c] as i32).unsigned_abs();
                suma += d;
                max = max.max(d);
            }
        }
        (suma / (px.len() as u32 * 3), max)
    }

    #[test]
    fn un_color_liso_444() {
        let (m, px) = dec(LISO).unwrap();
        assert_eq!((m.ancho, m.alto, m.formato), (16, 16, Formato::Jpeg));
        let (medio, max) = contra(&px, include_bytes!("../pruebas/liso.rgb"));
        assert!(medio <= 1 && max <= 3, "medio {medio} max {max}");
        assert!(px.iter().all(|p| p & OPACO == OPACO));
    }

    #[test]
    fn un_degradado_444_y_gris() {
        let (m, px) = dec(G444).unwrap();
        assert_eq!((m.ancho, m.alto), (64, 48));
        let (medio, max) = contra(&px, include_bytes!("../pruebas/grad444.rgb"));
        assert!(medio <= 2 && max <= 12, "444: medio {medio} max {max}");
        let (_, px) = dec(GRIS).unwrap();
        let (medio, max) = contra(&px, include_bytes!("../pruebas/gris.rgb"));
        assert!(medio <= 2 && max <= 8, "gris: medio {medio} max {max}");
    }

    /// 4:2:0: el croma se sube al vecino mas cercano; libjpeg lo SUAVIZA
    /// (fancy upsampling). En el degradado hay una raya donde el azul salta
    /// de 255 a 0, y ahi las dos formas de subir el croma se separan hasta
    /// ~85 en un pixel: por eso el maximo tolera un borde y el medio (1) no.
    #[test]
    fn un_degradado_420_y_sus_reinicios() {
        let (m, px) = dec(G420).unwrap();
        assert_eq!((m.ancho, m.alto), (64, 48));
        let (medio, max) = contra(&px, include_bytes!("../pruebas/grad420.rgb"));
        assert!(medio <= 2 && max <= 100, "420: medio {medio} max {max}");
        let (_, px) = dec(RESTART).unwrap();
        let (medio, max) = contra(&px, include_bytes!("../pruebas/restart.rgb"));
        assert!(medio <= 2 && max <= 100, "restart: medio {medio} max {max}");
    }

    /// Una FOTO de verdad (la del arranque, en `docs/evidencia`, a 640 de
    /// ancho): baseline 4:2:0 con muchos MCU y un borde que no es multiplo.
    #[test]
    fn una_foto_de_verdad() {
        let mut dst = vec![0u32; 640 * 362];
        let m = crate::decodificar(include_bytes!("../pruebas/foto.jpg"), &mut dst).unwrap();
        assert_eq!((m.ancho, m.alto), (640, 362));
        let (medio, max) = contra(&dst, include_bytes!("../pruebas/foto.rgb"));
        assert!(medio <= 2 && max <= 100, "foto: medio {medio} max {max}");
    }

    #[test]
    fn progresivo_se_dice_con_nombre() {
        assert_eq!(crate::medir(PROGRESIVO).err(), Some(Error::Variante));
        assert_eq!(dec(PROGRESIVO).err(), Some(Error::Variante));
    }

    #[test]
    fn un_jpeg_cortado_es_corto() {
        for n in [2, 4, 20, 100, 300, G444.len() - 100, G444.len() - 3] {
            match dec(&G444[..n]) {
                Err(Error::Corto) | Err(Error::Variante) => {}
                otro => panic!("con {n} bytes: {otro:?}"),
            }
        }
    }

    #[test]
    fn un_byte_cambiado_no_revienta() {
        for i in 2..G420.len() {
            let mut b = G420.to_vec();
            b[i] ^= 0x3C;
            let _ = dec(&b);
        }
    }

    /// La IDCT entera: un bloque solo DC sale plano en el valor esperado.
    #[test]
    fn la_idct_de_un_bloque_plano() {
        let mut coef = [0i32; 64];
        coef[0] = 8 * 50; // DC de 400 -> 400/8 = 50 sobre 128
        let mut s = [0u8; 64];
        idct(&coef, &mut s);
        assert!(s.iter().all(|&v| (v as i32 - 178).abs() <= 1), "{:?}", &s[..8]);
    }
}
