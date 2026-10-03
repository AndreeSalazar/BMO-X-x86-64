//! **El formato de `printf`**, con la convencion de BMO para los argumentos.
//!
//! Los argumentos NO llegan como una lista variadica del ABI de C: el frontend
//! los deja en un ARREGLO de palabras de 64 bits y pasa `(puntero, cuantos)`.
//! Un entero va tal cual; un `double`, con sus bits (`f64::to_bits`); una
//! cadena o un puntero, su direccion. Leer de mas no lee basura: lo que falta
//! vale 0.
//!
//! ```text
//!    banderas    -  0  +  espacio  #
//!    ancho       N  o  *           (del arreglo)
//!    precision   .N o .*           (%.8s corta una cadena: DOOM la usa)
//!    largo       hh h l ll z j t   (h y hh recortan; los demas, 64 bits)
//!    tipos       d i u x X o c s p f F e g %
//! ```
//!
//! [!] `%e` y `%g` salen como `%f`: no hay notacion cientifica. Y `%f` cuenta
//! con lo que da un `double` sin mas: pasado 2^64 sale `inf`.
//!
//! Los que escriben son `printf`/`fprintf` (a un fichero o a la consola) y
//! `snprintf` (a un bufer, cortando y contando lo que habria salido). Ninguno
//! tiene tope de largo: se escribe por trozos.

/// Donde va lo formateado.
pub trait Salida {
    fn poner(&mut self, b: u8);

    fn poner_todo(&mut self, s: &[u8]) {
        for &b in s {
            self.poner(b);
        }
    }
}

/// A un bufer, como `snprintf`: corta a `dst.len() - 1`, cierra con un cero y
/// cuenta lo que habria salido entero.
pub struct Bufer<'a> {
    pub dst: &'a mut [u8],
    pub total: usize,
}

impl Salida for Bufer<'_> {
    fn poner(&mut self, b: u8) {
        if self.total + 1 < self.dst.len() {
            self.dst[self.total] = b;
        }
        self.total += 1;
    }
}

impl Bufer<'_> {
    /// Poner el cero final. Devuelve lo que habria salido entero.
    pub fn cerrar(self) -> usize {
        if !self.dst.is_empty() {
            let fin = self.total.min(self.dst.len() - 1);
            self.dst[fin] = 0;
        }
        self.total
    }
}

/// Los argumentos: el arreglo y por donde va.
pub struct Args {
    ptr: *const u64,
    n: usize,
    i: usize,
}

impl Args {
    /// # Safety
    /// `ptr` apunta a `n` palabras legibles (o `n == 0`).
    pub unsafe fn new(ptr: *const u64, n: usize) -> Self {
        Self { ptr, n: if ptr.is_null() { 0 } else { n }, i: 0 }
    }

    pub fn de(v: &[u64]) -> Self {
        Self { ptr: v.as_ptr(), n: v.len(), i: 0 }
    }

    fn siguiente(&mut self) -> u64 {
        if self.i >= self.n {
            return 0;
        }
        let v = unsafe { *self.ptr.add(self.i) };
        self.i += 1;
        v
    }
}

#[derive(Clone, Copy, Default)]
struct Spec {
    izquierda: bool,
    ceros: bool,
    mas: bool,
    espacio: bool,
    almohadilla: bool,
    ancho: usize,
    precision: Option<usize>,
    largo: u8, // 0 normal, 1 h, 2 hh
}

/// Relleno + cuerpo, respetando el ancho y `-`/`0`.
fn con_ancho(s: &mut impl Salida, sp: &Spec, signo: &[u8], cuerpo: &[u8], numero: bool) {
    let largo = signo.len() + cuerpo.len();
    let falta = sp.ancho.saturating_sub(largo);
    if sp.izquierda {
        s.poner_todo(signo);
        s.poner_todo(cuerpo);
        (0..falta).for_each(|_| s.poner(b' '));
    } else if sp.ceros && numero {
        s.poner_todo(signo);
        (0..falta).for_each(|_| s.poner(b'0'));
        s.poner_todo(cuerpo);
    } else {
        (0..falta).for_each(|_| s.poner(b' '));
        s.poner_todo(signo);
        s.poner_todo(cuerpo);
    }
}

/// Las cifras de `v` en `base`, al final de `t`. Devuelve donde empiezan.
fn cifras(mut v: u64, base: u64, mayus: bool, t: &mut [u8; 24]) -> usize {
    let mut i = t.len();
    loop {
        i -= 1;
        let d = (v % base) as u8;
        t[i] = match d {
            0..=9 => b'0' + d,
            _ if mayus => b'A' + d - 10,
            _ => b'a' + d - 10,
        };
        v /= base;
        if v == 0 {
            return i;
        }
    }
}

fn entero(s: &mut impl Salida, sp: &Spec, v: u64, negativo: bool, base: u64, mayus: bool, prefijo: &[u8]) {
    let mut t = [0u8; 24];
    let i = cifras(v, base, mayus, &mut t);
    let mut cuerpo = [0u8; 48];
    let mut k = 0;
    // La precision de un entero es el minimo de cifras: rellena con ceros, y
    // `%.0d` de un 0 no escribe nada.
    let n_cifras = t.len() - i;
    let minimo = sp.precision.unwrap_or(1);
    if !(minimo == 0 && v == 0) {
        for _ in n_cifras..minimo.min(40) {
            cuerpo[k] = b'0';
            k += 1;
        }
        cuerpo[k..k + n_cifras].copy_from_slice(&t[i..]);
        k += n_cifras;
    }
    let mut signo = [0u8; 3];
    let mut ks = 0;
    if negativo {
        signo[0] = b'-';
        ks = 1;
    } else if sp.mas {
        signo[0] = b'+';
        ks = 1;
    } else if sp.espacio {
        signo[0] = b' ';
        ks = 1;
    }
    if v != 0 {
        signo[ks..ks + prefijo.len()].copy_from_slice(prefijo);
        ks += prefijo.len();
    }
    let mut sp2 = *sp;
    if sp.precision.is_some() {
        sp2.ceros = false;
    }
    con_ancho(s, &sp2, &signo[..ks], &cuerpo[..k], true);
}

fn flotante(s: &mut impl Salida, sp: &Spec, v: f64) {
    let prec = sp.precision.unwrap_or(6).min(17);
    let negativo = v < 0.0 || (v == 0.0 && v.to_bits() >> 63 != 0);
    let signo: &[u8] = if negativo {
        b"-"
    } else if sp.mas {
        b"+"
    } else if sp.espacio {
        b" "
    } else {
        b""
    };
    if v.is_nan() {
        return con_ancho(s, sp, b"", b"nan", false);
    }
    let mut x = if negativo { -v } else { v };
    // Redondear a `prec` decimales.
    let mut medio = 0.5;
    for _ in 0..prec {
        medio /= 10.0;
    }
    x += medio;
    if x.is_infinite() || x >= 18_446_744_073_709_551_616.0 {
        return con_ancho(s, sp, signo, b"inf", false);
    }
    let ent = x as u64;
    let mut frac = x - ent as f64;
    let mut cuerpo = [0u8; 48];
    let mut t = [0u8; 24];
    let i = cifras(ent, 10, false, &mut t);
    let mut k = t.len() - i;
    cuerpo[..k].copy_from_slice(&t[i..]);
    if prec > 0 || sp.almohadilla {
        cuerpo[k] = b'.';
        k += 1;
    }
    for _ in 0..prec {
        frac *= 10.0;
        let d = (frac as u8).min(9);
        cuerpo[k] = b'0' + d;
        k += 1;
        frac -= d as f64;
    }
    con_ancho(s, sp, signo, &cuerpo[..k], true);
}

/// **Formatear** `fmt` (hasta su cero o su final) con `args` en `s`.
pub fn formatear(s: &mut impl Salida, fmt: &[u8], args: &mut Args) {
    let mut i = 0;
    let en = |i: usize| fmt.get(i).copied().unwrap_or(0);
    while i < fmt.len() && fmt[i] != 0 {
        let c = fmt[i];
        i += 1;
        if c != b'%' {
            s.poner(c);
            continue;
        }
        let mut sp = Spec::default();
        loop {
            match en(i) {
                b'-' => sp.izquierda = true,
                b'0' => sp.ceros = true,
                b'+' => sp.mas = true,
                b' ' => sp.espacio = true,
                b'#' => sp.almohadilla = true,
                _ => break,
            }
            i += 1;
        }
        if en(i) == b'*' {
            let w = args.siguiente() as i64 as i32;
            if w < 0 {
                sp.izquierda = true;
            }
            sp.ancho = w.unsigned_abs() as usize;
            i += 1;
        } else {
            while en(i).is_ascii_digit() {
                sp.ancho = (sp.ancho * 10 + (en(i) - b'0') as usize).min(4096);
                i += 1;
            }
        }
        if en(i) == b'.' {
            i += 1;
            let mut p = 0usize;
            if en(i) == b'*' {
                let v = args.siguiente() as i64 as i32;
                p = v.max(0) as usize;
                i += 1;
            } else {
                while en(i).is_ascii_digit() {
                    p = (p * 10 + (en(i) - b'0') as usize).min(4096);
                    i += 1;
                }
            }
            sp.precision = Some(p);
        }
        loop {
            match en(i) {
                b'h' => sp.largo += 1,
                b'l' | b'z' | b'j' | b't' | b'L' | b'q' => {}
                _ => break,
            }
            i += 1;
        }
        let tipo = en(i);
        if tipo == 0 {
            break;
        }
        i += 1;
        let recorte = |v: u64, sp: &Spec| match sp.largo {
            0 => v,
            1 => v as u16 as u64,
            _ => v as u8 as u64,
        };
        match tipo {
            b'd' | b'i' => {
                let crudo = args.siguiente();
                let v = match sp.largo {
                    0 => crudo as i64,
                    1 => crudo as i16 as i64,
                    _ => crudo as i8 as i64,
                };
                entero(s, &sp, v.unsigned_abs(), v < 0, 10, false, b"");
            }
            b'u' => {
                let v = recorte(args.siguiente(), &sp);
                entero(s, &sp, v, false, 10, false, b"");
            }
            b'x' | b'X' => {
                let v = recorte(args.siguiente(), &sp);
                let pre: &[u8] = match (sp.almohadilla, tipo) {
                    (true, b'x') => b"0x",
                    (true, _) => b"0X",
                    _ => b"",
                };
                entero(s, &sp, v, false, 16, tipo == b'X', pre);
            }
            b'o' => {
                let v = recorte(args.siguiente(), &sp);
                entero(s, &sp, v, false, 8, false, if sp.almohadilla { b"0" } else { b"" });
            }
            b'p' => {
                // Un puntero es un numero: el nulo sale `0x0`, no un "(nil)".
                let v = args.siguiente();
                let mut sp2 = sp;
                sp2.precision = None;
                if v == 0 {
                    con_ancho(s, &sp2, b"", b"0x0", false);
                } else {
                    entero(s, &sp2, v, false, 16, false, b"0x");
                }
            }
            b'c' => {
                let v = args.siguiente() as u8;
                con_ancho(s, &sp, b"", &[v], false);
            }
            b's' => {
                let p = args.siguiente() as *const u8;
                let tope = sp.precision.unwrap_or(usize::MAX);
                if p.is_null() {
                    let nulo: &[u8] = b"(nulo)";
                    con_ancho(s, &sp, b"", &nulo[..nulo.len().min(tope)], false);
                } else {
                    let mut n = 0usize;
                    while n < tope && unsafe { *p.add(n) } != 0 {
                        n += 1;
                    }
                    let cad = unsafe { core::slice::from_raw_parts(p, n) };
                    con_ancho(s, &sp, b"", cad, false);
                }
            }
            b'f' | b'F' | b'e' | b'E' | b'g' | b'G' => {
                flotante(s, &sp, f64::from_bits(args.siguiente()));
            }
            b'%' => s.poner(b'%'),
            otro => {
                s.poner(b'%');
                s.poner(otro);
            }
        }
    }
}

/// Lo que mide una cadena C, hasta su cero.
///
/// # Safety
/// `p` es una cadena C valida.
pub unsafe fn cadena<'a>(p: *const u8) -> &'a [u8] {
    if p.is_null() {
        return &[];
    }
    core::slice::from_raw_parts(p, crate::string::largo(p))
}

/// `snprintf(buf, cap, fmt, ...)` con la convencion de BMO. Devuelve lo que
/// habria salido entero (sin el cero), como C.
///
/// # Safety
/// `buf` tiene `cap` bytes; `fmt` es una cadena C; `args` tiene `n` palabras.
#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "C" fn bmo_snprintf(buf: *mut u8, cap: usize, fmt: *const u8, n: u64, args: *const u64) -> i32 {
    let dst: &mut [u8] = if buf.is_null() { &mut [] } else { core::slice::from_raw_parts_mut(buf, cap) };
    let mut b = Bufer { dst, total: 0 };
    formatear(&mut b, cadena(fmt), &mut Args::new(args, n as usize));
    b.cerrar() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(fmt: &str, args: &[u64]) -> std::string::String {
        let mut dst = [0u8; 256];
        let mut b = Bufer { dst: &mut dst, total: 0 };
        formatear(&mut b, fmt.as_bytes(), &mut Args::de(args));
        let n = b.cerrar();
        std::string::String::from_utf8_lossy(&dst[..n]).into_owned()
    }

    #[test]
    fn enteros() {
        assert_eq!(f("%d|%i|%u", &[(-42i64) as u64, 7, 3_000_000_000]), "-42|7|3000000000");
        assert_eq!(f("%x %X %#x %o", &[255, 255, 255, 8]), "ff FF 0xff 10");
        assert_eq!(f("%ld %lld %zu", &[1, 2, 3]), "1 2 3");
        assert_eq!(f("%hhu %hd", &[0x1FF, 0x18000]), "255 -32768");
    }

    #[test]
    fn ancho_y_relleno() {
        assert_eq!(f("[%5d][%-5d][%05d]", &[42, 42, 42]), "[   42][42   ][00042]");
        assert_eq!(f("[%02d:%02d]", &[3, 7]), "[03:07]");
        assert_eq!(f("[%+d][% d][%05d]", &[5, 5, (-5i64) as u64]), "[+5][ 5][-0005]");
        assert_eq!(f("[%*d][%-*d]", &[4, 1, 3, 2]), "[   1][2  ]");
        assert_eq!(f("[%.3d][%5.3d]", &[7, 7]), "[007][  007]");
    }

    #[test]
    fn cadenas_y_caracteres() {
        let s = b"E1M1\0";
        let lump = b"PLAYPALXYZ\0";
        assert_eq!(f("%s %c%c", &[s.as_ptr() as u64, b'o' as u64, b'k' as u64]), "E1M1 ok");
        // `%.8s`: el nombre de un lump de DOOM no tiene por que acabar en cero.
        assert_eq!(f("[%.8s]", &[lump.as_ptr() as u64]), "[PLAYPALX]");
        assert_eq!(f("[%-6s][%6s]", &[s.as_ptr() as u64, s.as_ptr() as u64]), "[E1M1  ][  E1M1]");
        assert_eq!(f("%s", &[0]), "(nulo)");
    }

    #[test]
    fn punteros_y_porcentaje() {
        assert_eq!(f("%p %p 100%%", &[0x40001000, 0]), "0x40001000 0x0 100%");
    }

    #[test]
    fn flotantes() {
        assert_eq!(f("%f", &[1.5f64.to_bits()]), "1.500000");
        assert_eq!(f("%.2f", &[3.14159f64.to_bits()]), "3.14");
        assert_eq!(f("%.0f", &[2.5f64.to_bits()]), "3");
        assert_eq!(f("%8.3f|%-8.1f|", &[(-0.5f64).to_bits(), 9.96f64.to_bits()]), "  -0.500|10.0    |");
        assert_eq!(f("%f", &[f64::NAN.to_bits()]), "nan");
        assert_eq!(f("%f", &[f64::INFINITY.to_bits()]), "inf");
    }

    #[test]
    fn faltan_argumentos_y_se_corta() {
        assert_eq!(f("%d %d", &[1]), "1 0");
        let mut dst = [0u8; 4];
        let mut b = Bufer { dst: &mut dst, total: 0 };
        formatear(&mut b, b"abcdef", &mut Args::de(&[]));
        assert_eq!(b.cerrar(), 6);
        assert_eq!(&dst, b"abc\0");
    }
}
