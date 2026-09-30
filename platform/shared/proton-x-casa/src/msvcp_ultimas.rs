//! **Las ultimas DURAS de msvcp140** (tanda 20 de Cyberpunk, 30-09): las
//! que salieron en el censo del metal cuando la casa ya tenia las 79 (las
//! piden las DLL del juego que ahora se leen enteras).
//!
//! ```text
//!    ostream     << float (como num_put: %g, %f o %e con la precision y las
//!                banderas), << const void* (%p: 16 cifras), << long long
//!    istream     read, seekg(off, way), tellg
//!    y lo demas  setprecision, _Fiopen de un nombre ancho y el constructor
//!                de Concurrency::task_continuation_context
//! ```

use alloc::vec::Vec;

use crate::dir;
use crate::msvcp_flujos::{self, Ios, Sb};

const EOFBIT: i32 = 1;
const FAILBIT: i32 = 2;

// -- ostream -----------------------------------------------------------------------------------

/// El texto de un numero ya hecho, con el ancho y el relleno del ios (a la
/// izquierda, por dentro tras el signo, o a la derecha); el ancho se gasta.
fn con_ancho(os: *mut u8, t: &[u8]) -> *mut u8 {
    // SAFETY: el ios de un ostream.
    let i = unsafe { &mut *msvcp_flujos::ios_de(os) };
    let falta = (i.ancho.max(0) as usize).saturating_sub(t.len());
    let relleno = alloc::vec![i.relleno; falta];
    let signo = usize::from(matches!(t.first(), Some(b'+' | b'-')));
    let v: Vec<u8> = match i.banderas & 0x1C0 {
        0x40 => [t, &relleno].concat(),
        0x100 => [&t[..signo], &relleno, &t[signo..]].concat(),
        _ => [&relleno[..], t].concat(),
    };
    i.ancho = 0;
    msvcp_flujos::escribir(os, &v)
}

/// `<< double` como `num_put`: la conversion por `floatfield` (fixed %f,
/// scientific %e, los dos %a, ninguno %g), `showpos` y `showpoint`, la
/// precision del ios y `uppercase`.
fn doble(os: *mut u8, x: f64) -> *mut u8 {
    // SAFETY: el ios de un ostream.
    let i = unsafe { &*msvcp_flujos::ios_de(os) };
    let f = i.banderas;
    let mut fmt: Vec<u8> = alloc::vec![b'%'];
    if f & 0x20 != 0 {
        fmt.push(b'+');
    }
    if f & 0x10 != 0 {
        fmt.push(b'#');
    }
    let (conv, con_precision) = match f & 0x3000 {
        0x2000 => (b'f', true),
        0x1000 => (b'e', true),
        0x3000 => (b'a', false),
        _ => (b'g', true),
    };
    if con_precision {
        fmt.extend_from_slice(alloc::format!(".{}", i.prec.max(0)).as_bytes());
    }
    fmt.push(if f & 0x4 != 0 { conv.to_ascii_uppercase() } else { conv });
    let ranuras = [x.to_bits()];
    let mut l = bmo_proton_x::formato::Lista { ranuras: &ranuras, cadenas: &[], i: 0 };
    let t = bmo_proton_x::formato::formatear(&fmt, &mut l, false);
    con_ancho(os, &t)
}

extern "win64" fn os_float(os: *mut u8, x: f32) -> *mut u8 {
    doble(os, x as f64)
}

/// `<< const void*`: el `%p` de MSVC, 16 cifras hexadecimales en mayusculas.
extern "win64" fn os_puntero(os: *mut u8, p: u64) -> *mut u8 {
    con_ancho(os, alloc::format!("{p:016X}").as_bytes())
}

extern "win64" fn os_i64(os: *mut u8, v: i64) -> *mut u8 {
    msvcp_flujos::con_signo(os, v, 64)
}

// -- istream -----------------------------------------------------------------------------------

fn hueco(sb: *const Sb, h: usize) -> u64 {
    msvcp_flujos::hueco(sb, h)
}

/// El centinela de istream con `noskip`: si el estado no es bueno, failbit
/// y no; si lo es, vacia el tie. (Bloquea el streambuf mientras.)
fn centinela(is: *mut u8) -> bool {
    let ios = msvcp_flujos::ios_de(is);
    // SAFETY: un ios.
    let i = unsafe { &*ios };
    if i.estado != 0 {
        msvcp_flujos::ios_setstate(ios, FAILBIT, false);
        return false;
    }
    if !i.tie.is_null() {
        msvcp_flujos::os_flush(i.tie);
    }
    // SAFETY: un ios.
    unsafe { (*ios).estado == 0 }
}

/// `seekoff(off, way, in)` del streambuf (hueco 10): la posicion, o -1.
fn seekoff(sb: *mut Sb, off: i64, way: i32) -> [i64; 3] {
    let mut r = [0i64; 3];
    // SAFETY: el hueco 10: (this, vuelta, off, way, modo).
    unsafe { core::mem::transmute::<u64, extern "win64" fn(*mut Sb, *mut [i64; 3], i64, i32, i32) -> *mut [i64; 3]>(hueco(sb, 10))(sb, &mut r, off, way, 1) };
    r
}

/// `read(p, n)`: lo que haya, hasta `n` (`gcount`); si no llegan todos,
/// eofbit y failbit.
extern "win64" fn is_read(is: *mut u8, p: *mut u8, n: i64) -> *mut u8 {
    let ios = msvcp_flujos::ios_de(is);
    // SAFETY: `_Chcount`, detras del vbptr.
    let cuenta = unsafe { &mut *(is.add(8) as *mut i64) };
    *cuenta = 0;
    if centinela(is) {
        // SAFETY: su streambuf; el hueco 8 es xsgetn.
        let (sb, leidos) = unsafe {
            let sb = (*ios).sb;
            (sb, core::mem::transmute::<u64, extern "win64" fn(*mut Sb, *mut u8, i64) -> i64>(hueco(sb, 8))(sb, p, n))
        };
        let _ = sb;
        *cuenta = leidos;
        if leidos != n {
            msvcp_flujos::ios_setstate(ios, EOFBIT | FAILBIT, false);
        }
    }
    is
}

/// `seekg(off, way)`: quita eofbit; si no ha fallado, se mueve (o failbit).
extern "win64" fn is_seekg(is: *mut u8, off: i64, way: i32) -> *mut u8 {
    let ios = msvcp_flujos::ios_de(is);
    // SAFETY: un ios.
    unsafe { (*ios).estado &= !EOFBIT };
    // SAFETY: un ios.
    let (estado, sb) = unsafe { ((*ios).estado, (*ios).sb) };
    if estado & (FAILBIT | 4) == 0 && !sb.is_null() {
        let r = seekoff(sb, off, way);
        if r[0] + r[1] == -1 {
            msvcp_flujos::ios_setstate(ios, FAILBIT, false);
        }
    }
    is
}

/// `tellg()`: donde esta (seekoff(0, cur)), o -1 si ya fallo. Vuelve por
/// puntero (un `fpos` de 24 bytes).
extern "win64" fn is_tellg(is: *mut u8, r: *mut [i64; 3]) -> *mut [i64; 3] {
    let ios = msvcp_flujos::ios_de(is);
    // SAFETY: un ios.
    let (estado, sb) = unsafe { ((*ios).estado, (*ios).sb) };
    let v = if estado & (FAILBIT | 4) == 0 && !sb.is_null() { seekoff(sb, 0, 1) } else { [-1, 0, 0] };
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write(v) };
    r
}

// -- Lo demas ----------------------------------------------------------------------------------

extern "win64" fn poner_precision(ios: *mut Ios, n: i64) {
    // SAFETY: un ios.
    unsafe { (*ios).prec = n };
}

/// `setprecision(n)`: un `_Smanip<streamsize>` (vuelve por puntero).
extern "win64" fn setprecision(r: *mut [u64; 2], n: i64) -> *mut [u64; 2] {
    // SAFETY: la vuelta del `.exe`.
    unsafe { r.write([dir!(poner_precision), n as u64]) };
    r
}

/// `_Fiopen` de un nombre ancho: a UTF-8, y el de siempre.
extern "win64" fn fiopen_w(ruta: *const u16, modo: i32, prot: i32) -> u64 {
    if ruta.is_null() {
        return 0;
    }
    // SAFETY: una cadena UTF-16 del `.exe`, acabada en 0.
    let v: Vec<u16> = unsafe { (0..).map(|k| *ruta.add(k)).take_while(|&c| c != 0).collect() };
    let mut b = alloc::string::String::from_utf16_lossy(&v).into_bytes();
    b.push(0);
    msvcp_flujos::fiopen(b.as_ptr(), modo, prot)
}

/// `task_continuation_context()` (privado): `_ContextCallback(false)`, o sea
/// CAPTURA ya (`_Capture`), y en un programa de escritorio no hay contexto
/// que capturar: 0 (con 1, "diferida", el Windows del propietario dijo MAL
/// en tanda20.exe). Y sin correr en linea.
extern "win64" fn task_continuation_context(this: *mut u64) -> *mut u64 {
    // SAFETY: los 16 bytes del `.exe`.
    unsafe {
        this.write(0);
        *(this.add(1) as *mut u8) = 0;
    }
    this
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@M@Z" => dir!(os_float),
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@PEBX@Z" => dir!(os_puntero),
        "??6?$basic_ostream@DU?$char_traits@D@std@@@std@@QEAAAEAV01@_J@Z" => dir!(os_i64),
        "?read@?$basic_istream@DU?$char_traits@D@std@@@std@@QEAAAEAV12@PEAD_J@Z" => dir!(is_read),
        "?seekg@?$basic_istream@DU?$char_traits@D@std@@@std@@QEAAAEAV12@_JH@Z" => dir!(is_seekg),
        "?tellg@?$basic_istream@DU?$char_traits@D@std@@@std@@QEAA?AV?$fpos@U_Mbstatet@@@2@XZ" => dir!(is_tellg),
        "?setprecision@std@@YA?AU?$_Smanip@_J@1@_J@Z" => dir!(setprecision),
        "?_Fiopen@std@@YAPEAU_iobuf@@PEB_WHH@Z" => dir!(fiopen_w),
        "??0task_continuation_context@Concurrency@@AEAA@XZ" => dir!(task_continuation_context),
        _ => return None,
    })
}
