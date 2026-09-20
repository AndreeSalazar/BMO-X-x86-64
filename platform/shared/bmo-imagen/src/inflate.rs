//! **INFLATE** (RFC 1951) -- lo que hay dentro de un PNG, escrito aqui y sin
//! `alloc`.
//!
//! generacion: nieto -- recibe bytes y un pozo donde escribir; no sabe que es
//! un PNG ni una imagen.
//!
//! ## Por que propio y no `miniz`/`flate2`
//!
//! Por la regla de la casa: nada de terceros en lo que corre en la maquina, y
//! porque un descompresor es la segunda clase de fallo mas famosa de las
//! librerias de imagenes (la primera es la medida que no se comprueba): una
//! distancia que apunta antes del principio de la ventana, un codigo que no
//! esta en la tabla, un bloque que promete mas bytes de los que llegan. Aqui
//! cada uno de esos es un `Error`, no una lectura fuera.
//!
//! ## La forma: la de `puff` (zlib), que cabe en una pantalla
//!
//! Los codigos de Huffman se descodifican bit a bit contra las cuentas por
//! longitud (`cuentas[len]`) y la lista de simbolos ordenada: quince pasos como
//! mucho por simbolo, sin tabla de saltos. Es lento para un `gzip` de un
//! gigabyte y sobra para una imagen de 1024 de lado, y es la version que se
//! puede leer entera y creer.
//!
//! ## La ventana es de quien llama
//!
//! Las referencias hacia atras llegan hasta 32 KiB; ese anillo NO vive en la
//! pila (Ring 3 tiene 64 KiB de pila y esto corre dentro del escritorio):
//! lo trae el `taller` de quien llama. Cada byte que sale se entrega a un
//! `pozo` y se guarda en el anillo, y nada mas: el descompresor no conoce el
//! tamano de la salida.

/// Bytes de ventana que pide un flujo DEFLATE.
pub const VENTANA: usize = 32 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fallo {
    /// Se acabaron los bytes antes de acabar el flujo.
    Corto,
    /// Un bloque, un codigo o una distancia que el formato no permite.
    Malformado,
    /// El pozo dijo que no cabe mas.
    NoCabe,
}

/// El flujo de entrada, que puede venir en TROZOS (los `IDAT` de un PNG) sin
/// pegarlos: se lee bit a bit saltando de trozo en trozo.
struct Bits<'a> {
    trozos: &'a [&'a [u8]],
    trozo: usize,
    pos: usize,
    acum: u32,
    cuantos: u32,
}

impl<'a> Bits<'a> {
    fn byte(&mut self) -> Result<u8, Fallo> {
        loop {
            let t = self.trozos.get(self.trozo).ok_or(Fallo::Corto)?;
            if self.pos < t.len() {
                let b = t[self.pos];
                self.pos += 1;
                return Ok(b);
            }
            self.trozo += 1;
            self.pos = 0;
        }
    }

    fn bits(&mut self, n: u32) -> Result<u32, Fallo> {
        while self.cuantos < n {
            let b = self.byte()? as u32;
            self.acum |= b << self.cuantos;
            self.cuantos += 8;
        }
        let v = self.acum & ((1u32 << n) - 1);
        self.acum >>= n;
        self.cuantos -= n;
        Ok(v)
    }

    /// Tira lo que quede del byte en curso (un bloque `stored` empieza en byte).
    fn alinea(&mut self) {
        self.acum = 0;
        self.cuantos = 0;
    }
}

/// Una tabla canonica: cuantos codigos de cada longitud, y los simbolos en
/// orden de codigo.
struct Huffman {
    cuentas: [u16; 16],
    simbolos: [u16; 288],
}

impl Huffman {
    /// Construye la tabla desde las longitudes de cada simbolo. Devuelve
    /// `Malformado` si el conjunto esta sobresuscrito.
    fn de(longitudes: &[u8]) -> Result<Self, Fallo> {
        let mut h = Huffman { cuentas: [0; 16], simbolos: [0; 288] };
        for &l in longitudes {
            h.cuentas[l as usize] += 1;
        }
        if h.cuentas[0] as usize == longitudes.len() {
            // Ningun codigo: tabla vacia, valida para distancias sin usar.
            return Ok(h);
        }
        let mut sobra: i32 = 1;
        for len in 1..16 {
            sobra <<= 1;
            sobra -= h.cuentas[len] as i32;
            if sobra < 0 {
                return Err(Fallo::Malformado);
            }
        }
        let mut desplaza = [0u16; 16];
        for len in 1..15 {
            desplaza[len + 1] = desplaza[len] + h.cuentas[len];
        }
        for (s, &l) in longitudes.iter().enumerate() {
            if l != 0 {
                h.simbolos[desplaza[l as usize] as usize] = s as u16;
                desplaza[l as usize] += 1;
            }
        }
        Ok(h)
    }

    fn simbolo(&self, bits: &mut Bits<'_>) -> Result<u16, Fallo> {
        let mut codigo: i32 = 0;
        let mut primero: i32 = 0;
        let mut indice: i32 = 0;
        for len in 1..16 {
            codigo |= bits.bits(1)? as i32;
            let cuenta = self.cuentas[len] as i32;
            if codigo - cuenta < primero {
                return Ok(self.simbolos[(indice + (codigo - primero)) as usize]);
            }
            indice += cuenta;
            primero += cuenta;
            primero <<= 1;
            codigo <<= 1;
        }
        Err(Fallo::Malformado)
    }
}

const LONG_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LONG_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// El orden en que llegan las longitudes de los codigos de longitud.
const ORDEN: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// El anillo de salida y el sitio donde se entrega cada byte.
struct Salida<'a, 'p> {
    ventana: &'a mut [u8],
    pos: usize,
    total: usize,
    pozo: &'p mut dyn FnMut(u8) -> Result<(), Fallo>,
}

impl Salida<'_, '_> {
    fn emite(&mut self, b: u8) -> Result<(), Fallo> {
        (self.pozo)(b)?;
        self.ventana[self.pos] = b;
        self.pos = (self.pos + 1) % VENTANA;
        self.total += 1;
        Ok(())
    }

    fn copia(&mut self, distancia: usize, cuantos: usize) -> Result<(), Fallo> {
        // *** LA DISTANCIA NO PUEDE APUNTAR ANTES DEL PRINCIPIO. Es la lectura
        // fuera de rango clasica de un inflate a mano.
        if distancia == 0 || distancia > VENTANA || distancia > self.total {
            return Err(Fallo::Malformado);
        }
        for _ in 0..cuantos {
            let b = self.ventana[(self.pos + VENTANA - distancia) % VENTANA];
            self.emite(b)?;
        }
        Ok(())
    }
}

/// **Descomprime un flujo DEFLATE crudo** (sin la cabecera zlib) que llega en
/// `trozos`, entregando cada byte a `pozo`. `ventana` tiene que medir
/// [`VENTANA`] bytes.
pub fn inflar(
    trozos: &[&[u8]],
    ventana: &mut [u8],
    pozo: &mut dyn FnMut(u8) -> Result<(), Fallo>,
) -> Result<usize, Fallo> {
    if ventana.len() < VENTANA {
        return Err(Fallo::NoCabe);
    }
    let mut bits = Bits { trozos, trozo: 0, pos: 0, acum: 0, cuantos: 0 };
    let mut salida = Salida { ventana: &mut ventana[..VENTANA], pos: 0, total: 0, pozo };
    loop {
        let ultimo = bits.bits(1)? == 1;
        match bits.bits(2)? {
            0 => almacenado(&mut bits, &mut salida)?,
            1 => {
                let (lit, dist) = fijas()?;
                bloque(&mut bits, &mut salida, &lit, &dist)?;
            }
            2 => {
                let (lit, dist) = dinamicas(&mut bits)?;
                bloque(&mut bits, &mut salida, &lit, &dist)?;
            }
            _ => return Err(Fallo::Malformado),
        }
        if ultimo {
            return Ok(salida.total);
        }
    }
}

fn almacenado(bits: &mut Bits<'_>, salida: &mut Salida<'_, '_>) -> Result<(), Fallo> {
    bits.alinea();
    let len = bits.byte()? as u16 | ((bits.byte()? as u16) << 8);
    let nlen = bits.byte()? as u16 | ((bits.byte()? as u16) << 8);
    if len != !nlen {
        return Err(Fallo::Malformado);
    }
    for _ in 0..len {
        let b = bits.byte()?;
        salida.emite(b)?;
    }
    Ok(())
}

fn fijas() -> Result<(Huffman, Huffman), Fallo> {
    let mut l = [0u8; 288];
    for (i, x) in l.iter_mut().enumerate() {
        *x = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    let d = [5u8; 30];
    Ok((Huffman::de(&l)?, Huffman::de(&d)?))
}

fn dinamicas(bits: &mut Bits<'_>) -> Result<(Huffman, Huffman), Fallo> {
    let nlen = bits.bits(5)? as usize + 257;
    let ndist = bits.bits(5)? as usize + 1;
    let ncode = bits.bits(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err(Fallo::Malformado);
    }
    let mut longitudes = [0u8; 320];
    for &o in ORDEN.iter().take(ncode) {
        longitudes[o] = bits.bits(3)? as u8;
    }
    let codigos = Huffman::de(&longitudes[..19])?;
    let mut i = 0;
    let mut l = [0u8; 320];
    while i < nlen + ndist {
        let s = codigos.simbolo(bits)?;
        match s {
            0..=15 => {
                l[i] = s as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return Err(Fallo::Malformado);
                }
                let anterior = l[i - 1];
                let n = 3 + bits.bits(2)? as usize;
                if i + n > nlen + ndist {
                    return Err(Fallo::Malformado);
                }
                for _ in 0..n {
                    l[i] = anterior;
                    i += 1;
                }
            }
            17 | 18 => {
                let n = if s == 17 { 3 + bits.bits(3)? as usize } else { 11 + bits.bits(7)? as usize };
                if i + n > nlen + ndist {
                    return Err(Fallo::Malformado);
                }
                i += n; // ya son cero
            }
            _ => return Err(Fallo::Malformado),
        }
    }
    // El fin de bloque (256) tiene que tener codigo.
    if l[256] == 0 {
        return Err(Fallo::Malformado);
    }
    Ok((Huffman::de(&l[..nlen])?, Huffman::de(&l[nlen..nlen + ndist])?))
}

fn bloque(
    bits: &mut Bits<'_>,
    salida: &mut Salida<'_, '_>,
    lit: &Huffman,
    dist: &Huffman,
) -> Result<(), Fallo> {
    loop {
        let s = lit.simbolo(bits)? as usize;
        if s < 256 {
            salida.emite(s as u8)?;
        } else if s == 256 {
            return Ok(());
        } else {
            let k = s - 257;
            if k >= 29 {
                return Err(Fallo::Malformado);
            }
            let largo = LONG_BASE[k] as usize + bits.bits(LONG_EXTRA[k] as u32)? as usize;
            let d = dist.simbolo(bits)? as usize;
            if d >= 30 {
                return Err(Fallo::Malformado);
            }
            let distancia = DIST_BASE[d] as usize + bits.bits(DIST_EXTRA[d] as u32)? as usize;
            salida.copia(distancia, largo)?;
        }
    }
}

/// **Un flujo zlib** (RFC 1950): dos bytes de cabecera, DEFLATE, y Adler-32
/// que aqui NO se comprueba (el PNG ya trae CRC por chunk y la imagen se
/// juzga por sus medidas). Devuelve cuantos bytes salieron.
pub fn inflar_zlib(
    trozos: &[&[u8]],
    ventana: &mut [u8],
    pozo: &mut dyn FnMut(u8) -> Result<(), Fallo>,
) -> Result<usize, Fallo> {
    // La cabecera zlib puede caer en el primer trozo (siempre, en un PNG
    // sano: un IDAT de dos bytes seria raro, pero se admite recortando).
    let mut cab = [0u8; 2];
    let mut leidos = 0;
    let mut t = 0;
    let mut p = 0;
    while leidos < 2 {
        let tr = trozos.get(t).ok_or(Fallo::Corto)?;
        if p < tr.len() {
            cab[leidos] = tr[p];
            leidos += 1;
            p += 1;
        } else {
            t += 1;
            p = 0;
        }
    }
    if cab[0] & 0x0F != 8 || (cab[0] as u16 * 256 + cab[1] as u16) % 31 != 0 || cab[1] & 0x20 != 0 {
        return Err(Fallo::Malformado);
    }
    // Lo que queda: el primer trozo sin sus bytes de cabecera y los demas.
    let mut recortados: [&[u8]; 64] = [&[]; 64];
    if trozos.len() > 64 {
        return Err(Fallo::NoCabe);
    }
    let mut n = 0;
    for (i, tr) in trozos.iter().enumerate() {
        if i < t {
            continue;
        }
        recortados[n] = if i == t { &tr[p..] } else { tr };
        n += 1;
    }
    inflar(&recortados[..n], ventana, pozo)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Los flujos los escribio el zlib del anfitrion (`pruebas/*.z`,
    // generados con Python): la prueba es contra OTRA implementacion, no
    // contra la idea que este fichero tiene del formato.
    const HOLA: &[u8] = include_bytes!("../pruebas/hola.z");
    const VACIO: &[u8] = include_bytes!("../pruebas/vacio.z");
    const ABC: &[u8] = include_bytes!("../pruebas/abc.z");
    const DINAMICO: &[u8] = include_bytes!("../pruebas/dinamico.z");

    fn todo(trozos: &[&[u8]]) -> Result<Vec<u8>, Fallo> {
        let mut v = Vec::new();
        let mut ventana = vec![0u8; VENTANA];
        inflar_zlib(trozos, &mut ventana, &mut |b| {
            v.push(b);
            Ok(())
        })?;
        Ok(v)
    }

    /// Codigos FIJOS con una copia hacia atras.
    #[test]
    fn un_flujo_con_codigos_fijos_y_una_copia() {
        assert_eq!(todo(&[HOLA]).unwrap(), b"hola hola hola hola");
    }

    /// El mismo flujo partido en tres trozos, como los IDAT de un PNG.
    #[test]
    fn los_trozos_no_cambian_nada() {
        assert_eq!(todo(&[&HOLA[..1], &HOLA[1..7], &HOLA[7..]]).unwrap(), b"hola hola hola hola");
    }

    /// Bloques ALMACENADOS (nivel 0 de zlib): vacio y con tres bytes.
    #[test]
    fn un_bloque_almacenado() {
        assert_eq!(todo(&[VACIO]).unwrap(), b"");
        assert_eq!(todo(&[ABC]).unwrap(), b"abc");
    }

    /// Codigos DINAMICOS, con copias largas y a distancia grande.
    #[test]
    fn un_flujo_con_codigos_dinamicos() {
        let mut esperado: Vec<u8> = Vec::new();
        for _ in 0..8 {
            esperado.extend(0u8..=255);
        }
        for _ in 0..300 {
            esperado.extend_from_slice(b"BMO-X ");
        }
        assert_eq!(todo(&[DINAMICO]).unwrap(), esperado);
    }

    #[test]
    fn un_flujo_cortado_es_corto_y_no_una_lectura_fuera() {
        // Hasta los 4 bytes del Adler-32, que no se comprueban: sin ellos
        // el flujo DEFLATE ya esta entero.
        for n in 1..DINAMICO.len() - 4 {
            match todo(&[&DINAMICO[..n]]) {
                Err(Fallo::Corto) | Err(Fallo::Malformado) => {}
                otro => panic!("con {n} bytes: {otro:?}"),
            }
        }
    }

    #[test]
    fn un_byte_cambiado_no_revienta() {
        // Cada byte del flujo, invertido: sale otra cosa, un error, o el
        // mismo texto (si el bit no importaba), pero nunca un panico.
        for i in 2..HOLA.len() {
            let mut z = HOLA.to_vec();
            z[i] ^= 0xFF;
            let _ = todo(&[&z]);
        }
    }

    #[test]
    fn una_cabecera_que_no_es_zlib_no_pasa() {
        assert_eq!(todo(&[&[0x12, 0x34, 0x00]]), Err(Fallo::Malformado));
        assert_eq!(todo(&[&[0x78]]), Err(Fallo::Corto));
    }

    #[test]
    fn el_pozo_puede_decir_que_no() {
        let mut ventana = vec![0u8; VENTANA];
        let mut n = 0;
        let r = inflar_zlib(&[HOLA], &mut ventana, &mut |_b| {
            n += 1;
            if n > 4 { Err(Fallo::NoCabe) } else { Ok(()) }
        });
        assert_eq!(r, Err(Fallo::NoCabe));
    }
}
