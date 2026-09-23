//! **DEFLATE para ESCRIBIR** (RFC 1951, dentro de zlib RFC 1950) -- el espejo de
//! `inflate` (2026-09-22).
//!
//! generacion: nieto -- recibe bytes, un taller y un bufer de salida, y devuelve
//! cuantos bytes escribio o un motivo; no sabe de ficheros ni de pantallas.
//!
//! El propietario, con la primera captura de pantalla de BMO-X en la mano: *"en
//! caso de PNG, que tan inesperado es cuando BMO-X tiene el PNG RECONSTRUIDO"*.
//! Esto es la mitad que faltaba: BMO-X ya sabia DESCOMPRIMIR (el visor lee PNG
//! con su propio `inflate`), y no sabia comprimir. Un PNG hecho aqui es un PNG
//! como cualquier otro: el formato es un estandar abierto (RFC 1950/1951 e ISO
//! 15948), no es de nadie, y lo lee cualquier visor del mundo.
//!
//! ## Lo que hace
//!
//! ```text
//!    LZ77      cadenas de hash de 3 bytes, ventana de 32 KiB, hasta 32
//!              candidatos, y en cuanto aparece una coincidencia de 258 (la
//!              maxima) se para de buscar: en una captura hay filas enteras
//!              iguales y esa es la coincidencia que se repite
//!    Huffman   DINAMICO, un arbol por bloque, limitado a 15 bits (7 para el de
//!              las longitudes). Con el fijo, una FOTO no comprimia nada: los
//!              residuos chicos negativos (255, 254...) cuestan 9 bits
//!    STORED    si un bloque comprimido saldria mas grande que tal cual, va tal
//!              cual. Por eso [`cota`] es exacta y chica: la salida nunca
//!              pasa de la entrada mas unos bytes por bloque
//! ```
//!
//! ## Lo que NO hace, dicho
//!
//! No hay coincidencia perezosa (lazy matching) ni arboles optimos por
//! package-merge: si un arbol se pasa de 15 bits, las frecuencias se parten por
//! la mitad y se vuelve a construir. Sale un poco peor que zlib -9 y mucho
//! mejor que no comprimir; lo que importa aqui es que sea CORRECTO y que se
//! pueda leer entero.

use crate::Error;

/// La ventana de LZ77: la mas grande que admite DEFLATE.
const VENTANA: usize = 32 * 1024;
const HASH_BITS: u32 = 15;
const HASH: usize = 1 << HASH_BITS;
/// Simbolos por bloque antes de cerrarlo y hacer su arbol.
const BLOQUE: usize = 16 * 1024;
/// Cuantos candidatos se prueban por posicion.
const CADENA: u32 = 32;
const MIN: usize = 3;
const MAX: usize = 258;

/// Taller que pide [`comprimir`], en `u32`: las cabezas del hash, la cadena de
/// la ventana y los simbolos del bloque en curso. 384 KiB.
///
/// En `u32` y no en bytes a proposito: asi el taller llega ya alineado y aqui
/// no hace falta ni un `unsafe` para mirarlo como enteros.
pub const TALLER: usize = HASH + VENTANA + BLOQUE;

/// **Lo mas que puede ocupar** comprimir `n` bytes: tal cual, mas la cabecera
/// y el pie de zlib y 5 bytes por bloque STORED (de 65.535 como mucho).
pub fn cota(n: usize) -> usize {
    n + (n / 65_535 + 1) * 5 + 64
}

// -- Las tablas del formato ----------------------------------------------

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163,
    195, 227, 258,
];
const LEXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073,
    4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
];
/// El orden en que se escriben las longitudes del arbol de longitudes.
const ORDEN_CL: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn codigo_de_longitud(len: usize) -> usize {
    let mut i = 28;
    while LBASE[i] as usize > len {
        i -= 1;
    }
    i
}

fn codigo_de_distancia(d: usize) -> usize {
    let mut i = 29;
    while DBASE[i] as usize > d {
        i -= 1;
    }
    i
}

// -- La salida de bits ---------------------------------------------------

struct Bits<'a> {
    dst: &'a mut [u8],
    n: usize,
    acc: u64,
    nb: u32,
    lleno: bool,
}

impl<'a> Bits<'a> {
    fn byte(&mut self, b: u8) {
        if self.n < self.dst.len() {
            self.dst[self.n] = b;
            self.n += 1;
        } else {
            self.lleno = true;
        }
    }
    /// `n` bits de `v`, el menos significativo primero (el orden de DEFLATE).
    fn put(&mut self, v: u32, n: u32) {
        self.acc |= (v as u64) << self.nb;
        self.nb += n;
        while self.nb >= 8 {
            self.byte(self.acc as u8);
            self.acc >>= 8;
            self.nb -= 8;
        }
    }
    fn alinear(&mut self) {
        if self.nb > 0 {
            self.byte(self.acc as u8);
            self.acc = 0;
            self.nb = 0;
        }
    }
}

// -- Huffman ---------------------------------------------------------------

/// **Las longitudes de un codigo de Huffman** para `freq`, ninguna mas larga
/// que `limite`. Si el arbol se pasa, las frecuencias se parten por la mitad
/// (sin dejar a cero ninguna usada) y se vuelve a construir: aplana el arbol
/// hasta que cabe. Con una sola hoja usada, su longitud es 1.
fn longitudes(freq: &[u32], len: &mut [u8], limite: u8) {
    let n = freq.len();
    let mut f = [0u32; 288];
    f[..n].copy_from_slice(freq);
    loop {
        for l in len.iter_mut() {
            *l = 0;
        }
        let usadas = f[..n].iter().filter(|&&x| x > 0).count();
        if usadas == 0 {
            return;
        }
        if usadas == 1 {
            let i = f[..n].iter().position(|&x| x > 0).unwrap_or(0);
            len[i] = 1;
            return;
        }
        // Nodos: 0..n hojas, n.. internos. `peso` 0 = muerto.
        let mut peso = [0u64; 576];
        let mut padre = [u16::MAX; 576];
        let mut vivo = [false; 576];
        for i in 0..n {
            peso[i] = f[i] as u64;
            vivo[i] = f[i] > 0;
        }
        let mut total = n;
        let mut quedan = usadas;
        while quedan > 1 {
            // Los dos mas ligeros de los vivos.
            let (mut a, mut b) = (usize::MAX, usize::MAX);
            for i in 0..total {
                if !vivo[i] {
                    continue;
                }
                if a == usize::MAX || peso[i] < peso[a] {
                    b = a;
                    a = i;
                } else if b == usize::MAX || peso[i] < peso[b] {
                    b = i;
                }
            }
            peso[total] = peso[a] + peso[b];
            vivo[total] = true;
            vivo[a] = false;
            vivo[b] = false;
            padre[a] = total as u16;
            padre[b] = total as u16;
            total += 1;
            quedan -= 1;
        }
        let mut max = 0u8;
        for i in 0..n {
            if f[i] == 0 {
                continue;
            }
            let mut d = 0u8;
            let mut j = i;
            while padre[j] != u16::MAX {
                j = padre[j] as usize;
                d += 1;
            }
            len[i] = d;
            max = max.max(d);
        }
        if max <= limite {
            return;
        }
        for x in f[..n].iter_mut() {
            if *x > 0 {
                *x = (*x >> 1).max(1);
            }
        }
    }
}

/// Los codigos canonicos de unas longitudes, ya DADOS LA VUELTA: DEFLATE
/// escribe los codigos de Huffman del bit mas significativo al menos, y la
/// salida va del menos al mas.
fn codigos(len: &[u8], cod: &mut [u16]) {
    let mut cuenta = [0u16; 16];
    for &l in len {
        cuenta[l as usize] += 1;
    }
    cuenta[0] = 0;
    let mut sig = [0u16; 16];
    let mut c = 0u16;
    for b in 1..16 {
        c = (c + cuenta[b - 1]) << 1;
        sig[b] = c;
    }
    for (i, &l) in len.iter().enumerate() {
        if l == 0 {
            continue;
        }
        let v = sig[l as usize];
        sig[l as usize] += 1;
        let mut r = 0u16;
        for k in 0..l {
            if v & (1 << k) != 0 {
                r |= 1 << (l - 1 - k);
            }
        }
        cod[i] = r;
    }
}

// -- Un bloque ---------------------------------------------------------------

/// Un simbolo de LZ77: un literal (`< 256`), o `1<<31 | largo<<16 | distancia`.
const COINCIDE: u32 = 1 << 31;

fn cerrar_bloque(o: &mut Bits, simbolos: &[u32], src: &[u8], desde: usize, hasta: usize, fin: bool) {
    // -- Las frecuencias --
    let mut flit = [0u32; 286];
    let mut fdist = [0u32; 30];
    for &s in simbolos {
        if s & COINCIDE == 0 {
            flit[s as usize] += 1;
        } else {
            let largo = ((s >> 16) & 0x7FFF) as usize;
            let dist = (s & 0xFFFF) as usize;
            flit[257 + codigo_de_longitud(largo)] += 1;
            fdist[codigo_de_distancia(dist)] += 1;
        }
    }
    flit[256] = 1;
    let mut llit = [0u8; 286];
    let mut ldist = [0u8; 30];
    longitudes(&flit, &mut llit, 15);
    longitudes(&fdist, &mut ldist, 15);
    // Sin ninguna distancia, DEFLATE pide igual un codigo: uno de longitud 1.
    if ldist.iter().all(|&l| l == 0) {
        ldist[0] = 1;
    }
    let hlit = (257..=286).rev().find(|&k| llit[k - 1] != 0).unwrap_or(257).max(257);
    let hdist = (1..=30).rev().find(|&k| ldist[k - 1] != 0).unwrap_or(1).max(1);

    // -- Las longitudes, en RLE (16 repite, 17 y 18 ceros) --
    let mut todas = [0u8; 316];
    todas[..hlit].copy_from_slice(&llit[..hlit]);
    todas[hlit..hlit + hdist].copy_from_slice(&ldist[..hdist]);
    let todas = &todas[..hlit + hdist];
    let mut rle = [(0u8, 0u8); 316]; // (simbolo, extra)
    let mut nr = 0usize;
    let mut i = 0usize;
    while i < todas.len() {
        let l = todas[i];
        let mut j = i + 1;
        while j < todas.len() && todas[j] == l {
            j += 1;
        }
        let mut run = j - i;
        if l == 0 {
            while run >= 11 {
                let k = run.min(138);
                rle[nr] = (18, (k - 11) as u8);
                nr += 1;
                run -= k;
            }
            if run >= 3 {
                rle[nr] = (17, (run - 3) as u8);
                nr += 1;
                run = 0;
            }
        } else {
            rle[nr] = (l, 0);
            nr += 1;
            run -= 1;
            while run >= 3 {
                let k = run.min(6);
                rle[nr] = (16, (k - 3) as u8);
                nr += 1;
                run -= k;
            }
        }
        for _ in 0..run {
            rle[nr] = (l, 0);
            nr += 1;
        }
        i = j;
    }
    let mut fcl = [0u32; 19];
    for &(s, _) in &rle[..nr] {
        fcl[s as usize] += 1;
    }
    // ** El arbol de las longitudes tiene que ser COMPLETO: zlib rechaza uno
    // de un solo codigo de 1 bit (el de distancias si puede, este no). Si solo
    // se usa un simbolo, se le da pareja: un codigo que no se escribe nunca.
    if fcl.iter().filter(|&&x| x > 0).count() < 2 {
        let k = if fcl[0] == 0 { 0 } else { 1 };
        fcl[k] = 1;
    }
    let mut lcl = [0u8; 19];
    longitudes(&fcl, &mut lcl, 7);
    let hclen = (4..=19).rev().find(|&k| lcl[ORDEN_CL[k - 1]] != 0).unwrap_or(4).max(4);

    // -- Cuanto costaria, contra tal cual --
    let extra_rle = |s: u8| match s {
        16 => 2,
        17 => 3,
        18 => 7,
        _ => 0,
    };
    let mut bits: u64 = 3 + 5 + 5 + 4 + 3 * hclen as u64;
    for &(s, _) in &rle[..nr] {
        bits += lcl[s as usize] as u64 + extra_rle(s);
    }
    for (k, &f) in flit.iter().enumerate() {
        bits += f as u64 * llit[k] as u64;
        if k >= 257 {
            bits += f as u64 * LEXTRA[k - 257] as u64;
        }
    }
    for (k, &f) in fdist.iter().enumerate() {
        bits += f as u64 * (ldist[k] as u64 + DEXTRA[k] as u64);
    }
    let crudo = hasta - desde;
    let tal_cual = (crudo as u64 + (crudo as u64 / 65_535 + 1) * 5) * 8 + 8;
    // Un bloque sin simbolos (entrada vacia) va tal cual: su arbol de literales
    // tendria un solo codigo, y eso no todos los lectores lo aceptan.
    if bits > tal_cual || simbolos.is_empty() {
        // ** STORED: no gana comprimir. En trozos de 65.535, el ultimo con
        // BFINAL si este es el ultimo bloque.
        let mut p = desde;
        loop {
            let k = (hasta - p).min(65_535);
            let ultimo = p + k == hasta;
            o.put((fin && ultimo) as u32, 1);
            o.put(0, 2);
            o.alinear();
            o.byte(k as u8);
            o.byte((k >> 8) as u8);
            o.byte(!k as u8);
            o.byte((!k >> 8) as u8);
            for &b in &src[p..p + k] {
                o.byte(b);
            }
            p += k;
            if ultimo {
                return;
            }
        }
    }

    // -- DINAMICO --
    let mut clit = [0u16; 286];
    let mut cdist = [0u16; 30];
    let mut ccl = [0u16; 19];
    codigos(&llit, &mut clit);
    codigos(&ldist, &mut cdist);
    codigos(&lcl, &mut ccl);
    o.put(fin as u32, 1);
    o.put(2, 2);
    o.put((hlit - 257) as u32, 5);
    o.put((hdist - 1) as u32, 5);
    o.put((hclen - 4) as u32, 4);
    for &k in &ORDEN_CL[..hclen] {
        o.put(lcl[k] as u32, 3);
    }
    for &(s, e) in &rle[..nr] {
        o.put(ccl[s as usize] as u32, lcl[s as usize] as u32);
        let eb = extra_rle(s) as u32;
        if eb > 0 {
            o.put(e as u32, eb);
        }
    }
    for &s in simbolos {
        if s & COINCIDE == 0 {
            o.put(clit[s as usize] as u32, llit[s as usize] as u32);
        } else {
            let largo = ((s >> 16) & 0x7FFF) as usize;
            let dist = (s & 0xFFFF) as usize;
            let lc = codigo_de_longitud(largo);
            o.put(clit[257 + lc] as u32, llit[257 + lc] as u32);
            if LEXTRA[lc] > 0 {
                o.put((largo - LBASE[lc] as usize) as u32, LEXTRA[lc] as u32);
            }
            let dc = codigo_de_distancia(dist);
            o.put(cdist[dc] as u32, ldist[dc] as u32);
            if DEXTRA[dc] > 0 {
                o.put((dist - DBASE[dc] as usize) as u32, DEXTRA[dc] as u32);
            }
        }
    }
    o.put(clit[256] as u32, llit[256] as u32);
}

// -- La entrada --------------------------------------------------------------

fn hash(b: &[u8], i: usize) -> usize {
    (((b[i] as usize) << 10) ^ ((b[i + 1] as usize) << 5) ^ (b[i + 2] as usize)) & (HASH - 1)
}

fn adler32(b: &[u8]) -> u32 {
    let (mut a, mut s) = (1u32, 0u32);
    for trozo in b.chunks(5552) {
        for &x in trozo {
            a += x as u32;
            s += a;
        }
        a %= 65_521;
        s %= 65_521;
    }
    (s << 16) | a
}

/// **Comprime `src` como un flujo zlib en `dst`.** Devuelve cuantos bytes
/// escribio, o `NoCabe` si `dst` es mas chico que [`cota`] y no alcanzo, o
/// `SinTaller` si el taller no llega a [`TALLER`].
pub fn comprimir(src: &[u8], taller: &mut [u32], dst: &mut [u8]) -> Result<usize, Error> {
    if taller.len() < TALLER {
        return Err(Error::SinTaller);
    }
    let (cabezas, resto) = taller.split_at_mut(HASH);
    let (cadena, simbolos) = resto.split_at_mut(VENTANA);
    // Las cabezas guardan posicion + 1: el 0 es "nadie".
    for c in cabezas.iter_mut() {
        *c = 0;
    }

    let mut o = Bits { dst, n: 0, acc: 0, nb: 0, lleno: false };
    // zlib: deflate, ventana de 32 KiB, nivel "por defecto"; 0x789C es multiplo de 31.
    o.byte(0x78);
    o.byte(0x9C);

    let mut ns = 0usize;
    let mut desde = 0usize;
    let mut i = 0usize;
    let meter = |cabezas: &mut [u32], cadena: &mut [u32], p: usize| {
        if p + MIN <= src.len() {
            let h = hash(src, p);
            cadena[p & (VENTANA - 1)] = cabezas[h];
            cabezas[h] = (p + 1) as u32;
        }
    };
    while i < src.len() {
        let mut mejor = 0usize;
        let mut dist = 0usize;
        if i + MIN <= src.len() {
            let h = hash(src, i);
            let mut cand = cabezas[h] as usize;
            let tope = (src.len() - i).min(MAX);
            let mut vueltas = 0;
            while cand != 0 && vueltas < CADENA {
                let c = cand - 1;
                let d = i - c;
                if d > VENTANA || d == 0 {
                    break;
                }
                if src[c + mejor] == src[i + mejor] {
                    let mut l = 0usize;
                    while l < tope && src[c + l] == src[i + l] {
                        l += 1;
                    }
                    if l > mejor {
                        mejor = l;
                        dist = d;
                        if l == tope {
                            break;
                        }
                    }
                }
                let sig = cadena[c & (VENTANA - 1)] as usize;
                // La cadena solo va hacia atras: un eslabon que apunta
                // adelante es de otra vuelta de la ventana.
                if sig >= cand {
                    break;
                }
                cand = sig;
                vueltas += 1;
            }
        }
        if mejor >= MIN {
            simbolos[ns] = COINCIDE | ((mejor as u32) << 16) | dist as u32;
            for p in i..i + mejor {
                meter(cabezas, cadena, p);
            }
            i += mejor;
        } else {
            simbolos[ns] = src[i] as u32;
            meter(cabezas, cadena, i);
            i += 1;
        }
        ns += 1;
        if ns == BLOQUE {
            cerrar_bloque(&mut o, &simbolos[..ns], src, desde, i, i == src.len());
            ns = 0;
            desde = i;
            if o.lleno {
                return Err(Error::NoCabe);
            }
        }
    }
    if ns > 0 || src.is_empty() || desde < src.len() || o.n == 2 {
        cerrar_bloque(&mut o, &simbolos[..ns], src, desde, src.len(), true);
    }
    o.alinear();
    let a = adler32(src);
    for k in (0..4).rev() {
        o.byte((a >> (8 * k)) as u8);
    }
    if o.lleno {
        return Err(Error::NoCabe);
    }
    Ok(o.n)
}
