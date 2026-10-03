//! **BMO BANK CAT** -- el WRAPPER del motor COBOL de BANK CAT (BC2 de
//! `docs/plan/PLAN_BANK_CAT.md`, 2026-10-03).
//!
//! generacion: nieto -- lineas de texto y centimos; no sabe de procesos ni de pantallas
//! capa: puro -- ni un `unsafe`, ni un aparato
//!
//! El propietario: *"una libreria o WRAPPER para simplificar por completo"*.
//! El dinero lo calcula el MOTOR, en COBOL (`toolchain/lang/cobol/examples/
//! 11-bankcat/libro.cob`, con la libreria `CABDATOS` + `CABLIBRO`); esto es lo
//! que la cara necesita para hablarle sin saber COBOL:
//!
//! ```text
//!    Orden::Pagar(1999)  ->  "3\n19.99\n"         escribir
//!    "0\n1240.03\n"      ->  Respuesta { Hecho, 124003 }   Charla::empujar
//!    124003              ->  "1.240,03"           formato
//! ```
//!
//! ** CENTIMOS ENTEROS en los dos lados: el COBOL los guarda en COMP-3 y
//! aqui son un `i64`. Ni un `f64` en todo el camino, ni al leer ni al
//! escribir: `19.99` se lee como `1999`, no como `19.989999...`.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// Dinero de BANK CAT, en centimos.
pub type Centimos = i64;

/// Lo mas que cabe en el saldo del motor: `PIC S9(13)V99`.
pub const TOPE: Centimos = 999_999_999_999_999;

/// Lo mas largo que es una linea del motor (un saldo con signo y su punto).
pub const LINEA_MAX: usize = 24;

/// **Lo que se le pide al motor.** Cada orden son DOS lineas: el codigo y el
/// importe (el motor lee siempre dos, para no desincronizarse).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orden {
    /// El saldo de partida.
    Abrir(Centimos),
    /// Entra dinero (al HABER), por las veces que se dijeron.
    Cobrar(Centimos),
    /// Sale dinero (al DEBE), por las veces que se dijeron.
    Pagar(Centimos),
    /// El siguiente cobro o pago va por `n` veces (`3 x 19.99`).
    Veces(u32),
    /// Inicial + haber - debe tiene que ser el saldo.
    Cuadrar,
    /// Cuadra y el motor acaba.
    Cerrar,
}

/// **Como fue**: el `CAB-ESTADO` de `CABDATOS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    Hecho,
    NoCabe,
    SinSaldo,
    ImporteMalo,
    Descuadre,
    /// Hecho en la memoria del motor, pero el libro NO se guardo en el
    /// disco (BC4).
    SinDisco,
}

impl Estado {
    pub fn de(c: u8) -> Option<Estado> {
        Some(match c {
            b'0' => Estado::Hecho,
            b'1' => Estado::NoCabe,
            b'2' => Estado::SinSaldo,
            b'3' => Estado::ImporteMalo,
            b'4' => Estado::Descuadre,
            b'5' => Estado::SinDisco,
            _ => return None,
        })
    }

    /// Lo que dice el gato.
    pub fn texto(self) -> &'static str {
        match self {
            Estado::Hecho => "hecho, y apuntado",
            Estado::NoCabe => "no cabe: pasaria del tope del libro",
            Estado::SinSaldo => "no hay saldo: el gato no fia",
            Estado::ImporteMalo => "ese importe no vale",
            Estado::Descuadre => "EL LIBRO NO CUADRA",
            Estado::SinDisco => "hecho, pero el libro NO se guardo en el disco",
        }
    }
}

/// **Una respuesta del motor**: como fue, y el saldo despues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Respuesta {
    pub estado: Estado,
    pub saldo: Centimos,
}

/// Por que una linea del motor no se entiende.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallo {
    /// Una linea de mas de [`LINEA_MAX`].
    Largo,
    /// Un estado que no es 0..4.
    Estado,
    /// Un saldo que no es un importe.
    Importe,
    /// No cabe en el bufer de quien escribe.
    Corto,
}

/// `1250.00`, `-1.5`, `7`: a centimos. Mas de dos decimales, o pasar del
/// tope, no es un importe.
pub fn leer_centimos(b: &[u8]) -> Option<Centimos> {
    let b = b.strip_suffix(b"\r").unwrap_or(b);
    let (signo, b) = match b.first() {
        Some(b'-') => (-1, &b[1..]),
        Some(b'+') => (1, &b[1..]),
        _ => (1, b),
    };
    let (entera, decimales) = match b.iter().position(|&c| c == b'.') {
        Some(i) => (&b[..i], &b[i + 1..]),
        None => (b, &b""[..]),
    };
    if entera.is_empty() || decimales.len() > 2 || !entera.iter().chain(decimales).all(u8::is_ascii_digit) {
        return None;
    }
    let mut n: Centimos = 0;
    for &c in entera {
        n = n.checked_mul(10)?.checked_add((c - b'0') as Centimos)?;
    }
    let mut cent = 0;
    for k in 0..2 {
        cent = cent * 10 + decimales.get(k).map_or(0, |&c| (c - b'0') as Centimos);
    }
    let v = n.checked_mul(100)?.checked_add(cent)?;
    (v <= TOPE).then_some(signo * v)
}

/// `124003` -> `1240.03` (lo que lee el `ACCEPT` del motor). Devuelve cuantos.
pub fn escribir_centimos(c: Centimos, dst: &mut [u8]) -> Result<usize, Fallo> {
    let mut tmp = [0u8; 24];
    let mut i = tmp.len();
    let mut v = c.unsigned_abs();
    for k in 0.. {
        if k == 2 {
            i -= 1;
            tmp[i] = b'.';
        }
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 && k >= 2 {
            break;
        }
    }
    if c < 0 {
        i -= 1;
        tmp[i] = b'-';
    }
    let n = tmp.len() - i;
    dst.get_mut(..n).ok_or(Fallo::Corto)?.copy_from_slice(&tmp[i..]);
    Ok(n)
}

/// **Escribe una orden** con sus dos `\n`. Devuelve cuantos bytes.
pub fn escribir(o: &Orden, dst: &mut [u8]) -> Result<usize, Fallo> {
    let (codigo, importe) = match *o {
        Orden::Abrir(c) => (b'1', c),
        Orden::Cobrar(c) => (b'2', c),
        Orden::Pagar(c) => (b'3', c),
        // Las veces viajan como importe entero: el motor las mueve a un PIC 9(5).
        Orden::Veces(n) => (b'4', n as Centimos * 100),
        Orden::Cuadrar => (b'5', 0),
        Orden::Cerrar => (b'9', 0),
    };
    let mut i = 0;
    let mut poner = |dst: &mut [u8], b: &[u8]| -> Result<(), Fallo> {
        dst.get_mut(i..i + b.len()).ok_or(Fallo::Corto)?.copy_from_slice(b);
        i += b.len();
        Ok(())
    };
    poner(dst, &[codigo, b'\n'])?;
    let mut num = [0u8; 24];
    let n = escribir_centimos(importe, &mut num)?;
    poner(dst, &num[..n])?;
    poner(dst, b"\n")?;
    Ok(i)
}

/// **Junta lo que dice el motor** en respuestas de dos lineas: un byte cada
/// vez, como llega de la consola (que da de siete en siete).
#[derive(Clone, Copy, Debug)]
pub struct Charla {
    linea: [u8; LINEA_MAX],
    n: usize,
    estado: Option<Estado>,
}

impl Default for Charla {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Charla {
    pub const fn nueva() -> Self {
        Self { linea: [0; LINEA_MAX], n: 0, estado: None }
    }

    /// Un byte mas. `Some` cuando hay una respuesta entera, o un fallo (y
    /// tras un fallo se empieza de cero: una respuesta rota no arrastra a la
    /// siguiente).
    pub fn empujar(&mut self, b: u8) -> Option<Result<Respuesta, Fallo>> {
        if b != b'\n' {
            if self.n == LINEA_MAX {
                *self = Self::nueva();
                return Some(Err(Fallo::Largo));
            }
            self.linea[self.n] = b;
            self.n += 1;
            return None;
        }
        let l = &self.linea[..self.n];
        let l = l.strip_suffix(b"\r").unwrap_or(l);
        let r = match self.estado {
            None => match l {
                [c] => match Estado::de(*c) {
                    Some(e) => {
                        self.estado = Some(e);
                        self.n = 0;
                        return None;
                    }
                    None => Err(Fallo::Estado),
                },
                _ => Err(Fallo::Estado),
            },
            Some(estado) => leer_centimos(l).map(|saldo| Respuesta { estado, saldo }).ok_or(Fallo::Importe),
        };
        *self = Self::nueva();
        Some(r)
    }
}

/// **Para la pantalla**: `124003` -> `1.240,03`, con los miles con punto y
/// la coma decimal, como se escribe en castellano. Devuelve cuantos bytes.
pub fn formato(c: Centimos, dst: &mut [u8]) -> Result<usize, Fallo> {
    let mut tmp = [0u8; 32];
    let mut i = tmp.len();
    let mut v = c.unsigned_abs();
    let cent = v % 100;
    v /= 100;
    for d in [cent % 10, cent / 10] {
        i -= 1;
        tmp[i] = b'0' + d as u8;
    }
    i -= 1;
    tmp[i] = b',';
    let mut k = 0;
    loop {
        if k > 0 && k % 3 == 0 {
            i -= 1;
            tmp[i] = b'.';
        }
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
        k += 1;
        if v == 0 {
            break;
        }
    }
    if c < 0 {
        i -= 1;
        tmp[i] = b'-';
    }
    let n = tmp.len() - i;
    dst.get_mut(..n).ok_or(Fallo::Corto)?.copy_from_slice(&tmp[i..]);
    Ok(n)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn orden(o: Orden) -> String {
        let mut b = [0u8; 64];
        let n = escribir(&o, &mut b).unwrap();
        String::from_utf8(b[..n].to_vec()).unwrap()
    }

    fn fmt(c: Centimos) -> String {
        let mut b = [0u8; 32];
        let n = formato(c, &mut b).unwrap();
        String::from_utf8(b[..n].to_vec()).unwrap()
    }

    #[test]
    fn las_ordenes_son_las_que_lee_el_motor() {
        assert_eq!(orden(Orden::Abrir(125_000)), "1\n1250.00\n");
        assert_eq!(orden(Orden::Cobrar(5)), "2\n0.05\n");
        assert_eq!(orden(Orden::Pagar(1999)), "3\n19.99\n");
        assert_eq!(orden(Orden::Veces(3)), "4\n3.00\n");
        assert_eq!(orden(Orden::Cuadrar), "5\n0.00\n");
        assert_eq!(orden(Orden::Cerrar), "9\n0.00\n");
        assert_eq!(orden(Orden::Pagar(-100)), "3\n-1.00\n");
        let mut chico = [0u8; 4];
        assert_eq!(escribir(&Orden::Abrir(125_000), &mut chico), Err(Fallo::Corto));
    }

    #[test]
    fn los_importes_se_leen_sin_coma_flotante() {
        assert_eq!(leer_centimos(b"19.99"), Some(1999));
        assert_eq!(leer_centimos(b"1240.03"), Some(124_003));
        assert_eq!(leer_centimos(b"0.00"), Some(0));
        assert_eq!(leer_centimos(b"-1.5"), Some(-150));
        assert_eq!(leer_centimos(b"7"), Some(700));
        assert_eq!(leer_centimos(b"9999999999999.99"), Some(TOPE));
        assert_eq!(leer_centimos(b"10000000000000.00"), None, "pasa del tope");
        assert_eq!(leer_centimos(b"1."), Some(100), "un punto sin decimales vale");
        for malo in [&b""[..], b"1.234", b"1,00", b"abc", b".5", b"- 1", b"99999999999999999999999"] {
            assert_eq!(leer_centimos(malo), None, "{:?}", core::str::from_utf8(malo));
        }
    }

    #[test]
    fn escribir_y_leer_son_la_misma_cuenta() {
        for c in [0, 1, 9, 10, 99, 100, 1999, 124_003, -5, TOPE, -TOPE] {
            let mut b = [0u8; 24];
            let n = escribir_centimos(c, &mut b).unwrap();
            assert_eq!(leer_centimos(&b[..n]), Some(c), "{c}");
        }
    }

    #[test]
    fn la_charla_junta_respuestas_de_dos_lineas() {
        let mut ch = Charla::nueva();
        let mut r = Vec::new();
        for &b in b"0\n1250.00\n2\n1250.00\r\n0\n1240.03\n" {
            if let Some(x) = ch.empujar(b) {
                r.push(x);
            }
        }
        assert_eq!(
            r,
            [
                Ok(Respuesta { estado: Estado::Hecho, saldo: 125_000 }),
                Ok(Respuesta { estado: Estado::SinSaldo, saldo: 125_000 }),
                Ok(Respuesta { estado: Estado::Hecho, saldo: 124_003 }),
            ]
        );
    }

    #[test]
    fn una_respuesta_rota_no_arrastra_a_la_siguiente() {
        let mut ch = Charla::nueva();
        let mut r = Vec::new();
        for &b in b"7\nX\n0\n12.x\n0\n1.00\n" {
            if let Some(x) = ch.empujar(b) {
                r.push(x);
            }
        }
        assert_eq!(r[0], Err(Fallo::Estado));
        assert_eq!(*r.last().unwrap(), Ok(Respuesta { estado: Estado::Hecho, saldo: 100 }));
        assert!(r.contains(&Err(Fallo::Importe)));
        let mut ch = Charla::nueva();
        let largo: Vec<u8> = core::iter::repeat(b'9').take(LINEA_MAX + 1).collect();
        assert!(largo.iter().any(|&b| ch.empujar(b) == Some(Err(Fallo::Largo))));
    }

    #[test]
    fn el_saldo_se_ve_como_en_castellano() {
        assert_eq!(fmt(124_003), "1.240,03");
        assert_eq!(fmt(0), "0,00");
        assert_eq!(fmt(5), "0,05");
        assert_eq!(fmt(100_000_000), "1.000.000,00");
        assert_eq!(fmt(-1999), "-19,99");
        assert_eq!(fmt(TOPE), "9.999.999.999.999,99");
    }

    #[test]
    fn todo_estado_tiene_su_frase() {
        for c in b'0'..=b'5' {
            assert!(!Estado::de(c).unwrap().texto().is_empty());
        }
        assert_eq!(Estado::de(b'6'), None);
    }
}
