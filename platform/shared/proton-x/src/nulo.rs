//! **El salto a 0, leido del informe** (03-10, N4.4): de `datos/fallos.txt`
//! a la CASILLA que valia 0, y en el codigo del `.exe`, quien la toca.
//!
//! [carril]  VERDE     lee texto y bytes; no toca la maquina
//! [cuesta]  DATO      una casilla mal calculada manda a mirar otra cosa
//! [riesgo]  AJENO     el formato del informe es el de la autopsia del
//!                     kernel (`antes ... ff 15 xx xx xx xx | ret`); si
//!                     cambia, esto devuelve `None` y no inventa
//! [consumo] NADA      una vez al arrancar, y solo si el ultimo fallo fue un
//!                     salto a 0 por `call [rip+..]`
//!
//! Seis corridas murieron igual: `call [rip+0x1838229]` desde
//! `Cyberpunk2077.exe+0x1d4c6cf`, y la casilla valia 0. La autopsia del
//! kernel solo conoce el `.bex`; PROTON-X conoce el `.exe`. Asi que al
//! arrancar, PROTON-X lee el informe de la corrida ANTERIOR y dice:
//!
//! ```text
//!    la casilla      retorno + disp32 del `ff 15` que hay antes del retorno
//!    que es          una importacion (su nombre), o una variable global
//!    quien la toca   cada `[rip+..]` del codigo que la apunta: lee, escribe,
//!                    llama; y de cada escritura, la cadena que se paso en
//!                    `rdx` justo antes (el nombre de un GetProcAddress) y
//!                    la ultima importacion llamada antes (cual da el valor)
//! ```
//!
//! [!] Buscar `[rip+..]` sin desensamblar es una heuristica: se mira cada
//! byte con forma de ModRM RIP-relativo (`mod = 00, rm = 101`) y se calcula a
//! donde apunta. Un acierto exige los 32 bits del desplazamiento exactos, asi
//! que los falsos son raros; los que hubiera se dicen con su RVA y se pueden
//! comprobar a mano.

use alloc::vec::Vec;

/// Lo que se saca del informe: el retorno que quedo en `[rsp]` y la casilla
/// del `call [rip+disp]` que hay justo antes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaltoNulo {
    pub retorno: u64,
    pub casilla: u64,
}

/// **El ultimo salto a 0 del informe** (`datos/fallos.txt`), si lo hay. Un
/// fallo con `rip` distinto de 0, o un salto que no fue por `ff 15`, no da
/// casilla y se salta.
pub fn del_informe(texto: &str) -> Option<SaltoNulo> {
    let mut ultimo = None;
    for bloque in texto.split("== FALLO EN RING 3").skip(1) {
        if let Some(s) = de_un_fallo(bloque) {
            ultimo = Some(s);
        }
    }
    ultimo
}

fn campo<'a>(bloque: &'a str, nombre: &str) -> Option<&'a str> {
    bloque.lines().find_map(|l| {
        let l = l.trim_start();
        let resto = l.strip_prefix(nombre)?;
        resto.starts_with(' ').then_some(resto.trim())
    })
}

fn hex(s: &str) -> Option<u64> {
    u64::from_str_radix(s.trim().strip_prefix("0x")?, 16).ok()
}

fn de_un_fallo(bloque: &str) -> Option<SaltoNulo> {
    // `rip       0x0  (FUERA de la imagen)`
    if hex(campo(bloque, "rip")?.split_whitespace().next()?)? != 0 {
        return None;
    }
    // `pila      SIN retornos en 32 palabras: 0x1001d4c6cf 0x...`: la primera
    // palabra es `[rsp]`, lo que dejo el `call`.
    let pila = campo(bloque, "pila")?;
    let palabras = pila.rsplit_once(':').map(|(_, d)| d).unwrap_or(pila);
    let retorno = palabras.split_whitespace().find_map(hex)?;
    // `antes     20 02 ... ff 15 29 82 83 01 | ret (el call`
    let antes = campo(bloque, "antes")?;
    let antes = antes.split('|').next()?;
    let b: Vec<u8> = antes
        .split_whitespace()
        .map(|x| u8::from_str_radix(x, 16).ok())
        .collect::<Option<_>>()?;
    let n = b.len();
    if n < 6 || b[n - 6] != 0xFF || b[n - 5] != 0x15 {
        return None;
    }
    let disp = i32::from_le_bytes([b[n - 4], b[n - 3], b[n - 2], b[n - 1]]);
    Some(SaltoNulo { retorno, casilla: retorno.wrapping_add(disp as i64 as u64) })
}

/// Que hace con la casilla la instruccion que la apunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toque {
    Lee,
    Escribe,
    Llama,
    Salta,
    /// `lea`, una comparacion rara...: la apunta, no se sabe para que.
    Otro,
}

impl Toque {
    pub fn nombre(self) -> &'static str {
        match self {
            Toque::Lee => "LEE",
            Toque::Escribe => "ESCRIBE",
            Toque::Llama => "LLAMA",
            Toque::Salta => "SALTA",
            Toque::Otro => "la apunta",
        }
    }
}

/// Una instruccion del codigo que apunta a la casilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Referencia {
    /// La RVA del byte de operacion (el prefijo REX, si lo hay, va antes).
    pub rva: u32,
    pub toque: Toque,
    /// Solo en las escrituras: la RVA de lo ultimo que se puso en `rdx` con
    /// `lea rdx, [rip+..]` antes (el nombre que se le pide a GetProcAddress).
    pub cadena: Option<u32>,
    /// Solo en las escrituras: la casilla de la ultima `call [rip+..]` antes
    /// (la importacion que dio el valor).
    pub tras_llamar: Option<u32>,
}

/// Cuanto se mira hacia atras desde una escritura buscando el `lea rdx` y la
/// `call`: lo que ocupa un `GetProcAddress(h, "x")` con su comprobacion.
const ATRAS: usize = 96;

/// Las que se devuelven como mucho: una casilla que toca medio programa no
/// es un puntero a funcion.
pub const TOPE: usize = 32;

fn disp_en(c: &[u8], i: usize) -> Option<i32> {
    Some(i32::from_le_bytes(c.get(i..i + 4)?.try_into().ok()?))
}

/// **Quien apunta a `casilla`** en `codigo`, los bytes de una seccion que
/// empieza en la RVA `desde`. Todo en RVA del mismo modulo.
pub fn referencias(codigo: &[u8], desde: u32, casilla: u32) -> Vec<Referencia> {
    let mut v = Vec::new();
    for j in 1..codigo.len().saturating_sub(4) {
        let m = codigo[j];
        if m & 0xC7 != 0x05 {
            continue;
        }
        let op = codigo[j - 1];
        let reg = (m >> 3) & 7;
        let previo = if j >= 2 { codigo[j - 2] } else { 0 };
        // Lo que va DETRAS del desplazamiento: el `rip` cuenta desde el final
        // de la instruccion, inmediato incluido.
        let inmediato = match op {
            0xC7 | 0x81 | 0x69 => 4,
            0xF7 if reg == 0 => 4,
            0xC6 | 0x80 | 0x83 | 0x6B | 0xC0 | 0xC1 => 1,
            0xF6 if reg == 0 => 1,
            _ => 0,
        };
        let Some(d) = disp_en(codigo, j + 1) else { continue };
        let fin = (desde as u64 + j as u64 + 5 + inmediato) as i64;
        if fin + d as i64 != casilla as i64 {
            continue;
        }
        let toque = match op {
            0x89 | 0xC7 | 0x87 => Toque::Escribe,
            0x11 | 0x29 | 0xD6 | 0x7F | 0xB1 if previo == 0x0F => Toque::Escribe,
            0x8B | 0x39 | 0x3B | 0x85 => Toque::Lee,
            0x10 | 0x28 | 0x6F | 0x7E if previo == 0x0F => Toque::Lee,
            0xFF if reg == 2 => Toque::Llama,
            0xFF if reg == 4 => Toque::Salta,
            0xFF if reg == 6 => Toque::Lee,
            _ => Toque::Otro,
        };
        let (cadena, tras_llamar) = if toque == Toque::Escribe {
            atras(codigo, desde, j - 1)
        } else {
            (None, None)
        };
        v.push(Referencia { rva: desde + j as u32 - 1, toque, cadena, tras_llamar });
        if v.len() == TOPE {
            break;
        }
    }
    v
}

/// Desde la escritura en `hasta` hacia atras: el ULTIMO `lea rdx, [rip+..]`
/// (`48 8d 15`) y la ULTIMA `call [rip+..]` (`ff 15`).
fn atras(codigo: &[u8], desde: u32, hasta: usize) -> (Option<u32>, Option<u32>) {
    let mut cadena = None;
    let mut llamada = None;
    for i in hasta.saturating_sub(ATRAS)..hasta {
        let a = |k: usize| (desde as i64 + k as i64) as u32;
        match codigo.get(i..i + 3) {
            Some([0x48, 0x8D, 0x15]) => {
                if let Some(d) = disp_en(codigo, i + 3) {
                    cadena = Some(a(i + 7).wrapping_add(d as u32));
                }
            }
            Some([0xFF, 0x15, _]) => {
                if let Some(d) = disp_en(codigo, i + 2) {
                    llamada = Some(a(i + 6).wrapping_add(d as u32));
                }
            }
            _ => {}
        }
    }
    (cadena, llamada)
}

/// Una cadena ASCII que se pueda imprimir (el nombre de una funcion), de
/// como mucho 64 bytes y terminada en 0. Si no lo parece, `None`.
pub fn cadena_en(img: &[u8], rva: u32) -> Option<&str> {
    let resto = img.get(rva as usize..)?;
    let n = resto.iter().take(65).position(|&b| b == 0)?;
    let s = &resto[..n];
    if n < 2 || !s.iter().all(|&b| (0x20..0x7F).contains(&b)) {
        return None;
    }
    core::str::from_utf8(s).ok()
}
