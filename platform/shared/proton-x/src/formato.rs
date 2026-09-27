//! **El `printf` de C** (P4f5, 27-09): lo que hace `__stdio_common_vfprintf`
//! del UCRT con su cadena de formato, sin punteros.
//!
//! ```text
//!    %[banderas][ancho][.precision][largo]conversion
//!    banderas    -  +  espacio  0  #
//!    ancho, .p   un numero o `*` (se saca de los argumentos)
//!    largo       hh h l ll L z j t w  I  I32  I64   (lo de Microsoft tambien)
//!    conversion  d i u o x X c s p e E f F g G %   (y S, C: el ancho "otro")
//! ```
//!
//! Los argumentos los da quien llama por [`Argumentos`]: en Windows x64 son
//! ranuras de 8 bytes (un `double` tambien), y una cadena es un puntero que
//! solo quien tiene la memoria sabe leer.
//!
//! Los `double` se escriben con las cuentas de `core` (enteras: valen igual
//! con soft-float en Ring 3). Lo que no esta, dicho: `%a` (hexadecimal de
//! coma flotante) y `%n` (el UCRT lo tiene APAGADO por seguridad) salen tal
//! cual, sin consumir argumento; y la configuracion regional es la de "C"
//! (el punto decimal es un punto).

use alloc::string::String;
use alloc::vec::Vec;

/// **De donde salen los argumentos**, uno detras de otro.
pub trait Argumentos {
    /// La siguiente ranura, entera (tambien un puntero).
    fn entero(&mut self) -> u64;
    /// La siguiente ranura, como `double`.
    fn doble(&mut self) -> f64 {
        f64::from_bits(self.entero())
    }
    /// Una cadena de bytes por su puntero (sin el 0).
    fn cadena(&mut self, p: u64) -> Vec<u8>;
    /// Una cadena UTF-16 por su puntero, ya en UTF-8.
    fn cadena_ancha(&mut self, p: u64) -> Vec<u8>;
}

#[derive(Default, Clone, Copy)]
struct Spec {
    izquierda: bool,
    signo: bool,
    espacio: bool,
    ceros: bool,
    alterna: bool,
    ancho: usize,
    precision: Option<usize>,
    /// 1 hh, 2 h, 4 normal, 8 l/ll/z/j/t/I64/I; -1 L.
    largo: i8,
    /// `l` o `w`: el caracter/la cadena "anchos".
    ancho_car: bool,
    /// `h`: el caracter/la cadena "estrechos".
    estrecho_car: bool,
}

/// Rellenar a `ancho`, a la izquierda o a la derecha.
fn poner(salida: &mut Vec<u8>, cuerpo: &[u8], s: &Spec) {
    let falta = s.ancho.saturating_sub(cuerpo.len());
    if s.izquierda {
        salida.extend_from_slice(cuerpo);
        salida.extend(core::iter::repeat(b' ').take(falta));
    } else {
        salida.extend(core::iter::repeat(b' ').take(falta));
        salida.extend_from_slice(cuerpo);
    }
}

/// Un numero con su signo/prefijo delante y los ceros de relleno en medio.
fn poner_numero(salida: &mut Vec<u8>, delante: &[u8], digitos: &[u8], s: &Spec, ceros_validos: bool) {
    let mut cuerpo = Vec::from(delante);
    let largo = delante.len() + digitos.len();
    if s.ceros && !s.izquierda && ceros_validos && s.ancho > largo {
        cuerpo.extend(core::iter::repeat(b'0').take(s.ancho - largo));
    }
    cuerpo.extend_from_slice(digitos);
    poner(salida, &cuerpo, s);
}

fn en_base(mut v: u64, base: u64, mayus: bool) -> Vec<u8> {
    let cifras: &[u8] = if mayus { b"0123456789ABCDEF" } else { b"0123456789abcdef" };
    let mut d = Vec::new();
    loop {
        d.push(cifras[(v % base) as usize]);
        v /= base;
        if v == 0 {
            break;
        }
    }
    d.reverse();
    d
}

fn recortar(v: u64, largo: i8, con_signo: bool) -> (bool, u64) {
    match (largo, con_signo) {
        (1, true) => ((v as i8) < 0, (v as i8).unsigned_abs() as u64),
        (2, true) => ((v as i16) < 0, (v as i16).unsigned_abs() as u64),
        (4, true) => ((v as i32) < 0, (v as i32).unsigned_abs() as u64),
        (_, true) => ((v as i64) < 0, (v as i64).unsigned_abs()),
        (1, false) => (false, v as u8 as u64),
        (2, false) => (false, v as u16 as u64),
        (4, false) => (false, v as u32 as u64),
        (_, false) => (false, v),
    }
}

/// Digitos y exponente decimal de `x` (> 0) con `p` cifras tras la primera:
/// `d.ddd * 10^e`, redondeado como `core` (al par).
fn cientifica(x: f64, p: usize) -> (Vec<u8>, i32) {
    let s = alloc::format!("{:.*e}", p, x);
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    (m.bytes().filter(|c| c.is_ascii_digit()).collect(), e.parse().unwrap_or(0))
}

/// `%e`: `d.ddde+XX` (al menos dos cifras de exponente, como el UCRT).
fn formato_e(x: f64, p: usize, alterna: bool, mayus: bool) -> Vec<u8> {
    let (d, e) = if x == 0.0 { (alloc::vec![b'0'; p + 1], 0) } else { cientifica(x, p) };
    let mut o = Vec::from(&d[..1]);
    if p > 0 || alterna {
        o.push(b'.');
    }
    o.extend_from_slice(&d[1..]);
    o.push(if mayus { b'E' } else { b'e' });
    o.push(if e < 0 { b'-' } else { b'+' });
    let ex = en_base(e.unsigned_abs() as u64, 10, false);
    if ex.len() < 2 {
        o.push(b'0');
    }
    o.extend_from_slice(&ex);
    o
}

/// `%f`.
fn formato_f(x: f64, p: usize, alterna: bool) -> Vec<u8> {
    let mut o: Vec<u8> = alloc::format!("{:.*}", p, x).into_bytes();
    if p == 0 && alterna {
        o.push(b'.');
    }
    o
}

/// `%g`: la regla de C (P cifras; %e si el exponente es < -4 o >= P) y sin
/// los ceros del final salvo con `#`.
fn formato_g(x: f64, prec: usize, alterna: bool, mayus: bool) -> Vec<u8> {
    let p = if prec == 0 { 1 } else { prec };
    let e = if x == 0.0 { 0 } else { cientifica(x, p - 1).1 };
    let mut o = if e < -4 || e >= p as i32 { formato_e(x, p - 1, alterna, mayus) } else { formato_f(x, (p as i32 - 1 - e) as usize, alterna) };
    if !alterna {
        // Quitar los ceros de la parte decimal (antes del exponente, si lo hay).
        let fin_mantisa = o.iter().position(|&c| c == b'e' || c == b'E').unwrap_or(o.len());
        if o[..fin_mantisa].contains(&b'.') {
            let mut k = fin_mantisa;
            while k > 0 && o[k - 1] == b'0' {
                k -= 1;
            }
            if k > 0 && o[k - 1] == b'.' {
                k -= 1;
            }
            o.drain(k..fin_mantisa);
        }
    }
    o
}

fn coma_flotante(salida: &mut Vec<u8>, x: f64, conv: u8, s: &Spec) {
    let mayus = conv.is_ascii_uppercase();
    let negativo = x.is_sign_negative() && !(x == 0.0 && !x.is_sign_negative());
    let delante: &[u8] = if negativo {
        b"-"
    } else if s.signo {
        b"+"
    } else if s.espacio {
        b" "
    } else {
        b""
    };
    let a = x.abs();
    if !a.is_finite() {
        // Lo del UCRT: "inf" y "nan" (en mayusculas con E, F y G).
        let t: &[u8] = match (a.is_nan(), mayus) {
            (true, false) => b"nan",
            (true, true) => b"NAN",
            (false, false) => b"inf",
            (false, true) => b"INF",
        };
        poner_numero(salida, delante, t, s, false);
        return;
    }
    let p = s.precision.unwrap_or(6);
    let cuerpo = match conv {
        b'e' | b'E' => formato_e(a, p, s.alterna, mayus),
        b'f' | b'F' => formato_f(a, p, s.alterna),
        _ => formato_g(a, p, s.alterna, mayus),
    };
    poner_numero(salida, delante, &cuerpo, s, true);
}

/// **Formatear** `fmt` con `args`. `ancha`: la llamada es de la familia
/// `w` (wprintf): ahi `%s`/`%c` son anchos y `%S`/`%C` estrechos (lo que el
/// UCRT hace por omision, `_CRT_INTERNAL_PRINTF_LEGACY_WIDE_SPECIFIERS`).
pub fn formatear(fmt: &[u8], args: &mut impl Argumentos, ancha: bool) -> Vec<u8> {
    let mut o = Vec::with_capacity(fmt.len() + 16);
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c != b'%' {
            o.push(c);
            i += 1;
            continue;
        }
        let inicio = i;
        i += 1;
        let mut s = Spec { largo: 4, ..Spec::default() };
        while i < fmt.len() {
            match fmt[i] {
                b'-' => s.izquierda = true,
                b'+' => s.signo = true,
                b' ' => s.espacio = true,
                b'0' => s.ceros = true,
                b'#' => s.alterna = true,
                _ => break,
            }
            i += 1;
        }
        if fmt.get(i) == Some(&b'*') {
            let w = args.entero() as i32;
            if w < 0 {
                s.izquierda = true;
            }
            s.ancho = w.unsigned_abs() as usize;
            i += 1;
        } else {
            while let Some(d) = fmt.get(i).filter(|d| d.is_ascii_digit()) {
                s.ancho = s.ancho * 10 + (d - b'0') as usize;
                i += 1;
            }
        }
        if fmt.get(i) == Some(&b'.') {
            i += 1;
            if fmt.get(i) == Some(&b'*') {
                let p = args.entero() as i32;
                s.precision = (p >= 0).then_some(p as usize);
                i += 1;
            } else {
                let mut p = 0;
                while let Some(d) = fmt.get(i).filter(|d| d.is_ascii_digit()) {
                    p = p * 10 + (d - b'0') as usize;
                    i += 1;
                }
                s.precision = Some(p);
            }
        }
        // El largo.
        let resto = &fmt[i..];
        let (largo, k) = if resto.starts_with(b"hh") {
            (1, 2)
        } else if resto.starts_with(b"ll") || resto.starts_with(b"I64") {
            (8, if resto[0] == b'I' { 3 } else { 2 })
        } else if resto.starts_with(b"I32") {
            (4, 3)
        } else {
            match resto.first() {
                Some(b'h') => (2, 1),
                Some(b'l') | Some(b'w') => {
                    s.ancho_car = true;
                    (4, 1)
                }
                Some(b'z') | Some(b'j') | Some(b't') | Some(b'I') => (8, 1),
                Some(b'L') => (-1, 1),
                _ => (4, 0),
            }
        };
        if largo == 2 {
            s.estrecho_car = true;
        }
        s.largo = largo;
        i += k;
        let Some(&conv) = fmt.get(i) else {
            o.extend_from_slice(&fmt[inicio..]);
            break;
        };
        i += 1;
        match conv {
            b'%' => o.push(b'%'),
            b'd' | b'i' => {
                let (neg, v) = recortar(args.entero(), s.largo, true);
                let mut d = if s.precision == Some(0) && v == 0 { Vec::new() } else { en_base(v, 10, false) };
                if let Some(p) = s.precision {
                    while d.len() < p {
                        d.insert(0, b'0');
                    }
                }
                let delante: &[u8] = if neg {
                    b"-"
                } else if s.signo {
                    b"+"
                } else if s.espacio {
                    b" "
                } else {
                    b""
                };
                poner_numero(&mut o, delante, &d, &s, s.precision.is_none());
            }
            b'u' | b'o' | b'x' | b'X' => {
                let (_, v) = recortar(args.entero(), s.largo, false);
                let base = match conv {
                    b'u' => 10,
                    b'o' => 8,
                    _ => 16,
                };
                let mut d = if s.precision == Some(0) && v == 0 { Vec::new() } else { en_base(v, base, conv == b'X') };
                if let Some(p) = s.precision {
                    while d.len() < p {
                        d.insert(0, b'0');
                    }
                }
                let delante: &[u8] = match (conv, s.alterna && v != 0) {
                    (b'x', true) => b"0x",
                    (b'X', true) => b"0X",
                    (b'o', true) if d.first() != Some(&b'0') => b"0",
                    _ => b"",
                };
                poner_numero(&mut o, delante, &d, &s, s.precision.is_none());
            }
            b'p' => {
                // El UCRT: 16 cifras hexadecimales en mayuscula, sin 0x.
                let d = alloc::format!("{:016X}", args.entero());
                poner(&mut o, d.as_bytes(), &s);
            }
            b'c' | b'C' => {
                let v = args.entero();
                let es_ancho = if s.ancho_car { true } else if s.estrecho_car { false } else { (conv == b'c') == ancha };
                let mut b = [0u8; 4];
                let t: Vec<u8> = if es_ancho { Vec::from(char::from_u32(v as u16 as u32).unwrap_or('\u{FFFD}').encode_utf8(&mut b).as_bytes()) } else { alloc::vec![v as u8] };
                poner(&mut o, &t, &s);
            }
            b's' | b'S' => {
                let p = args.entero();
                let es_ancho = if s.ancho_car { true } else if s.estrecho_car { false } else { (conv == b's') == ancha };
                let mut t = if p == 0 { Vec::from(&b"(null)"[..]) } else if es_ancho { args.cadena_ancha(p) } else { args.cadena(p) };
                if let Some(pr) = s.precision {
                    // La precision cuenta caracteres, no bytes: no cortar a medias.
                    let mut k = 0;
                    let mut n = 0;
                    while k < t.len() && n < pr {
                        k += match t[k] {
                            0..=0x7F => 1,
                            0xC0..=0xDF => 2,
                            0xE0..=0xEF => 3,
                            _ => 4,
                        };
                        n += 1;
                    }
                    t.truncate(k.min(t.len()));
                }
                poner(&mut o, &t, &s);
            }
            b'e' | b'E' | b'f' | b'F' | b'g' | b'G' => {
                let x = args.doble();
                coma_flotante(&mut o, x, conv, &s);
            }
            // %a y %n: tal cual (ver arriba), y lo que no es una conversion.
            _ => o.extend_from_slice(&fmt[inicio..i]),
        }
    }
    o
}

/// Para el banco: formatear con argumentos dados en una lista.
pub struct Lista<'a> {
    pub ranuras: &'a [u64],
    pub cadenas: &'a [(u64, &'a str)],
    pub i: usize,
}

impl Argumentos for Lista<'_> {
    fn entero(&mut self) -> u64 {
        let v = self.ranuras.get(self.i).copied().unwrap_or(0);
        self.i += 1;
        v
    }
    fn cadena(&mut self, p: u64) -> Vec<u8> {
        self.cadenas.iter().find(|c| c.0 == p).map(|c| Vec::from(c.1.as_bytes())).unwrap_or_default()
    }
    fn cadena_ancha(&mut self, p: u64) -> Vec<u8> {
        self.cadena(p)
    }
}

/// Un `String` del banco, por comodidad.
pub fn a_texto(v: Vec<u8>) -> String {
    String::from_utf8(v).unwrap_or_default()
}
