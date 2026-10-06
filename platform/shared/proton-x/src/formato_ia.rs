//! **Los formatos de un vertice** (03-10, N3.1): de los bytes de un elemento
//! del input layout a los cuatro componentes que lee el sombreador.
//!
//! [carril]  VERDE     bytes a numeros; no toca la maquina
//! [cuesta]  DATO      un formato mal leido mueve un vertice o cambia un color
//! [riesgo]  SILENCIO  un vertice mal leido no falla: se dibuja en otro sitio.
//!                     Cada clase tiene su fila en el banco
//! [consumo] NADA      una vez por vertice distinto de cada dibujo
//!
//! Hasta el 03-10 la casa solo leia floats de 32 bits, y cualquier otro
//! formato NEGABA el PSO entero (`E_INVALIDARG`): en el metal, Cyberpunk pidio
//! uno de esos y se le nego. Un motor guarda normales y tangentes en 8 o 16
//! bits, y colores en RGBA8: es lo normal, no lo raro.
//!
//! Como los lee el sombreador, que es lo que importa:
//!
//! ```text
//!    FLOAT (32, 16, 11/10)   el float
//!    UNORM                   n / (2^bits - 1), de 0 a 1
//!    SNORM                   n / (2^(bits-1) - 1), de -1 a 1 (el mas bajo, -1)
//!    UINT, SINT              el ENTERO, en los bits del registro: el
//!                            interprete guarda los enteros asi (`Valor::Bits`)
//!    lo que el formato no    0, 0, 0 y 1 (el 1 como float o como entero,
//!    trae                    segun la clase), como D3D12
//!    B8G8R8A8                se da la vuelta: el sombreador ve RGBA
//! ```

use alloc::vec::Vec;

/// Como se lee cada componente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clase {
    Float,
    Unorm,
    Snorm,
    Uint,
    Sint,
}

/// **La forma de un formato**: cuantos bytes ocupa, cuantos componentes, de
/// cuantos bits cada uno, y su clase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forma {
    pub bytes: u32,
    pub clase: Clase,
    /// Los bits de cada componente, en orden (0 = no lo trae).
    pub bits: [u8; 4],
    /// B8G8R8A8: los bytes vienen al reves de como los ve el sombreador.
    pub bgra: bool,
}

impl Forma {
    const fn de(clase: Clase, bits: [u8; 4]) -> Self {
        let total = bits[0] as u32 + bits[1] as u32 + bits[2] as u32 + bits[3] as u32;
        Forma { bytes: total / 8, clase, bits, bgra: false }
    }

    /// Cuantos componentes trae.
    pub fn componentes(&self) -> usize {
        self.bits.iter().filter(|&&b| b != 0).count()
    }
}

/// `R32G32B32A32_FLOAT` y compania: los que la 3060 ya lee tal cual.
pub const FMT_R32G32B32A32_FLOAT: u32 = 2;
pub const FMT_R32G32B32_FLOAT: u32 = 6;
pub const FMT_R32G32_FLOAT: u32 = 16;
pub const FMT_R32_FLOAT: u32 = 41;

/// **La forma de un `DXGI_FORMAT` de vertice**, o `None` si no es uno.
pub fn forma(formato: u32) -> Option<Forma> {
    use Clase::*;
    let f = match formato {
        2 => Forma::de(Float, [32, 32, 32, 32]),
        3 => Forma::de(Uint, [32, 32, 32, 32]),
        4 => Forma::de(Sint, [32, 32, 32, 32]),
        6 => Forma::de(Float, [32, 32, 32, 0]),
        7 => Forma::de(Uint, [32, 32, 32, 0]),
        8 => Forma::de(Sint, [32, 32, 32, 0]),
        10 => Forma::de(Float, [16, 16, 16, 16]),
        11 => Forma::de(Unorm, [16, 16, 16, 16]),
        12 => Forma::de(Uint, [16, 16, 16, 16]),
        13 => Forma::de(Snorm, [16, 16, 16, 16]),
        14 => Forma::de(Sint, [16, 16, 16, 16]),
        16 => Forma::de(Float, [32, 32, 0, 0]),
        17 => Forma::de(Uint, [32, 32, 0, 0]),
        18 => Forma::de(Sint, [32, 32, 0, 0]),
        24 => Forma::de(Unorm, [10, 10, 10, 2]),
        25 => Forma::de(Uint, [10, 10, 10, 2]),
        26 => Forma::de(Float, [11, 11, 10, 0]),
        28 => Forma::de(Unorm, [8, 8, 8, 8]),
        30 => Forma::de(Uint, [8, 8, 8, 8]),
        31 => Forma::de(Snorm, [8, 8, 8, 8]),
        32 => Forma::de(Sint, [8, 8, 8, 8]),
        34 => Forma::de(Float, [16, 16, 0, 0]),
        35 => Forma::de(Unorm, [16, 16, 0, 0]),
        36 => Forma::de(Uint, [16, 16, 0, 0]),
        37 => Forma::de(Snorm, [16, 16, 0, 0]),
        38 => Forma::de(Sint, [16, 16, 0, 0]),
        41 => Forma::de(Float, [32, 0, 0, 0]),
        42 => Forma::de(Uint, [32, 0, 0, 0]),
        43 => Forma::de(Sint, [32, 0, 0, 0]),
        49 => Forma::de(Unorm, [8, 8, 0, 0]),
        50 => Forma::de(Uint, [8, 8, 0, 0]),
        51 => Forma::de(Snorm, [8, 8, 0, 0]),
        52 => Forma::de(Sint, [8, 8, 0, 0]),
        54 => Forma::de(Float, [16, 0, 0, 0]),
        56 => Forma::de(Unorm, [16, 0, 0, 0]),
        57 => Forma::de(Uint, [16, 0, 0, 0]),
        58 => Forma::de(Snorm, [16, 0, 0, 0]),
        59 => Forma::de(Sint, [16, 0, 0, 0]),
        61 => Forma::de(Unorm, [8, 0, 0, 0]),
        62 => Forma::de(Uint, [8, 0, 0, 0]),
        63 => Forma::de(Snorm, [8, 0, 0, 0]),
        64 => Forma::de(Sint, [8, 0, 0, 0]),
        87 => Forma { bgra: true, ..Forma::de(Unorm, [8, 8, 8, 8]) },
        _ => return None,
    };
    Some(f)
}

/// Si es un float de 32 bits por componente (lo que la 3060 lee hoy).
pub fn es_float32(formato: u32) -> bool {
    matches!(formato, FMT_R32G32B32A32_FLOAT | FMT_R32G32B32_FLOAT | FMT_R32G32_FLOAT | FMT_R32_FLOAT)
}

/// Un half (IEEE 754 binary16) a float.
pub fn half(h: u16) -> f32 {
    let s = (h as u32 & 0x8000) << 16;
    let e = (h >> 10) as u32 & 0x1F;
    let m = h as u32 & 0x3FF;
    let bits = match e {
        0 if m == 0 => s,
        0 => {
            // Subnormal: normalizarlo.
            let mut m = m;
            let mut e = 113u32;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            s | e << 23 | (m & 0x3FF) << 13
        }
        0x1F => s | 0x7F80_0000 | m << 13,
        _ => s | (e + 112) << 23 | m << 13,
    };
    f32::from_bits(bits)
}

/// Un float sin signo de 11 o 10 bits (5 de exponente) a float.
fn chico(v: u32, mantisa: u32) -> f32 {
    let e = v >> mantisa & 0x1F;
    let m = v & ((1 << mantisa) - 1);
    match e {
        // Subnormal: m / 2^mantisa por 2^-14.
        0 => m as f32 / (1u32 << mantisa) as f32 * 6.103_515_6e-5,
        0x1F => {
            if m == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => f32::from_bits((e + 112) << 23 | m << (23 - mantisa)),
    }
}

/// **Los cuatro componentes** de un elemento de formato `formato` en `v`
/// (sus bytes, desde el elemento). Lo que falte en `v` cuenta como 0.
pub fn leer(formato: u32, v: &[u8]) -> [f32; 4] {
    let Some(f) = forma(formato) else { return [0.0, 0.0, 0.0, 1.0] };
    let entero = matches!(f.clase, Clase::Uint | Clase::Sint);
    let uno = if entero { f32::from_bits(1) } else { 1.0 };
    let mut x = [0.0, 0.0, 0.0, uno];
    // Los bytes del elemento, como un numero de hasta 128 bits.
    let mut b = [0u8; 16];
    let n = (f.bytes as usize).min(v.len()).min(16);
    b[..n].copy_from_slice(&v[..n]);
    let todo = u128::from_le_bytes(b);
    let mut desde = 0u32;
    for (c, &bits) in f.bits.iter().enumerate() {
        if bits == 0 {
            continue;
        }
        let bits = bits as u32;
        let crudo = ((todo >> desde) & ((1u128 << bits) - 1)) as u32;
        desde += bits;
        let signo = |r: u32| ((r << (32 - bits)) as i32) >> (32 - bits);
        x[c] = match (f.clase, bits) {
            (Clase::Float, 32) => f32::from_bits(crudo),
            (Clase::Float, 16) => half(crudo as u16),
            (Clase::Float, 11) => chico(crudo, 6),
            (Clase::Float, 10) => chico(crudo, 5),
            (Clase::Float, _) => 0.0,
            (Clase::Unorm, _) => crudo as f32 / ((1u64 << bits) - 1) as f32,
            (Clase::Snorm, _) => (signo(crudo) as f32 / ((1u32 << (bits - 1)) - 1) as f32).max(-1.0),
            (Clase::Uint, _) => f32::from_bits(crudo),
            (Clase::Sint, _) => f32::from_bits(signo(crudo) as u32),
        };
    }
    if f.bgra {
        x.swap(0, 2);
    }
    x
}

/// Un float SIN signo (|x|) a uno de 5 bits de exponente y `mantisa` bits
/// (el half, sin su signo; los de 11 y 10 de R11G11B10), redondeando al PAR,
/// como D3D. Lo que pasa del mayor, infinito.
fn a_cinco(x: f32, mantisa: u32) -> u32 {
    let b = x.to_bits() & 0x7FFF_FFFF;
    let e = (b >> 23) as i32;
    let m = b & 0x7F_FFFF;
    let inf = 0x1F << mantisa;
    if e == 0xFF {
        // Infinito, o NaN (con un bit de mantisa, que siga siendo NaN).
        return inf | if m != 0 { 1 << (mantisa - 1) } else { 0 };
    }
    let e = e - 127 + 15;
    if e >= 0x1F {
        return inf;
    }
    // Normal, o subnormal (con el 1 escondido a la vista y corrido de mas).
    let (mant, corre) = if e <= 0 { (m | 0x80_0000, (24 - mantisa) as i32 - e) } else { (m, (23 - mantisa) as i32) };
    if corre > 24 {
        return 0;
    }
    let corre = corre as u32;
    let base = if e <= 0 { 0 } else { (e as u32) << mantisa };
    let resto = mant & ((1 << corre) - 1);
    let mitad = 1 << (corre - 1);
    let mut h = base + (mant >> corre);
    if resto > mitad || (resto == mitad && h & 1 != 0) {
        h += 1;
    }
    h
}

/// Un float a half (IEEE 754 de 16 bits), redondeando al PAR, como D3D.
pub fn a_half(x: f32) -> u16 {
    ((x.to_bits() >> 16) & 0x8000) as u16 | a_cinco(x, 10) as u16
}

/// **Un color, como lo deja un formato** (N5.16, 05-10): lo que se lee de
/// vuelta tras escribirlo en `formato` (un RGBA16F redondea al half, un
/// UNORM satura...). Lo que la casa guarda en floats de 32 bits se cuantiza
/// asi al escribirlo, para que valga lo mismo que en la GPU.
pub fn cuantizar(formato: u32, c: [f32; 4]) -> [f32; 4] {
    match empaquetar(formato, c.map(f32::to_bits), false) {
        Some(b) => leer(formato, &b),
        None => c,
    }
}

/// Si es un formato de ENTEROS (UINT o SINT): sus valores van en los bits
/// del registro, no en un float.
pub fn es_entero(formato: u32) -> bool {
    forma(formato).is_some_and(|f| matches!(f.clase, Clase::Uint | Clase::Sint))
}

/// **Lo que un sombreador ENTERO deja en un render target de enteros**
/// (05-10, los R32_UINT, R8_UINT, RGBA16_SINT...): cada canal son los bits
/// de un entero de 32 (sin signo en un UINT, con signo en un SINT) y se
/// SATURA a los bits del canal, la regla de conversion entre enteros de la
/// especificacion funcional de D3D11.3 (3.2.3.6), que D3D12 hereda: 300 en
/// un R8_UINT es 255, -40000 en un R16_SINT es -32768. Sin mezcla (en un
/// entero no la hay). Devuelve lo que se lee de vuelta (como [`leer`]), o
/// `None` si el formato no es de enteros.
pub fn de_entero(formato: u32, c: [f32; 4]) -> Option<[f32; 4]> {
    let f = forma(formato).filter(|f| matches!(f.clase, Clase::Uint | Clase::Sint))?;
    let mut v = c.map(f32::to_bits);
    if f.bgra {
        v.swap(0, 2);
    }
    for (x, &n) in v.iter_mut().zip(&f.bits) {
        let n = n as u32;
        if n == 0 || n == 32 {
            continue;
        }
        *x = match f.clase {
            Clase::Uint => (*x).min((1 << n) - 1),
            _ => (*x as i32).clamp(-(1 << (n - 1)), (1 << (n - 1)) - 1) as u32,
        };
    }
    if f.bgra {
        v.swap(0, 2);
    }
    empaquetar(formato, v, true).map(|b| leer(formato, &b))
}

/// **Un elemento en su formato** (N5.3c, 05-10, lo de `ClearUnorderedAccessView`):
/// lo contrario de [`leer`]. `v` trae un valor por canal: con `crudo` (la
/// version Uint de D3D12), los bits bajos de cada uno tal cual, sin
/// convertir; si no (la Float), los bits de un f32 convertidos a la clase
/// del formato. `None` si el formato no es uno de estos (o es de floats de
/// 11 y 10 bits: todavia no).
pub fn empaquetar(formato: u32, v: [u32; 4], crudo: bool) -> Option<Vec<u8>> {
    let f = forma(formato)?;
    let mut v = v;
    if f.bgra {
        v.swap(0, 2);
    }
    let (mut todo, mut desde) = (0u128, 0u32);
    for c in 0..4 {
        let n = f.bits[c] as u32;
        if n == 0 {
            continue;
        }
        let mascara = if n == 32 { u32::MAX } else { (1u32 << n) - 1 };
        let x = f32::from_bits(v[c]);
        let bits = if crudo {
            v[c] & mascara
        } else {
            match (f.clase, n) {
                (Clase::Float, 32) => v[c],
                (Clase::Float, 16) => a_half(x) as u32,
                // R11G11B10: sin signo (lo negativo es 0; NaN sigue NaN).
                (Clase::Float, 11) => a_cinco(if x < 0.0 { 0.0 } else { x }, 6),
                (Clase::Float, 10) => a_cinco(if x < 0.0 { 0.0 } else { x }, 5),
                (Clase::Float, _) => return None,
                // Saturar (NaN es 0), por el maximo y redondear.
                (Clase::Unorm, _) => (if x > 0.0 { x.min(1.0) } else { 0.0 } * mascara as f32 + 0.5) as u32,
                (Clase::Snorm, _) => {
                    let m = (mascara >> 1) as f32;
                    let y = if x.is_nan() { 0.0 } else { x.clamp(-1.0, 1.0) * m };
                    (if y < 0.0 { y - 0.5 } else { y + 0.5 }) as i32 as u32 & mascara
                }
                (Clase::Uint, _) => (if x > 0.0 { x } else { 0.0 } as u64).min(mascara as u64) as u32,
                (Clase::Sint, _) => {
                    let tope = (mascara >> 1) as i64;
                    (x as i64).clamp(-tope - 1, tope) as u32 & mascara
                }
            }
        };
        todo |= (bits as u128) << desde;
        desde += n;
    }
    Some(todo.to_le_bytes()[..f.bytes as usize].to_vec())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn empaquetar_es_lo_contrario_de_leer() {
        let f = |x: [f32; 4]| x.map(f32::to_bits);
        // R8G8B8A8_UNORM (28): saturar y redondear; B8G8R8A8 (87): al reves.
        assert_eq!(empaquetar(28, f([1.0, 0.5, -3.0, 2.0]), false), Some(vec![255, 128, 0, 255]));
        assert_eq!(empaquetar(87, f([1.0, 0.5, 0.0, 1.0]), false), Some(vec![0, 128, 255, 255]));
        // R16G16B16A16_FLOAT (10): ida y vuelta exacta en lo que cabe.
        let h = empaquetar(10, f([1.5, -0.25, 65504.0, 0.0]), false).unwrap();
        assert_eq!(leer(10, &h), [1.5, -0.25, 65504.0, 0.0]);
        assert_eq!(a_half(1.0 + 1.0 / 2048.0), 0x3C00, "empate al par: abajo");
        assert_eq!(a_half(1.0 + 3.0 / 2048.0), 0x3C02, "empate al par: arriba");
        assert_eq!(a_half(1e-7), 0x0002, "subnormal");
        assert_eq!(a_half(-2.0), 0xC000);
        assert_eq!(a_half(70000.0), 0x7C00, "pasa del mayor: infinito");
        // R11G11B10_FLOAT (26): ida y vuelta, y lo negativo a 0.
        let p = empaquetar(26, f([1.5, 0.25, 3.0, 0.0]), false).unwrap();
        assert_eq!(leer(26, &p), [1.5, 0.25, 3.0, 1.0]);
        assert_eq!(cuantizar(26, [-1.0, 1.0 + 1.0 / 128.0, 0.0, 0.0])[..2], [0.0, 1.0], "6 bits de mantisa: 1/128 se va");
        assert_eq!(cuantizar(10, [1.0 / 3.0, 5.5, 0.0, 1.0]), [half(a_half(1.0 / 3.0)), 5.5, 0.0, 1.0]);
        // La Uint: los bits bajos, sin convertir (R8G8B8A8_UINT 30).
        assert_eq!(empaquetar(30, [0x1FF, 2, 3, 4], true), Some(vec![0xFF, 2, 3, 4]));
        // R32_UINT (42) y R32_FLOAT (41).
        assert_eq!(empaquetar(42, [0xDEAD_BEEF, 0, 0, 0], true), Some(0xDEAD_BEEFu32.to_le_bytes().to_vec()));
        assert_eq!(empaquetar(41, f([2.5, 0.0, 0.0, 0.0]), false), Some(2.5f32.to_le_bytes().to_vec()));
    }

    #[test]
    fn floats_de_32_como_siempre() {
        let v: Vec<u8> = [1.5f32, -2.0, 0.25, 7.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        assert_eq!(leer(2, &v), [1.5, -2.0, 0.25, 7.0]);
        assert_eq!(leer(6, &v), [1.5, -2.0, 0.25, 1.0], "lo que falta: w = 1");
        assert_eq!(leer(41, &v), [1.5, 0.0, 0.0, 1.0]);
        assert!(es_float32(16) && !es_float32(28));
        assert_eq!(forma(6).unwrap().bytes, 12);
    }

    #[test]
    fn unorm_y_snorm_de_8_y_16() {
        assert_eq!(leer(28, &[0, 255, 128, 51]), [0.0, 1.0, 128.0 / 255.0, 0.2]);
        // SNORM: 127 = 1, -127 = -1, y -128 tambien -1.
        assert_eq!(leer(31, &[127, 0x81, 0x80, 0]), [1.0, -1.0, -1.0, 0.0]);
        assert_eq!(leer(35, &[0xFF, 0xFF, 0, 0]), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(leer(37, &0x8001u16.to_le_bytes()), [-1.0, 0.0, 0.0, 1.0]);
        assert_eq!(leer(61, &[255]), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(forma(28).unwrap().bytes, 4);
    }

    #[test]
    fn enteros_en_los_bits_del_registro() {
        let x = leer(30, &[1, 2, 255, 0]);
        assert_eq!(x.map(f32::to_bits), [1, 2, 255, 0]);
        let x = leer(32, &[0xFF, 0x80, 5, 0]);
        assert_eq!(x.map(f32::to_bits), [(-1i32) as u32, (-128i32) as u32, 5, 0]);
        // Lo que falta en uno entero: w = 1 ENTERO, no 1.0.
        assert_eq!(leer(42, &7u32.to_le_bytes()).map(f32::to_bits), [7, 0, 0, 1]);
    }

    #[test]
    fn un_render_target_de_enteros_satura_lo_que_no_cabe() {
        let b = |v: [u32; 4]| v.map(f32::from_bits);
        // R8_UINT (62): 300 -> 255, 77 tal cual; lo que no trae, (0, 0, 1).
        assert_eq!(de_entero(62, b([300, 9, 9, 9])).unwrap().map(f32::to_bits), [255, 0, 0, 1]);
        assert_eq!(de_entero(62, b([77, 0, 0, 0])).unwrap().map(f32::to_bits), [77, 0, 0, 1]);
        // R16G16B16A16_SINT (14): -5, 40000 -> 32767, -40000 -> -32768, 123.
        let s = |x: i32| x as u32;
        assert_eq!(de_entero(14, b([s(-5), 40000, s(-40000), 123])).unwrap().map(f32::to_bits), [s(-5), 32767, s(-32768), 123]);
        // R32_UINT y R32_SINT: los 32 bits, tal cual.
        assert_eq!(de_entero(42, b([0xDEAD_BEEF, 0, 0, 0])).unwrap()[0].to_bits(), 0xDEAD_BEEF);
        assert_eq!(de_entero(43, b([s(-7), 0, 0, 0])).unwrap()[0].to_bits(), s(-7));
        // Uno de floats no es de enteros.
        assert!(de_entero(41, [0.0; 4]).is_none() && de_entero(28, [0.0; 4]).is_none());
        assert!(es_entero(30) && es_entero(43) && !es_entero(10));
    }

    #[test]
    fn halfs_y_floats_chicos() {
        // 1.0, -2.0, 0.5, 65504 (el mayor half).
        let h: Vec<u8> = [0x3C00u16, 0xC000, 0x3800, 0x7BFF].iter().flat_map(|x| x.to_le_bytes()).collect();
        assert_eq!(leer(10, &h), [1.0, -2.0, 0.5, 65504.0]);
        assert_eq!(half(0x0001), 2f32.powi(-24), "el subnormal mas chico");
        assert!(half(0x7C00).is_infinite() && half(0x7E00).is_nan());
        // R11G11B10: 1.0 en los tres (exponente 15).
        let uno11 = 15u32 << 6;
        let uno10 = 15u32 << 5;
        let v = (uno11 | uno11 << 11 | uno10 << 22).to_le_bytes();
        assert_eq!(leer(26, &v), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn diez_diez_diez_dos_y_bgra() {
        let v = (1023u32 | 0 << 10 | 511 << 20 | 3 << 30).to_le_bytes();
        let x = leer(24, &v);
        assert_eq!(x[0], 1.0);
        assert_eq!(x[1], 0.0);
        assert!((x[2] - 511.0 / 1023.0).abs() < 1e-6);
        assert_eq!(x[3], 1.0);
        assert_eq!(leer(87, &[0, 0, 255, 255]), [1.0, 0.0, 0.0, 1.0], "BGRA: el rojo va en el tercer byte");
    }

    #[test]
    fn lo_que_no_es_un_formato_de_vertice() {
        assert_eq!(forma(0), None);
        assert_eq!(forma(71), None, "BC1 no es de vertice");
        assert_eq!(leer(0, &[1, 2, 3]), [0.0, 0.0, 0.0, 1.0]);
        // Bytes que no llegan: lo que falta, cero.
        assert_eq!(leer(28, &[255]), [1.0, 0.0, 0.0, 0.0]);
    }
}
