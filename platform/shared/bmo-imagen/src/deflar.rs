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
//!    LZ77      cadenas de hash de 3 bytes, ventana de 32 KiB, hasta 8
//!              candidatos (ver [`CADENA`]), y en cuanto aparece una coincidencia de 258 (la
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
//!
//! ## ** Y SE PUEDE PAUSAR (2026-09-23)
//!
//! La captura de pantalla congelaba el escritorio 1.068 ms en el Ryzen, casi
//! todo aqui y en el filtro de `png_escribir`. [`Deflar`] guarda por donde iba
//! --el cursor de la entrada, el bloque a medias y los bits sin volcar-- y
//! [`Deflar::paso`] avanza un presupuesto de bytes y vuelve. Quien llama decide
//! cuanto por fotograma; el resultado es el MISMO flujo que de una vez.
//!
//! Y lo que se hizo mas barato, medido con las dos capturas del Ryzen:
//!
//! ```text
//!    el codigo de un largo o distancia   tabla, no una busqueda de 29 pasos
//!    el arbol de Huffman                 dos colas sobre hojas ordenadas, no
//!                                        buscar los dos minimos cada vez
//!    comparar una coincidencia           de 8 en 8 bytes
//!    el hash de 3 bytes                  multiplicativo: el de antes perdia
//!                                        los bits altos del primer byte
//! ```

use crate::Error;

/// La ventana de LZ77: la mas grande que admite DEFLATE.
const VENTANA: usize = 32 * 1024;
const HASH_BITS: u32 = 15;
const HASH: usize = 1 << HASH_BITS;
/// Simbolos por bloque antes de cerrarlo y hacer su arbol.
const BLOQUE: usize = 16 * 1024;
/// Cuantos candidatos se prueban por posicion.
///
/// ** Era 32. Medido el 2026-09-23 con las dos capturas del Ryzen (1920x1080),
/// en el anfitrion:
///
/// ```text
///    CADENA   la ciudad                 las ventanas
///      32     2.744.477 B  121 ms       708.628 B  34 ms
///      16     2.766.040 B   94 ms       713.350 B  30 ms
///       8     2.787.585 B   74 ms       717.846 B  27 ms   <- esta
///       4     2.810.403 B   61 ms       724.219 B  25 ms
/// ```
///
/// Un 1,6 % mas de fichero por un 40 % menos de trabajo. Con la captura partida
/// por fotogramas ya no congela nada, asi que lo que se elige aqui es CPU
/// (vatios) contra bytes de disco, y el disco no es el que falta.
const CADENA: u32 = 8;
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

/// El codigo de cada largo (3..=258), hecho al compilar.
const LCODIGO: [u8; 259] = {
    let mut t = [0u8; 259];
    let mut c = 0;
    while c < 29 {
        let mut l = LBASE[c] as usize;
        let hasta = if c == 28 { 259 } else { LBASE[c] as usize + (1 << LEXTRA[c]) };
        while l < hasta && l < 259 {
            t[l] = c as u8;
            l += 1;
        }
        c += 1;
    }
    t
};

/// El codigo de cada distancia, a la manera de zlib: las 256 primeras una a
/// una, y de ahi en adelante de 128 en 128 (los codigos altos van alineados).
const DCODIGO: [u8; 512] = {
    let mut t = [0u8; 512];
    let mut c = 0;
    while c < 30 {
        let desde = DBASE[c] as usize;
        let hasta = desde + (1 << DEXTRA[c]);
        let mut d = desde;
        while d < hasta {
            let k = if d - 1 < 256 { d - 1 } else { 256 + ((d - 1) >> 7) };
            t[k] = c as u8;
            d += 1;
        }
        c += 1;
    }
    t
};

fn codigo_de_longitud(len: usize) -> usize {
    LCODIGO[len] as usize
}

fn codigo_de_distancia(d: usize) -> usize {
    let k = if d - 1 < 256 { d - 1 } else { 256 + ((d - 1) >> 7) };
    DCODIGO[k] as usize
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
        // Las hojas usadas, de la mas ligera a la mas pesada.
        let mut hojas = [(0u32, 0u16); 288];
        let mut u = 0usize;
        for (i, &x) in f[..n].iter().enumerate() {
            if x > 0 {
                hojas[u] = (x, i as u16);
                u += 1;
            }
        }
        if u == 0 {
            return;
        }
        if u == 1 {
            len[hojas[0].1 as usize] = 1;
            return;
        }
        hojas[..u].sort_unstable();
        // ** DOS COLAS: las hojas ya ordenadas, y los nodos internos, que
        // salen en orden no decreciente por construccion. Los dos mas ligeros
        // estan siempre al principio de alguna de las dos: O(n) tras ordenar,
        // en vez de recorrer todos los vivos por cada fusion.
        let mut peso = [0u64; 576];
        let mut padre = [0u16; 576];
        for k in 0..u {
            peso[k] = hojas[k].0 as u64;
        }
        let (mut qh, mut qi, mut total) = (0usize, u, u);
        for _ in 0..u - 1 {
            let mut dos = [0usize; 2];
            for d in &mut dos {
                if qh < u && (qi >= total || peso[qh] <= peso[qi]) {
                    *d = qh;
                    qh += 1;
                } else {
                    *d = qi;
                    qi += 1;
                }
            }
            peso[total] = peso[dos[0]] + peso[dos[1]];
            padre[dos[0]] = total as u16;
            padre[dos[1]] = total as u16;
            total += 1;
        }
        // La profundidad, de la raiz hacia abajo: un padre tiene siempre un
        // indice mayor que sus hijos.
        let mut prof = [0u8; 576];
        let mut max = 0u8;
        for k in (0..total - 1).rev() {
            prof[k] = prof[padre[k] as usize] + 1;
        }
        for k in 0..u {
            len[hojas[k].1 as usize] = prof[k];
            max = max.max(prof[k]);
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

// -- La entrada, PAUSABLE ----------------------------------------------------

fn hash(b: &[u8], i: usize) -> usize {
    let v = b[i] as u32 | (b[i + 1] as u32) << 8 | (b[i + 2] as u32) << 16;
    (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

/// Cuantos bytes coinciden desde `c` y desde `i`, hasta `tope`. De 8 en 8: el
/// primer byte distinto lo dice el XOR de las dos palabras.
fn igual_hasta(src: &[u8], c: usize, i: usize, tope: usize) -> usize {
    let mut l = 0usize;
    while l + 8 <= tope {
        let a = u64::from_le_bytes([
            src[c + l], src[c + l + 1], src[c + l + 2], src[c + l + 3],
            src[c + l + 4], src[c + l + 5], src[c + l + 6], src[c + l + 7],
        ]);
        let b = u64::from_le_bytes([
            src[i + l], src[i + l + 1], src[i + l + 2], src[i + l + 3],
            src[i + l + 4], src[i + l + 5], src[i + l + 6], src[i + l + 7],
        ]);
        let x = a ^ b;
        if x != 0 {
            return l + (x.trailing_zeros() / 8) as usize;
        }
        l += 8;
    }
    while l < tope && src[c + l] == src[i + l] {
        l += 1;
    }
    l
}

/// **Un DEFLATE a medias**: todo lo que hace falta para seguir donde se dejo.
///
/// La entrada, el taller y la salida NO viven aqui: se le pasan en cada
/// [`Deflar::paso`], y tienen que ser LOS MISMOS de principio a fin (el taller
/// guarda la ventana y el bloque en curso).
pub struct Deflar {
    /// Por donde va la entrada.
    i: usize,
    /// Simbolos del bloque en curso, y donde empezo en la entrada.
    ns: usize,
    desde: usize,
    /// La salida: bytes escritos y los bits que aun no llenan uno.
    n: usize,
    acc: u64,
    nb: u32,
    /// Adler-32 de la entrada, llevado a la par.
    adler_a: u32,
    adler_s: u32,
    empezado: bool,
}

impl Default for Deflar {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Deflar {
    pub const fn nuevo() -> Self {
        Self { i: 0, ns: 0, desde: 0, n: 0, acc: 0, nb: 0, adler_a: 1, adler_s: 0, empezado: false }
    }

    /// Cuanto de la entrada lleva, en bytes.
    pub fn hecho(&self) -> usize {
        self.i
    }

    fn sumar_adler(&mut self, b: &[u8]) {
        for trozo in b.chunks(5552) {
            for &x in trozo {
                self.adler_a += x as u32;
                self.adler_s += self.adler_a;
            }
            self.adler_a %= 65_521;
            self.adler_s %= 65_521;
        }
    }

    /// **Avanza unos `presupuesto` bytes de la entrada** y vuelve. `Ok(Some(n))`
    /// cuando el flujo zlib esta entero en `dst[..n]`; `Ok(None)` si queda.
    ///
    /// Se para en frontera de simbolo, nunca a mitad de una coincidencia: el
    /// presupuesto es aproximado por arriba (una coincidencia de 258 pasa).
    pub fn paso(&mut self, src: &[u8], taller: &mut [u32], dst: &mut [u8], presupuesto: usize) -> Result<Option<usize>, Error> {
        if taller.len() < TALLER {
            return Err(Error::SinTaller);
        }
        let (cabezas, resto) = taller.split_at_mut(HASH);
        let (cadena, simbolos) = resto.split_at_mut(VENTANA);
        let mut o = Bits { dst, n: self.n, acc: self.acc, nb: self.nb, lleno: false };
        if !self.empezado {
            // Las cabezas guardan posicion + 1: el 0 es "nadie".
            for c in cabezas.iter_mut() {
                *c = 0;
            }
            // zlib: deflate, ventana de 32 KiB, nivel "por defecto"; 0x789C es
            // multiplo de 31.
            o.byte(0x78);
            o.byte(0x9C);
            self.empezado = true;
        }

        let (mut i, mut ns, mut desde) = (self.i, self.ns, self.desde);
        let empieza = i;
        let hasta = i.saturating_add(presupuesto);
        let meter = |cabezas: &mut [u32], cadena: &mut [u32], p: usize| {
            if p + MIN <= src.len() {
                let h = hash(src, p);
                cadena[p & (VENTANA - 1)] = cabezas[h];
                cabezas[h] = (p + 1) as u32;
            }
        };
        while i < src.len() && i < hasta {
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
                        let l = igual_hasta(src, c, i, tope);
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
        self.sumar_adler(&src[empieza..i]);
        if i < src.len() {
            // A medias: se guarda por donde iba y se vuelve.
            (self.i, self.ns, self.desde) = (i, ns, desde);
            (self.n, self.acc, self.nb) = (o.n, o.acc, o.nb);
            return Ok(None);
        }
        if ns > 0 || src.is_empty() || desde < src.len() || o.n == 2 {
            cerrar_bloque(&mut o, &simbolos[..ns], src, desde, src.len(), true);
        }
        o.alinear();
        let a = (self.adler_s << 16) | self.adler_a;
        for k in (0..4).rev() {
            o.byte((a >> (8 * k)) as u8);
        }
        if o.lleno {
            return Err(Error::NoCabe);
        }
        (self.i, self.ns, self.desde) = (i, 0, src.len());
        (self.n, self.acc, self.nb) = (o.n, 0, 0);
        Ok(Some(o.n))
    }
}

/// **Comprime `src` como un flujo zlib en `dst`, de una vez.** Devuelve cuantos
/// bytes escribio, o `NoCabe` si `dst` es mas chico que [`cota`] y no alcanzo,
/// o `SinTaller` si el taller no llega a [`TALLER`].
pub fn comprimir(src: &[u8], taller: &mut [u32], dst: &mut [u8]) -> Result<usize, Error> {
    let mut d = Deflar::nuevo();
    loop {
        if let Some(n) = d.paso(src, taller, dst, usize::MAX)? {
            return Ok(n);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// ** A TROZOS SALE EXACTAMENTE LO MISMO QUE DE UNA: el mismo flujo, byte a
    /// byte. Si no, pausar cambiaria el fichero segun cuanto tardara la maquina.
    #[test]
    fn a_trozos_da_el_mismo_flujo() {
        let src: Vec<u8> = (0..200_000u32)
            .map(|i| if (i / 700) % 3 == 0 { (i % 7) as u8 } else { (i.wrapping_mul(2_654_435_761) >> 24) as u8 })
            .collect();
        let mut t1 = vec![0u32; TALLER];
        let mut d1 = vec![0u8; cota(src.len())];
        let n1 = comprimir(&src, &mut t1, &mut d1).unwrap();
        for presupuesto in [1usize, 777, 4096, 65_536] {
            let mut t2 = vec![0u32; TALLER];
            let mut d2 = vec![0u8; cota(src.len())];
            let mut z = Deflar::nuevo();
            let mut vueltas = 0;
            let n2 = loop {
                vueltas += 1;
                if let Some(n) = z.paso(&src, &mut t2, &mut d2, presupuesto).unwrap() {
                    break n;
                }
            };
            assert!(vueltas > 1, "con {presupuesto} no se llego a partir");
            assert_eq!(&d2[..n2], &d1[..n1], "a trozos de {presupuesto} sale otro flujo");
        }
    }

    /// Las tablas dicen lo mismo que la busqueda de antes, para todo valor.
    #[test]
    fn las_tablas_de_codigo_cuadran() {
        for l in 3..=258usize {
            let mut i = 28;
            while LBASE[i] as usize > l {
                i -= 1;
            }
            assert_eq!(codigo_de_longitud(l), i, "largo {l}");
        }
        for d in 1..=32_768usize {
            let mut i = 29;
            while DBASE[i] as usize > d {
                i -= 1;
            }
            assert_eq!(codigo_de_distancia(d), i, "distancia {d}");
        }
    }

    /// Ida y vuelta con el inflate de la casa.
    #[test]
    fn se_lee_con_el_inflate_de_la_casa() {
        let src: Vec<u8> = (0..100_000u32).map(|i| ((i / 3) % 251) as u8 ^ (i >> 12) as u8).collect();
        let mut t = vec![0u32; TALLER];
        let mut d = vec![0u8; cota(src.len())];
        let n = comprimir(&src, &mut t, &mut d).unwrap();
        let mut vuelta: Vec<u8> = Vec::new();
        let mut ventana = vec![0u8; 32 * 1024];
        let mut pozo = |b: u8| {
            vuelta.push(b);
            Ok(())
        };
        crate::inflate::inflar_zlib(&[&d[..n]], &mut ventana, &mut pozo).expect("se infla");
        assert_eq!(vuelta, src);
    }
}
