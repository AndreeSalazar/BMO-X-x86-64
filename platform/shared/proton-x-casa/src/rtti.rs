//! **El RTTI de C++ de MSVC** (tanda 17 de Cyberpunk, 30-09): DURAS del
//! censo.
//!
//! ```text
//!    __RTtypeid(objeto)       el type_info del objeto COMPLETO: el
//!                             CompleteObjectLocator esta en vftable[-1]
//!    __RTDynamicCast(objeto, delta, desde, hacia, es_referencia)
//!                             el objeto completo, y en su jerarquia de bases
//!                             (ClassHierarchyDescriptor) la base de ese
//!                             tipo, por NOMBRE; si no esta: NULL, o
//!                             std::bad_cast si es una referencia
//!    __unDName(Ex)            des-decorar un nombre de MSVC: los TIPOS
//!                             (lo que usa type_info::name: "?AVHija@@" ->
//!                             "class Hija", con plantillas y espacios de
//!                             nombres) y el NOMBRE de una funcion
//!                             (UNDNAME_NAME_ONLY); la firma entera de una
//!                             funcion, todavia no: da su nombre
//! ```
//!
//! Las tablas (x64, firma 1): el CompleteObjectLocator { firma, desp,
//! cdDesp, RVA del TypeDescriptor, RVA del ClassHierarchyDescriptor, RVA de
//! si mismo } (la base de la imagen es su direccion menos su propia RVA); el
//! CHD { firma, atributos, cuantas bases, RVA de la lista }; cada
//! BaseClassDescriptor { RVA del TypeDescriptor, bases que contiene, PMD
//! {mdisp, pdisp, vdisp}, atributos }. El nombre de un TypeDescriptor
//! empieza en +16.
//!
//! `__RTDynamicCast` y `__RTtypeid` los envuelve un trozo de asm en
//! `msvcp_errores` (lanzar necesita la pila tal como la dejo el `.exe`).

use alloc::string::String;
use alloc::vec::Vec;

use crate::dir;

// -- Leer la memoria del `.exe` ------------------------------------------------------------

fn u32_en(d: u64) -> u32 {
    // SAFETY: las tablas de RTTI del `.exe` (las pone su compilador).
    unsafe { (d as *const u32).read_unaligned() }
}

fn i32_en(d: u64) -> i32 {
    u32_en(d) as i32
}

fn u64_en(d: u64) -> u64 {
    // SAFETY: lo mismo: un puntero del objeto o de su vtabla.
    unsafe { (d as *const u64).read_unaligned() }
}

/// El nombre decorado de un TypeDescriptor (desde su +16, hasta el cero).
fn nombre_de(td: u64) -> Vec<u8> {
    let mut v = Vec::new();
    while v.len() < 4096 {
        // SAFETY: el nombre del TypeDescriptor, terminado en cero.
        let c = unsafe { ((td + 16 + v.len() as u64) as *const u8).read() };
        if c == 0 {
            break;
        }
        v.push(c);
    }
    v
}

/// El CompleteObjectLocator del objeto y la base de su imagen; `None` si
/// su vtabla no lo trae (los objetos que lanza la propia casa: su hueco del
/// COL es 0) o no es el de x64 (firma 1).
fn localizador(objeto: u64) -> Option<(u64, u64)> {
    let vft = u64_en(objeto);
    let col = u64_en(vft - 8);
    if col == 0 || u32_en(col) != 1 {
        return None;
    }
    Some((col, col - u32_en(col + 20) as u64))
}

/// El objeto completo de `objeto` (el que tiene ese vfptr): menos su
/// desplazamiento, y el del vtordisp si lo hay.
fn completo(objeto: u64, col: u64) -> u64 {
    let desp = u32_en(col + 4) as u64;
    let cd = u32_en(col + 8);
    let mut c = objeto - desp;
    if cd != 0 {
        c = c.wrapping_sub(i32_en(objeto - cd as u64) as i64 as u64);
    }
    c
}

/// **`__RTtypeid`**: el TypeDescriptor (el type_info) del objeto completo.
pub(crate) extern "win64" fn rt_typeid(objeto: u64) -> u64 {
    match localizador(objeto) {
        Some((col, base)) => base + u32_en(col + 12) as u64,
        None => {
            crate::aviso("__RTtypeid de un objeto sin RTTI (de la casa)");
            0
        }
    }
}

/// **`__RTDynamicCast`** sin lanzar: el resultado, o 0. (El VfDelta, que el
/// compilador solo pone distinto de 0 con vtordisp raros, no se usa: el
/// objeto completo sale de su propio vfptr.)
pub(crate) extern "win64" fn rt_dynamic_cast(objeto: u64, _delta: i32, _desde: u64, hacia: u64) -> u64 {
    if objeto == 0 {
        return 0;
    }
    let Some((col, base)) = localizador(objeto) else { return 0 };
    let c = completo(objeto, col);
    let buscado = nombre_de(hacia);
    let chd = base + u32_en(col + 16) as u64;
    let n = u32_en(chd + 8).min(4096);
    let lista = base + u32_en(chd + 12) as u64;
    for k in 0..n as u64 {
        let bcd = base + u32_en(lista + 4 * k) as u64;
        let atributos = u32_en(bcd + 20);
        // BCD_NOTVISIBLE (1) y BCD_AMBIGUOUS (2): no se llega a ella.
        if atributos & 3 != 0 {
            continue;
        }
        if nombre_de(base + u32_en(bcd) as u64) != buscado {
            continue;
        }
        let (mdisp, pdisp, vdisp) = (i32_en(bcd + 8) as i64, i32_en(bcd + 12) as i64, i32_en(bcd + 16) as i64);
        let mut desp = mdisp;
        if pdisp >= 0 {
            // Una base virtual: su sitio lo dice la vbtable.
            let vbt = u64_en((c as i64 + pdisp) as u64);
            desp += pdisp + i32_en((vbt as i64 + vdisp) as u64) as i64;
        }
        return (c as i64 + desp) as u64;
    }
    0
}

// -- Des-decorar ----------------------------------------------------------------------------

/// Quien lee un nombre decorado: por donde va y los nombres de antes (las
/// referencias "0".."9").
struct Lector<'a> {
    s: &'a [u8],
    i: usize,
    nombres: Vec<String>,
}

impl Lector<'_> {
    fn ver(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn tomar(&mut self) -> Option<u8> {
        let c = self.ver()?;
        self.i += 1;
        Some(c)
    }

    /// Un identificador hasta su '@'.
    fn ident(&mut self) -> Option<String> {
        let desde = self.i;
        while self.ver()? != b'@' {
            self.i += 1;
        }
        let n = String::from_utf8_lossy(&self.s[desde..self.i]).into_owned();
        self.i += 1;
        Some(n)
    }

    /// Un trozo de un nombre: una referencia, una plantilla o un identificador.
    fn trozo(&mut self) -> Option<String> {
        let c = self.ver()?;
        if c.is_ascii_digit() {
            self.i += 1;
            return self.nombres.get((c - b'0') as usize).cloned();
        }
        if self.s[self.i..].starts_with(b"?$") {
            self.i += 2;
            // Una plantilla: su nombre y sus argumentos, con sus propias
            // referencias de nombres.
            let mut dentro = Lector { s: self.s, i: self.i, nombres: Vec::new() };
            let n = dentro.ident()?;
            dentro.nombres.push(n.clone());
            let mut args = Vec::new();
            while dentro.ver()? != b'@' {
                args.push(dentro.argumento()?);
            }
            dentro.i += 1;
            self.i = dentro.i;
            let mut t = alloc::format!("{n}<{}", args.join(","));
            if t.ends_with('>') {
                t.push(' ');
            }
            t.push('>');
            self.recordar(&t);
            return Some(t);
        }
        let n = self.ident()?;
        self.recordar(&n);
        Some(n)
    }

    fn recordar(&mut self, n: &str) {
        if self.nombres.len() < 10 && !self.nombres.iter().any(|x| x == n) {
            self.nombres.push(String::from(n));
        }
    }

    /// Un nombre calificado hasta su "@" final: `B@A@@` -> `A::B`.
    fn calificado(&mut self) -> Option<String> {
        let mut partes = Vec::new();
        while self.ver()? != b'@' {
            partes.push(self.trozo()?);
        }
        self.i += 1;
        partes.reverse();
        Some(partes.join("::"))
    }

    /// Un argumento de plantilla: un tipo o un numero ($0...).
    fn argumento(&mut self) -> Option<String> {
        if self.s[self.i..].starts_with(b"$0") {
            self.i += 2;
            return self.numero().map(|n| alloc::format!("{n}"));
        }
        self.tipo()
    }

    /// Un numero de MSVC: '?' delante es negativo; un digito d es d+1; si
    /// no, cifras hexadecimales de 'A' (0) a 'P' (15) hasta '@'.
    fn numero(&mut self) -> Option<i64> {
        let negativo = self.ver()? == b'?';
        if negativo {
            self.i += 1;
        }
        let c = self.tomar()?;
        let v = if c.is_ascii_digit() {
            (c - b'0') as i64 + 1
        } else {
            let mut v = (c.checked_sub(b'A')? as i64) & 0xF;
            loop {
                let d = self.tomar()?;
                if d == b'@' {
                    break;
                }
                v = v * 16 + (d.checked_sub(b'A')? as i64 & 0xF);
            }
            v
        };
        Some(if negativo { -v } else { v })
    }

    /// Un tipo.
    fn tipo(&mut self) -> Option<String> {
        let c = self.tomar()?;
        let simple = |s: &str| Some(String::from(s));
        match c {
            b'C' => simple("signed char"),
            b'D' => simple("char"),
            b'E' => simple("unsigned char"),
            b'F' => simple("short"),
            b'G' => simple("unsigned short"),
            b'H' => simple("int"),
            b'I' => simple("unsigned int"),
            b'J' => simple("long"),
            b'K' => simple("unsigned long"),
            b'M' => simple("float"),
            b'N' => simple("double"),
            b'O' => simple("long double"),
            b'X' => simple("void"),
            b'_' => match self.tomar()? {
                b'N' => simple("bool"),
                b'J' => simple("__int64"),
                b'K' => simple("unsigned __int64"),
                b'W' => simple("wchar_t"),
                b'S' => simple("char16_t"),
                b'U' => simple("char32_t"),
                b'Q' => simple("char8_t"),
                _ => None,
            },
            b'V' => Some(alloc::format!("class {}", self.calificado()?)),
            b'U' => Some(alloc::format!("struct {}", self.calificado()?)),
            b'T' => Some(alloc::format!("union {}", self.calificado()?)),
            b'W' => {
                self.tomar()?; // el tipo de debajo del enum (4: int)
                Some(alloc::format!("enum {}", self.calificado()?))
            }
            b'P' | b'Q' | b'A' => {
                // Puntero (P; Q: const) o referencia (A): __ptr64 (E) y el
                // const/volatile de lo apuntado (A nada, B const, C
                // volatile, D los dos).
                let ptr64 = if self.ver()? == b'E' {
                    self.i += 1;
                    " __ptr64"
                } else {
                    ""
                };
                let cv = match self.tomar()? {
                    b'B' => "const ",
                    b'C' => "volatile ",
                    b'D' => "const volatile ",
                    _ => "",
                };
                let debajo = self.tipo()?;
                let marca = if c == b'A' { "&" } else { "*" };
                let mut t = alloc::format!("{cv}{debajo} {marca}{ptr64}");
                if c == b'Q' {
                    t.push_str(" const");
                }
                Some(t)
            }
            _ => None,
        }
    }
}

/// El nombre de una funcion especial (`??0` constructor, `??1`
/// destructor...), o None.
fn especial(l: &mut Lector) -> Option<String> {
    let c = l.tomar()?;
    Some(String::from(match c {
        b'0' | b'1' => {
            // Constructor o destructor: el nombre de su clase.
            let resto = l.calificado()?;
            let clase = String::from(resto.rsplit("::").next().unwrap_or(""));
            let nombre = if c == b'0' { clase } else { alloc::format!("~{clase}") };
            return Some(alloc::format!("{resto}::{nombre}"));
        }
        b'2' => "operator new",
        b'3' => "operator delete",
        b'4' => "operator=",
        b'8' => "operator==",
        b'9' => "operator!=",
        b'A' => "operator[]",
        b'R' => "operator()",
        b'_' => match l.tomar()? {
            b'7' => "`vftable'",
            b'8' => "`vbtable'",
            b'U' => "operator new[]",
            b'V' => "operator delete[]",
            _ => return None,
        },
        _ => return None,
    }))
}

/// **Des-decorar** (lo que hace __unDName): un tipo ("?AV...", ".?AV..."),
/// o el nombre calificado de una funcion o variable ("?f@A@@..."). `None`
/// si no se entiende.
pub(crate) fn des_decorar(s: &[u8]) -> Option<String> {
    let s = s.strip_prefix(b".").unwrap_or(s);
    let s = s.strip_prefix(b"?")?;
    let mut l = Lector { s, i: 0, nombres: Vec::new() };
    if l.ver()? == b'A' && matches!(s.get(1), Some(b'V' | b'U' | b'T' | b'W')) {
        l.i += 1;
        return l.tipo();
    }
    if l.ver()? == b'?' {
        l.i += 1;
        let e = especial(&mut l)?;
        if e.contains("::") {
            return Some(e);
        }
        let resto = l.calificado()?;
        return Some(if resto.is_empty() { e } else { alloc::format!("{resto}::{e}") });
    }
    l.calificado()
}

/// **`__unDName(salida, decorado, largo, reservar, soltar, banderas)`**: el
/// nombre en `salida` (o en un bloque de `reservar`, si no la dan). NULL si
/// no se entiende.
extern "win64" fn un_d_name(salida: *mut u8, decorado: *const u8, largo: i32, reservar: u64, _soltar: u64, _banderas: u16) -> u64 {
    if decorado.is_null() {
        return 0;
    }
    let s = crate::crt::cadena_c(decorado as u64);
    let Some(t) = des_decorar(&s) else { return 0 };
    let (d, cabe) = if salida.is_null() {
        if reservar == 0 {
            return 0;
        }
        // SAFETY: la funcion de reservar del `.exe` (como malloc).
        let p = unsafe { crate::hilos::llamar_win64(reservar, t.len() as u64 + 1, 0, 0) };
        (p as *mut u8, t.len() + 1)
    } else {
        (salida, largo.max(0) as usize)
    };
    if d.is_null() || cabe == 0 {
        return 0;
    }
    let n = t.len().min(cabe - 1);
    // SAFETY: `cabe` bytes del `.exe` (o del bloque recien reservado).
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), d, n);
        d.add(n).write(0);
    }
    d as u64
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn un_d_name_ex(salida: *mut u8, decorado: *const u8, largo: i32, reservar: u64, soltar: u64, _param: u64, banderas: u32) -> u64 {
    un_d_name(salida, decorado, largo, reservar, soltar, banderas as u16)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "__unDName" => dir!(un_d_name),
        "__unDNameEx" => dir!(un_d_name_ex),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn tipos_como_type_info_name() {
        let d = |s: &str| des_decorar(s.as_bytes());
        assert_eq!(d(".?AVHija@@").as_deref(), Some("class Hija"));
        assert_eq!(d("?AUBase@@").as_deref(), Some("struct Base"));
        assert_eq!(d("?AVB@A@@").as_deref(), Some("class A::B"));
        assert_eq!(d("?AV?$Caja@H@@").as_deref(), Some("class Caja<int>"));
        assert_eq!(d("?AV?$vector@HV?$allocator@H@std@@@std@@").as_deref(), Some("class std::vector<int,class std::allocator<int> >"));
        assert_eq!(d("?AV?$Fija@$02@@").as_deref(), Some("class Fija<3>"));
        assert_eq!(d(".PEAUB@@").as_deref(), None, "un puntero no es ?A: no es un tipo con nombre");
    }

    #[test]
    fn nombres_de_funciones() {
        let d = |s: &str| des_decorar(s.as_bytes());
        assert_eq!(d("?f@@YAHH@Z").as_deref(), Some("f"));
        assert_eq!(d("?que@Hija@@UEBAHXZ").as_deref(), Some("Hija::que"));
        assert_eq!(d("??0Hija@@QEAA@XZ").as_deref(), Some("Hija::Hija"));
        assert_eq!(d("??1Hija@@UEAA@XZ").as_deref(), Some("Hija::~Hija"));
        assert_eq!(d("??_7type_info@@6B@").as_deref(), Some("type_info::`vftable'"));
    }
}
