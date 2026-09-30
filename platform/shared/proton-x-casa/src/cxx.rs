//! **Las excepciones de C++ de MSVC, en la casa** (29-09): `throw` y
//! `catch` de un `.exe` de Windows (y el `panic` de Rust, que va por lo
//! mismo), sobre las excepciones estructuradas de `excepciones.rs`.
//!
//! ```text
//!    _CxxThrowException(objeto, ThrowInfo)
//!        RaiseException(0xE06D7363, NO_CONTINUABLE, [0x19930520, objeto,
//!        ThrowInfo, base de su imagen]); `throw;` (los dos a 0) relanza la
//!        que se esta cogiendo
//!    __CxxFrameHandler3   el manejador de cada funcion con try o destructores
//!        buscar     el estado del marco (ip -> estado), los try que lo
//!                   contienen, y en cada uno sus catch: el tipo que casa
//!                   (por nombre, referencia, const) o catch(...)
//!        coger      desenrollar hasta ese marco (`excepciones::desenrollar_y`)
//!                   y ALLI: el objeto del catch (copia, referencia o puntero,
//!                   con el ajuste a la base), el funclet del catch (rdx = el
//!                   marco de la funcion), destruir la excepcion, y seguir
//!                   donde el funclet diga
//!        desenrollar  los destructores del marco (el mapa de estados), de su
//!                   estado hasta el del try (o hasta -1)
//! ```
//!
//! Las tablas (FuncInfo, UnwindMap, TryBlockMap, HandlerType, IPtoStateMap,
//! ThrowInfo, CatchableType, TypeDescriptor) son el formato publico de MSVC
//! x64 que tambien emite clang: RVAs sobre la base de su imagen.
//!
//! **Un catch dentro de otro:** mientras corre el funclet de un catch, el
//! marco de su funcion sigue "en el catch": un `throw;` o una excepcion nueva
//! no la vuelve a coger ese mismo try (`Captura`), y el despacho sube por el
//! marco de la funcion (`excepciones::poner_salto`) al acabarse la pila en
//! la casa.
//!
//! **`__CxxFrameHandler4`** (30-09): el formato comprimido de MSVC 2019 y
//! despues, el de Cyberpunk. Sus tablas las lee `cxx4` a la misma forma
//! plana (`Tablas`), y el manejador es este mismo; lo suyo: destructores
//! directos en el mapa de estados, funclets de catch con sus propias tablas,
//! y continuaciones en la tabla (el funclet devuelve el indice).
//!
//! **Lo que NO hace, dicho:** el FH4 de codigo separado en trozos (/hotpatch,
//! BBT); las especificaciones `throw()` y `noexcept` no llaman a
//! `std::terminate` (la excepcion sigue subiendo); /EHa (un catch(...) que
//! coge excepciones de SEH) tampoco.

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::desenrollar::{Contexto, Memoria, CONTEXT_BYTES};
use bmo_proton_x::seh::{self, Despacho, Registro, DESPACHO_BYTES, REGISTRO_BYTES};

use crate::excepciones::{self, Viva};
use crate::{aviso, dir, hilos, kernel32, plataforma};

/// El codigo de una excepcion de C++ ('msc' | 0xE0000000).
pub(crate) const EXCEPCION_CXX: u32 = 0xE06D_7363;
/// El primer parametro: la "magia" de MSVC.
const MAGIA_TIRAR: u64 = 0x1993_0520;
const DISPOSICION_BUSCAR: u32 = 1;
/// Los FuncInfo que se entienden (0x19930520..22; los 3 bits de arriba son
/// banderas).
const MAGIA_MIN: u32 = 0x1993_0520;
const MAGIA_MAX: u32 = 0x1993_0522;

/// HandlerType.adjectives.
const HT_CONST: u32 = 1;
const HT_VOLATIL: u32 = 2;
const HT_REFERENCIA: u32 = 8;
const HT_PUNTOS: u32 = 0x40;
/// ThrowInfo.attributes.
const TI_CONST: u32 = 1;
const TI_VOLATIL: u32 = 2;
/// CatchableType.properties.
const CT_SIMPLE: u32 = 1;
const CT_SOLO_REFERENCIA: u32 = 2;
const CT_BASE_VIRTUAL: u32 = 4;

// -- El estado --------------------------------------------------------------------

/// Una excepcion de C++ en vuelo (lanzada, aun no acabada de coger).
#[derive(Clone, Copy)]
struct EnCurso {
    teb: u64,
    objeto: u64,
    info: u64,
    base: u64,
}

/// Un catch que esta corriendo: el marco de su funcion y su try.
#[derive(Clone, Copy)]
struct Captura {
    teb: u64,
    /// El marco (establisher) de la funcion del try.
    padre: u64,
    /// El FuncInfo y el indice del try.
    funcion: u64,
    intento: u32,
    /// El objeto que se cogio.
    objeto: u64,
    /// Un sitio de la pila del que la corre: por debajo de rsp, esta muerta.
    marca: u64,
}

/// Lo que va a coger un despacho: el marco destino y hasta que estado se
/// desenrolla en el.
#[derive(Clone, Copy)]
struct Pendiente {
    teb: u64,
    marco: u64,
    estado: i32,
}

struct Estado {
    en_curso: Vec<EnCurso>,
    capturas: Vec<Captura>,
    pendientes: Vec<Pendiente>,
    /// `__current_exception` y `__current_exception_context`: lo que se coge.
    registro: u64,
    contexto: u64,
}

struct Global(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y ninguna
// referencia al estado cruza una llamada al `.exe` (ver `con`).
unsafe impl Sync for Global {}
static ESTADO: Global = Global(UnsafeCell::new(Estado { en_curso: Vec::new(), capturas: Vec::new(), pendientes: Vec::new(), registro: 0, contexto: 0 }));

fn con<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    // SAFETY: ver `Global`: `f` no llama al `.exe`.
    f(unsafe { &mut *ESTADO.0.get() })
}

pub(crate) fn reiniciar() {
    con(|e| {
        e.en_curso.clear();
        e.capturas.clear();
        e.pendientes.clear();
        e.registro = 0;
        e.contexto = 0;
    });
}

fn rsp_ahora() -> u64 {
    let r: u64;
    // SAFETY: leer un registro.
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) r, options(nomem, nostack, preserves_flags)) };
    r
}

/// Las capturas (y los saltos) de ESTE hilo por debajo de `rsp`: catch
/// abandonados cuya pila ya es de otro. Se poda al coger, con el rsp del
/// marco que coge: todo lo de debajo esta muerto.
fn podar(rsp: u64) {
    let t = kernel32::teb();
    con(|e| e.capturas.retain(|c| c.teb != t || c.marca > rsp));
    excepciones::podar_saltos(rsp);
}

// -- Leer las tablas ------------------------------------------------------------------

fn u32_en(m: &Viva, d: u64) -> Option<u32> {
    m.u32_en(d)
}

fn i32_en(m: &Viva, d: u64) -> Option<i32> {
    m.u32_en(d).map(|v| v as i32)
}

/// FuncInfo (FH3, x64).
#[derive(Clone, Copy)]
struct FuncInfo {
    base: u64,
    mapa: u64,
    n_intentos: u32,
    intentos: u64,
    n_ips: u32,
    ips: u64,
}

impl FuncInfo {
    fn leer(m: &Viva, base: u64, d: u64) -> Option<FuncInfo> {
        let magia = u32_en(m, d)? & 0x1FFF_FFFF;
        if !(MAGIA_MIN..=MAGIA_MAX).contains(&magia) {
            return None;
        }
        let rva = |o: u64| u32_en(m, d + o).map(|r| base + r as u64);
        Some(FuncInfo { base, mapa: rva(8)?, n_intentos: u32_en(m, d + 12)?, intentos: rva(16)?, n_ips: u32_en(m, d + 20)?, ips: rva(24)? })
    }

    /// El estado de `pc`: el de la ultima fila con ip <= pc.
    fn estado(&self, m: &Viva, pc: u64) -> i32 {
        let pc = pc.wrapping_sub(self.base);
        let mut s = -1;
        for k in 0..self.n_ips as u64 {
            let Some(ip) = u32_en(m, self.ips + 8 * k) else { break };
            if ip as u64 > pc {
                break;
            }
            s = i32_en(m, self.ips + 8 * k + 4).unwrap_or(-1);
        }
        s
    }

    /// UnwindMap[s]: (toState, action).
    fn desenrollar(&self, m: &Viva, s: i32) -> Option<(i32, Accion)> {
        let d = self.mapa + 8 * s as u64;
        let accion = u32_en(m, d + 4)?;
        Some((i32_en(m, d)?, if accion == 0 { Accion::Nada } else { Accion::Funclet(self.base + accion as u64) }))
    }

    fn intento(&self, m: &Viva, k: u32) -> Option<Intento> {
        let d = self.intentos + 20 * k as u64;
        Some(Intento { bajo: i32_en(m, d)?, alto: i32_en(m, d + 4)?, n_catches: u32_en(m, d + 12)?, catches: self.base + u32_en(m, d + 16)? as u64 })
    }

    /// HandlerType k de un try.
    fn catch(&self, m: &Viva, t: &Intento, k: u32) -> Option<Catch> {
        let d = t.catches + 20 * k as u64;
        let tipo = u32_en(m, d + 4)?;
        Some(Catch {
            adjetivos: u32_en(m, d)?,
            tipo: if tipo == 0 { 0 } else { self.base + tipo as u64 },
            objeto: i32_en(m, d + 8)?,
            funclet: self.base + u32_en(m, d + 12)? as u64,
            marco: u32_en(m, d + 16)?,
            seguir: [0; 2],
            cuantas_seguir: 0,
        })
    }
}

#[derive(Clone, Copy)]
struct Intento {
    bajo: i32,
    alto: i32,
    n_catches: u32,
    catches: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct Catch {
    pub(crate) adjetivos: u32,
    /// El TypeDescriptor, o 0 (catch(...)).
    pub(crate) tipo: u64,
    /// Donde va el objeto del catch, desde el marco de la funcion (0: sin).
    pub(crate) objeto: i32,
    pub(crate) funclet: u64,
    /// Donde guarda el funclet el marco de su funcion, desde el suyo (FH3;
    /// en FH4 lo dice el FuncInfo4 del funclet).
    pub(crate) marco: u32,
    /// FH4: las continuaciones de la tabla (el funclet devuelve el indice);
    /// sin ellas (FH3), el funclet devuelve la direccion.
    pub(crate) seguir: [u64; 2],
    pub(crate) cuantas_seguir: u8,
}

/// Lo que hace una fila del mapa de estados al desenrollar.
#[derive(Clone, Copy)]
pub(crate) enum Accion {
    Nada,
    /// Un funclet de limpieza: rdx = el marco de la funcion.
    Funclet(u64),
    /// FH4: el destructor (this = el marco + el desplazamiento).
    Destructor(u64, u32),
    /// FH4: el destructor de lo que apunta el puntero de (el marco + el
    /// desplazamiento).
    DestructorDePuntero(u64, u32),
}

/// **Las tablas de una funcion**, de FH3 (en la memoria, tal cual) o de FH4
/// (leidas y descomprimidas: `cxx4`).
enum Tablas {
    Fh3(FuncInfo),
    Fh4(crate::cxx4::Fh4, u64),
}

impl Tablas {
    fn clave(&self) -> u64 {
        match self {
            Tablas::Fh3(f) => f.mapa,
            Tablas::Fh4(f, _) => f.clave,
        }
    }

    fn estado(&self, m: &Viva, pc: u64) -> i32 {
        match self {
            Tablas::Fh3(f) => f.estado(m, pc),
            Tablas::Fh4(f, inicio) => f.estado(pc, *inicio),
        }
    }

    fn desenrollar(&self, m: &Viva, s: i32) -> Option<(i32, Accion)> {
        match self {
            Tablas::Fh3(f) => f.desenrollar(m, s),
            Tablas::Fh4(f, _) => f.mapa.get(usize::try_from(s).ok()?).copied(),
        }
    }

    fn n_intentos(&self) -> u32 {
        match self {
            Tablas::Fh3(f) => f.n_intentos,
            Tablas::Fh4(f, _) => f.intentos.len() as u32,
        }
    }

    /// El try `k` (en FH4, `catches` es su indice).
    fn intento(&self, m: &Viva, k: u32) -> Option<Intento> {
        match self {
            Tablas::Fh3(f) => f.intento(m, k),
            Tablas::Fh4(f, _) => f.intentos.get(k as usize).map(|t| Intento { bajo: t.bajo, alto: t.alto, n_catches: t.catches.len() as u32, catches: k as u64 }),
        }
    }

    fn catch(&self, m: &Viva, t: &Intento, j: u32) -> Option<Catch> {
        match self {
            Tablas::Fh3(f) => f.catch(m, t, j),
            Tablas::Fh4(f, _) => f.intentos.get(t.catches as usize)?.catches.get(j as usize).copied(),
        }
    }
}

/// El nombre decorado de un TypeDescriptor (tras vftable y spare).
fn mismo_nombre(a: u64, b: u64) -> bool {
    if a == b {
        return true;
    }
    let (m, _) = Viva::de_ahora();
    for k in 0..256 {
        match (m.u8_en(a + 16 + k), m.u8_en(b + 16 + k)) {
            (Some(x), Some(y)) if x == y => {
                if x == 0 {
                    return true;
                }
            }
            _ => return false,
        }
    }
    false
}

/// CatchableType.
#[derive(Clone, Copy)]
struct Cogible {
    propiedades: u32,
    tipo: u64,
    mdisp: i32,
    pdisp: i32,
    vdisp: i32,
    tam: u32,
    copia: u64,
}

impl Cogible {
    fn leer(m: &Viva, base: u64, d: u64) -> Option<Cogible> {
        let copia = u32_en(m, d + 24)?;
        Some(Cogible {
            propiedades: u32_en(m, d)?,
            tipo: base + u32_en(m, d + 4)? as u64,
            mdisp: i32_en(m, d + 8)?,
            pdisp: i32_en(m, d + 12)?,
            vdisp: i32_en(m, d + 16)?,
            tam: u32_en(m, d + 20)?,
            copia: if copia == 0 { 0 } else { base + copia as u64 },
        })
    }

    /// `AdjustPointer`: del objeto lanzado a la base que coge el catch.
    fn ajustar(&self, p: u64) -> u64 {
        let mut r = p.wrapping_add(self.mdisp as i64 as u64);
        if self.pdisp >= 0 {
            // SAFETY: el objeto lanzado (vivo) y su tabla de bases virtuales.
            unsafe {
                let vbt = ((p + self.pdisp as u64) as *const u64).read_unaligned();
                let d = ((vbt + self.vdisp as u64) as *const i32).read_unaligned();
                r = r.wrapping_add(self.pdisp as i64 as u64).wrapping_add(d as i64 as u64);
            }
        }
        r
    }
}

/// ThrowInfo.
#[derive(Clone, Copy)]
struct Tirada {
    atributos: u32,
    destructor: u64,
    cogibles: u64,
    base: u64,
}

impl Tirada {
    fn leer(m: &Viva, base: u64, d: u64) -> Option<Tirada> {
        let dtor = u32_en(m, d + 4)?;
        Some(Tirada { atributos: u32_en(m, d)?, destructor: if dtor == 0 { 0 } else { base + dtor as u64 }, cogibles: base + u32_en(m, d + 12)? as u64, base })
    }

    /// El CatchableType que casa con el catch `c`, si alguno.
    fn casa_con(&self, m: &Viva, c: &Catch) -> Option<Cogible> {
        let n = u32_en(m, self.cogibles)?;
        for k in 0..n.min(64) as u64 {
            let ct = Cogible::leer(m, self.base, self.base + u32_en(m, self.cogibles + 4 + 4 * k)? as u64)?;
            if !mismo_nombre(ct.tipo, c.tipo) {
                continue;
            }
            if ct.propiedades & CT_SOLO_REFERENCIA != 0 && c.adjetivos & HT_REFERENCIA == 0 {
                continue;
            }
            if (self.atributos & TI_CONST != 0 && c.adjetivos & HT_CONST == 0) || (self.atributos & TI_VOLATIL != 0 && c.adjetivos & HT_VOLATIL == 0) {
                continue;
            }
            return Some(ct);
        }
        None
    }
}

// -- Lanzar ---------------------------------------------------------------------------

/// Los cuatro parametros de la excepcion, para RaiseException (que los copia
/// al entrar, antes de que otro hilo pueda lanzar: los hilos son
/// cooperativos).
static mut PARAMETROS: [u64; 4] = [0; 4];

/// Las tablas de C++ que hace la casa (msvcp_errores: los ThrowInfo de
/// std::length_error...): la base de sus RVAs (por debajo de ellas Y del
/// codigo que nombran: las RVAs no tienen signo), y donde estan.
static mut TABLAS_CASA: (u64, u64, u64) = (0, 0, 0);

pub(crate) fn tablas_de_la_casa(base: u64, desde: u64, hasta: u64) {
    // SAFETY: una tarea, hilos cooperativos; se pone una vez.
    unsafe { TABLAS_CASA = (base, desde, hasta) };
}

/// Donde estan esas tablas, para leerlas (`Viva`).
pub(crate) fn rango_casa() -> Option<(u64, u64)> {
    // SAFETY: ver TABLAS_CASA.
    let (_, d, h) = unsafe { TABLAS_CASA };
    (d != 0).then_some((d, h))
}

/// La base de la imagen donde cae `d` (el ThrowInfo), o 0.
pub(crate) fn base_de(d: u64) -> u64 {
    // SAFETY: ver TABLAS_CASA.
    let (b, desde, hasta) = unsafe { TABLAS_CASA };
    if d >= desde && d < hasta {
        return b;
    }
    let (_, imagenes) = Viva::de_ahora();
    seh::imagen_de(&imagenes, d).map_or(0, |i| i.base)
}

/// Lo que hace `_CxxThrowException` antes de RaiseException: la excepcion en
/// vuelo (o la que se coge, si es `throw;`) y sus parametros.
extern "win64" fn preparar(objeto: u64, info: u64) -> *const u64 {
    let t = kernel32::teb();
    let (objeto, info, base) = if objeto == 0 && info == 0 {
        let cogida = con(|e| e.capturas.iter().rev().find(|c| c.teb == t).map(|c| c.objeto));
        match cogida.and_then(|o| con(|e| e.en_curso.iter().rev().find(|x| x.teb == t && x.objeto == o).copied())) {
            Some(x) => (x.objeto, x.info, x.base),
            None => {
                aviso("throw; sin una excepcion de C++ que se este cogiendo: std::terminate");
                (plataforma().salir)(3)
            }
        }
    } else {
        let base = base_de(info);
        con(|e| e.en_curso.push(EnCurso { teb: t, objeto, info, base }));
        (objeto, info, base)
    };
    // SAFETY: ver PARAMETROS.
    unsafe {
        let p = core::ptr::addr_of_mut!(PARAMETROS);
        *p = [MAGIA_TIRAR, objeto, info, base];
        p as *const u64
    }
}

core::arch::global_asm!(
    // _CxxThrowException(objeto, ThrowInfo): los parametros, y SALTAR a
    // RaiseException, para que la foto sea la de quien lanza (el `.exe`).
    ".globl proton_x_cxx_throw",
    "proton_x_cxx_throw:",
    "sub rsp, 40",
    "call {preparar}",
    "add rsp, 40",
    "mov r9, rax",
    "mov ecx, 0xE06D7363",
    "mov edx, 1",
    "mov r8d, 4",
    "jmp proton_x_raise_exception",
    preparar = sym preparar,
);

extern "C" {
    pub(crate) fn proton_x_cxx_throw();
}

// -- El manejador ---------------------------------------------------------------------

/// Lo que se sabe de un marco al manejarlo.
struct Marco {
    fi: Tablas,
    /// El marco de la funcion (el del padre si este es un funclet de catch).
    padre: u64,
    /// El estado efectivo.
    estado: i32,
    /// Si es el funclet de un catch: su try.
    funclet_de: Option<u32>,
    /// Si la funcion esta en un catch de uno de sus try (otro despacho).
    captura: Option<Captura>,
}

fn marco(m: &Viva, d: &Despacho, cuatro: bool) -> Option<Marco> {
    let datos = d.base_imagen + u32_en(m, d.datos)? as u64;
    let inicio = u32_en(m, d.funcion)? as u64 + d.base_imagen;
    let fi = if cuatro {
        let f = crate::cxx4::Fh4::leer(m, d.base_imagen, datos, inicio)?;
        // FH4: el funclet de un catch tiene SUS tablas y dice donde guardo
        // el marco de su funcion.
        if let Some(desp) = f.marco_del_padre {
            let padre = m.u64_en(d.establecido + desp as u64)?;
            let estado = f.estado(d.pc, inicio);
            return Some(Marco { fi: Tablas::Fh4(f, inicio), padre, estado, funclet_de: Some(0), captura: None });
        }
        Tablas::Fh4(f, inicio)
    } else {
        let fi = FuncInfo::leer(m, d.base_imagen, datos)?;
        // FH3: es el funclet de un catch si empieza donde el handler de alguno.
        for k in 0..fi.n_intentos.min(4096) {
            let t = fi.intento(m, k)?;
            for j in 0..t.n_catches.min(64) {
                let c = fi.catch(m, &t, j)?;
                if c.funclet == inicio {
                    let padre = m.u64_en(d.establecido + c.marco as u64)?;
                    return Some(Marco { fi: Tablas::Fh3(fi), padre, estado: fi.estado(m, d.pc), funclet_de: Some(k), captura: None });
                }
            }
        }
        Tablas::Fh3(fi)
    };
    let t = kernel32::teb();
    let clave = fi.clave();
    let captura = con(|e| e.capturas.iter().rev().find(|c| c.teb == t && c.padre == d.establecido && c.funcion == clave).copied());
    let estado = match captura {
        // Esta en el catch de su try: fuera de el (el estado de antes del try).
        Some(c) => fi.intento(m, c.intento).and_then(|t| fi.desenrollar(m, t.bajo)).map_or(-1, |(a, _)| a),
        None => fi.estado(m, d.pc),
    };
    Some(Marco { fi, padre: d.establecido, estado, funclet_de: None, captura })
}

/// Correr los destructores del marco de `desde` hasta `hasta` (sin incluir).
fn destructores(m: &Viva, mc: &Marco, desde: i32, hasta: i32) {
    let mut s = desde;
    let mut vueltas = 0;
    while s > hasta && s >= 0 && vueltas < 100_000 {
        let Some((siguiente, accion)) = mc.fi.desenrollar(m, s) else {
            aviso("__CxxFrameHandler: el mapa de estados no se deja leer");
            return;
        };
        // SAFETY: lo que el mapa de estados del `.exe` dice que se corra.
        unsafe {
            match accion {
                Accion::Nada => 0,
                // El funclet de limpieza: rdx = el marco.
                Accion::Funclet(f) => hilos::llamar_win64(f, 0, mc.padre, 0),
                // El destructor de un objeto del marco: this = su sitio.
                Accion::Destructor(f, o) => hilos::llamar_win64(f, mc.padre + o as u64, 0, 0),
                Accion::DestructorDePuntero(f, o) => match m.u64_en(mc.padre + o as u64) {
                    Some(p) => hilos::llamar_win64(f, p, 0, 0),
                    None => 0,
                },
            }
        };
        s = siguiente;
        vueltas += 1;
    }
}

/// Destruir (si hay destructor) la excepcion en vuelo `objeto` y olvidarla.
fn acabar_con(objeto: u64) {
    let t = kernel32::teb();
    let x = con(|e| {
        let i = e.en_curso.iter().rposition(|x| x.teb == t && x.objeto == objeto)?;
        Some(e.en_curso.remove(i))
    });
    if let Some(x) = x {
        let (m, _) = Viva::de_ahora();
        if let Some(ti) = Tirada::leer(&m, x.base, x.info) {
            if ti.destructor != 0 {
                // SAFETY: el destructor del tipo lanzado: void f(this).
                unsafe { hilos::llamar_win64(ti.destructor, x.objeto, 0, 0) };
            }
        }
    }
}

/// **`__CxxFrameHandler3`** (rec, marco, contexto, DISPATCHER_CONTEXT).
pub(crate) extern "win64" fn cxx_frame_handler3(rec: *mut u8, establecido: u64, ctx: *mut u8, dc: *mut u8) -> u32 {
    manejar(rec, establecido, ctx, dc, false)
}

/// **`__CxxFrameHandler4`**: el mismo, con las tablas comprimidas (`cxx4`).
pub(crate) extern "win64" fn cxx_frame_handler4(rec: *mut u8, establecido: u64, ctx: *mut u8, dc: *mut u8) -> u32 {
    manejar(rec, establecido, ctx, dc, true)
}

fn manejar(rec: *mut u8, establecido: u64, ctx: *mut u8, dc: *mut u8, cuatro: bool) -> u32 {
    // SAFETY: los del despachador de la casa, con sus medidas.
    let r = Registro::de_bytes(unsafe { core::slice::from_raw_parts(rec, REGISTRO_BYTES) });
    let d = Despacho::de_bytes(unsafe { core::slice::from_raw_parts(dc, DESPACHO_BYTES) });
    let (m, _) = Viva::de_ahora();
    let Some(mc) = marco(&m, &d, cuatro) else {
        aviso(if cuatro { "__CxxFrameHandler4: el FuncInfo4 no se entiende" } else { "__CxxFrameHandler3: el FuncInfo no se entiende" });
        return DISPOSICION_BUSCAR;
    };
    let t = kernel32::teb();
    if r.banderas & seh::EXCEPTION_UNWINDING != 0 {
        let hasta = if r.banderas & seh::EXCEPTION_TARGET_UNWIND != 0 {
            con(|e| e.pendientes.iter().rev().find(|p| p.teb == t && p.marco == establecido).map(|p| p.estado)).unwrap_or(-1)
        } else {
            -1
        };
        destructores(&m, &mc, mc.estado, hasta);
        // Un catch que se abandona por una excepcion NUEVA: la suya ya no se
        // acabara de coger nunca.
        let lanzado = r.parametros.get(1).copied().unwrap_or(0);
        if mc.funclet_de.is_some() || mc.captura.is_some() {
            let muertas: Vec<u64> = con(|e| e.capturas.iter().filter(|c| c.teb == t && c.padre == mc.padre).map(|c| c.objeto).collect());
            for o in muertas {
                if r.codigo != EXCEPCION_CXX || o != lanzado {
                    acabar_con(o);
                }
            }
            if r.banderas & seh::EXCEPTION_TARGET_UNWIND == 0 {
                con(|e| e.capturas.retain(|c| !(c.teb == t && c.padre == mc.padre)));
            }
        }
        return DISPOSICION_BUSCAR;
    }
    if r.codigo != EXCEPCION_CXX || r.parametros.len() < 3 || r.parametros[0] & 0xFFFF_FFF0 != MAGIA_TIRAR & 0xFFFF_FFF0 {
        return DISPOSICION_BUSCAR;
    }
    let (objeto, info) = (r.parametros[1], r.parametros[2]);
    let base = r.parametros.get(3).copied().unwrap_or_else(|| base_de(info));
    let Some(ti) = Tirada::leer(&m, base, info) else {
        aviso("_CxxThrowException: el ThrowInfo no se deja leer");
        return DISPOSICION_BUSCAR;
    };
    for k in 0..mc.fi.n_intentos().min(4096) {
        let Some(it) = mc.fi.intento(&m, k) else { break };
        if mc.estado < it.bajo || mc.estado > it.alto {
            continue;
        }
        for j in 0..it.n_catches.min(64) {
            let Some(c) = mc.fi.catch(&m, &it, j) else { break };
            let todos = c.tipo == 0 || c.adjetivos & HT_PUNTOS != 0;
            let cogible = if todos { None } else { ti.casa_con(&m, &c) };
            if !todos && cogible.is_none() {
                continue;
            }
            // ** Este catch la coge.
            let hasta = mc.fi.desenrollar(&m, it.bajo).map_or(-1, |(a, _)| a);
            con(|e| e.pendientes.push(Pendiente { teb: t, marco: establecido, estado: hasta }));
            // SAFETY: el CONTEXT de la excepcion (la foto de RaiseException).
            let inicio = Contexto::de_context(unsafe { core::slice::from_raw_parts(ctx, CONTEXT_BYTES) });
            let (padre, funcion, intento) = (mc.padre, mc.fi.clave(), k);
            excepciones::desenrollar_y(establecido, rec, inicio, &mut |fin: &mut Contexto| {
                con(|e| e.pendientes.retain(|p| !(p.teb == t && p.marco == establecido)));
                fin.rip = coger(rec, fin, padre, funcion, intento, &c, cogible, objeto);
            });
        }
    }
    DISPOSICION_BUSCAR
}

/// **Coger**: el objeto del catch, su funclet y la excepcion destruida.
/// Devuelve donde sigue el `.exe` (lo que devuelve el funclet).
#[allow(clippy::too_many_arguments)]
fn coger(rec: *mut u8, fin: &Contexto, padre: u64, funcion: u64, intento: u32, c: &Catch, cogible: Option<Cogible>, objeto: u64) -> u64 {
    podar(fin.gp[bmo_proton_x::desenrollar::RSP]);
    if let (Some(ct), true) = (cogible, c.objeto != 0) {
        let dest = padre.wrapping_add(c.objeto as i64 as u64);
        // SAFETY: el hueco del objeto del catch en el marco de la funcion
        // (vivo: es el marco destino) y el objeto lanzado (vivo, en la pila
        // de debajo, que nadie ha pisado todavia).
        unsafe {
            if c.adjetivos & HT_REFERENCIA != 0 {
                (dest as *mut u64).write_unaligned(ct.ajustar(objeto));
            } else if ct.propiedades & CT_SIMPLE != 0 {
                core::ptr::copy(objeto as *const u8, dest as *mut u8, ct.tam as usize);
                if ct.tam == 8 {
                    let p = (dest as *const u64).read_unaligned();
                    if p != 0 {
                        (dest as *mut u64).write_unaligned(ct.ajustar(p));
                    }
                }
            } else {
                let fuente = ct.ajustar(objeto);
                if ct.copia == 0 {
                    core::ptr::copy(fuente as *const u8, dest as *mut u8, ct.tam as usize);
                } else if ct.propiedades & CT_BASE_VIRTUAL != 0 {
                    hilos::llamar_win64(ct.copia, dest, fuente, 1);
                } else {
                    hilos::llamar_win64(ct.copia, dest, fuente, 0);
                }
            }
        }
    }
    let t = kernel32::teb();
    let marca = rsp_ahora();
    con(|e| e.capturas.push(Captura { teb: t, padre, funcion, intento, objeto, marca }));
    // El marco de la funcion, para que el despacho de un `throw;` suba por el.
    #[repr(C, align(16))]
    struct Foto([u8; CONTEXT_BYTES]);
    let mut foto = Foto([0; CONTEXT_BYTES]);
    fin.a_context(&mut foto.0);
    let (antes_r, antes_c) = con(|e| (e.registro, e.contexto));
    con(|e| {
        e.registro = rec as u64;
        e.contexto = foto.0.as_ptr() as u64;
    });
    excepciones::poner_salto(marca, foto.0.as_ptr() as u64);
    // SAFETY: el funclet del catch: rdx = el marco de su funcion; devuelve
    // donde seguir.
    let devuelto = unsafe { hilos::llamar_win64(c.funclet, 0, padre, 0) };
    // FH4 con continuaciones en la tabla: el funclet dice CUAL (0 o 1).
    let seguir = match c.cuantas_seguir {
        0 => devuelto,
        n if devuelto < n as u64 => c.seguir[devuelto as usize],
        _ => {
            aviso("__CxxFrameHandler4: el funclet del catch devolvio una continuacion que no hay");
            c.seguir[0]
        }
    };
    excepciones::soltar_salto(foto.0.as_ptr() as u64);
    con(|e| {
        e.capturas.retain(|x| x.marca != marca);
        e.registro = antes_r;
        e.contexto = antes_c;
    });
    acabar_con(objeto);
    seguir
}

// -- Lo demas que se exporta ----------------------------------------------------------

/// `__current_exception`: donde esta el EXCEPTION_RECORD* de la que se coge.
pub(crate) extern "win64" fn current_exception() -> u64 {
    con(|e| core::ptr::addr_of_mut!(e.registro) as u64)
}

/// `__current_exception_context`: donde esta su CONTEXT*.
extern "win64" fn current_exception_context() -> u64 {
    con(|e| core::ptr::addr_of_mut!(e.contexto) as u64)
}

/// `__uncaught_exceptions`: las lanzadas que aun no ha cogido nadie (las que
/// se estan cogiendo no cuentan).
pub(crate) extern "win64" fn uncaught_exceptions() -> i32 {
    let t = kernel32::teb();
    con(|e| e.en_curso.iter().filter(|x| x.teb == t && !e.capturas.iter().any(|c| c.teb == t && c.objeto == x.objeto)).count() as i32)
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "_CxxThrowException" => dir!(proton_x_cxx_throw),
        "__CxxFrameHandler3" => dir!(cxx_frame_handler3),
        "__CxxFrameHandler4" => dir!(cxx_frame_handler4),
        "__current_exception" => dir!(current_exception),
        "__current_exception_context" => dir!(current_exception_context),
        "__uncaught_exceptions" => dir!(uncaught_exceptions),
        _ => return None,
    })
}
