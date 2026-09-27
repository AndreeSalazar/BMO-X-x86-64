//! **`user32.dll` de la casa** (P2, 27-09): las ventanas y su cola de mensajes.
//!
//! Una ventana de Windows es aqui una SUPERFICIE del escritorio de BMO-X: la
//! misma que usan VERRANO y DOOM, con su buzon para las teclas y el raton.
//!
//! ```text
//!    RegisterClassExW        guarda el nombre y la WndProc
//!    CreateWindowExW         pide la superficie (sin mostrar) y manda WM_CREATE
//!    ShowWindow              la ofrece al escritorio, e invalida
//!    UpdateWindow            WM_PAINT en el acto, si hace falta
//!    GetMessageW             el buzon -> mensajes; la cola en el orden de
//!                            Windows (bmo_proton_x::ventanas::Cola); duerme si
//!                            no hay nada
//!    TranslateMessage        nada: el escritorio de BMO-X ya da las letras
//!                            cocinadas (WM_CHAR llega solo)
//!    DispatchMessageW        llama a la WndProc de la ventana
//!    DefWindowProcW          WM_CLOSE -> DestroyWindow; lo demas, 0
//!    DestroyWindow           WM_DESTROY y la ventana muere
//!    PostQuitMessage         WM_QUIT cuando la cola se vacie
//!    InvalidateRect          la ventana tiene que pintarse
//!    BeginPaint / EndPaint   el HDC de la ventana; EndPaint MUESTRA el dibujo
//! ```
//!
//! **Lo que no es Windows, dicho:** el marco y la barra de titulo los pinta el
//! escritorio de BMO-X, asi que `ancho` x `alto` de `CreateWindowExW` son el
//! AREA DE CLIENTE entera (en Windows incluyen el marco). Y cerrar con la X del
//! escritorio mata el proceso: el `.exe` no ve WM_CLOSE por ese camino.

use alloc::boxed::Box;
use alloc::vec::Vec;

use bmo_proton_x::ventanas::{de_evento, Msg, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_PAINT, WM_QUIT};

use crate::{aviso, con, dir, plataforma, Clase, Superficie, Ventana};

/// `WNDPROC`: `LRESULT CALLBACK(HWND, UINT, WPARAM, LPARAM)`.
type WndProc = extern "win64" fn(u64, u32, u64, u64) -> i64;

/// `CW_USEDEFAULT` en ancho o alto: aqui, 640 x 480.
const CW_USEDEFAULT: i32 = 0x8000_0000_u32 as i32;
const ANCHO_POR_DEFECTO: u32 = 640;
const ALTO_POR_DEFECTO: u32 = 480;

/// Los HWND: numeros que no son punteros, uno por ventana. El HDC de una
/// ventana es su HWND con este bit: asi `StretchDIBits` la encuentra.
const HWND_BASE: u64 = 0x0001_0010;
pub(crate) const BIT_HDC: u64 = 0x4000_0000;

fn llamar(wndproc: u64, h: u64, m: u32, w: u64, l: u64) -> i64 {
    // SAFETY: `wndproc` es la que el `.exe` registro con RegisterClassExW:
    // una funcion suya con la convencion de Windows.
    let f: WndProc = unsafe { core::mem::transmute(wndproc as usize) };
    f(h, m, w, l)
}

/// La WndProc de una ventana VIVA, o `None`.
fn proc_de(h: u64) -> Option<u64> {
    con(|e| e.ventanas.iter().find(|v| v.hwnd == h && v.viva).map(|v| v.wndproc))
}

pub(crate) fn superficie_de(h: u64) -> Option<Superficie> {
    con(|e| e.ventanas.iter().find(|v| v.hwnd == h && v.viva).map(|v| v.sup))
}

/// Un nombre de UTF-16 hasta su cero.
///
/// # Safety
/// `p` apunta a una cadena de UTF-16 terminada en cero, como en Windows.
unsafe fn utf16(p: *const u16) -> Vec<u16> {
    let mut v = Vec::new();
    let mut i = 0;
    loop {
        let c = p.add(i).read();
        if c == 0 || i >= 256 {
            return v;
        }
        v.push(c);
        i += 1;
    }
}

/// `RegisterClassExW`: 0 si la estructura no es la de x64 o no trae WndProc.
extern "win64" fn register_class_ex_w(wc: *const u8) -> u16 {
    if wc.is_null() {
        return 0;
    }
    // SAFETY: `WNDCLASSEXW` de x64: cbSize +0, lpfnWndProc +8,
    // lpszClassName +64; 80 bytes que el `.exe` promete legibles.
    let (tam, wndproc, nombre) = unsafe {
        ((wc as *const u32).read(), (wc.add(8) as *const u64).read(), (wc.add(64) as *const u64).read())
    };
    if tam != 80 || wndproc == 0 || nombre == 0 {
        return 0;
    }
    let nombre = if nombre < 0x1_0000 {
        aviso("RegisterClassExW con un atomo por nombre: todavia no");
        return 0;
    } else {
        // SAFETY: una cadena suya, ver `utf16`.
        unsafe { utf16(nombre as *const u16) }
    };
    con(|e| {
        if e.clases.iter().any(|c| c.nombre == nombre) {
            return 0; // ERROR_CLASS_ALREADY_EXISTS en Windows
        }
        let atomo = 0xC000 + e.clases.len() as u16;
        e.clases.push(Clase { nombre, atomo, wndproc });
        atomo
    })
}

/// `CreateWindowExW`: la superficie (sin mostrar), la ventana, y WM_CREATE
/// con su `CREATESTRUCTW`. 0 si la clase no existe, no hay superficie o la
/// WndProc contesta -1 a WM_CREATE.
#[allow(clippy::too_many_arguments)]
extern "win64" fn create_window_ex_w(
    ex: u32,
    clase: *const u16,
    titulo: *const u16,
    estilo: u32,
    x: i32,
    y: i32,
    ancho: i32,
    alto: i32,
    padre: u64,
    menu: u64,
    inst: u64,
    param: u64,
) -> u64 {
    let wndproc = if (clase as u64) < 0x1_0000 {
        let atomo = clase as u64 as u16;
        con(|e| e.clases.iter().find(|c| c.atomo == atomo).map(|c| c.wndproc))
    } else {
        // SAFETY: una cadena suya, ver `utf16`.
        let nombre = unsafe { utf16(clase) };
        con(|e| e.clases.iter().find(|c| c.nombre == nombre).map(|c| c.wndproc))
    };
    let Some(wndproc) = wndproc else { return 0 };
    let medida = |v: i32, por_defecto| if v == CW_USEDEFAULT || v <= 0 { por_defecto } else { v as u32 };
    let (w, h) = (medida(ancho, ANCHO_POR_DEFECTO), medida(alto, ALTO_POR_DEFECTO));
    let Some(sup) = (plataforma().superficie)(w, h) else {
        aviso("CreateWindowExW: el escritorio no dio superficie");
        return 0;
    };
    let hwnd = con(|e| {
        let hwnd = HWND_BASE + 0x10 * e.ventanas.len() as u64;
        e.ventanas.push(Ventana { hwnd, wndproc, sup, viva: true, mostrada: false });
        hwnd
    });
    // CREATESTRUCTW de x64, en el orden de sus campos.
    let cs = Box::new(CreateStruct {
        params: param,
        inst,
        menu,
        padre,
        cy: h as i32,
        cx: w as i32,
        y,
        x,
        estilo: estilo as i32,
        nombre: titulo as u64,
        clase: clase as u64,
        ex,
    });
    if llamar(wndproc, hwnd, WM_CREATE, 0, &*cs as *const CreateStruct as u64) == -1 {
        destroy_window(hwnd);
        return 0;
    }
    hwnd
}

/// `CREATESTRUCTW` de Windows x64 (80 bytes).
#[repr(C)]
struct CreateStruct {
    params: u64,
    inst: u64,
    menu: u64,
    padre: u64,
    cy: i32,
    cx: i32,
    y: i32,
    x: i32,
    estilo: i32,
    nombre: u64,
    clase: u64,
    ex: u32,
}

/// `ShowWindow`: la primera vez, la superficie se ofrece al escritorio y la
/// ventana se invalida. Devuelve si ya se veia (0 la primera vez).
extern "win64" fn show_window(h: u64, _como: i32) -> i32 {
    let Some((sup, ya)) = con(|e| e.ventanas.iter().find(|v| v.hwnd == h && v.viva).map(|v| (v.sup, v.mostrada))) else {
        return 0;
    };
    if ya {
        return 1;
    }
    if !(plataforma().mostrar)(&sup) {
        aviso("ShowWindow: el escritorio no tomo la ventana");
        return 0;
    }
    con(|e| {
        if let Some(v) = e.ventanas.iter_mut().find(|v| v.hwnd == h) {
            v.mostrada = true;
        }
        e.cola.invalidar(h);
    });
    0
}

/// `UpdateWindow`: si hay algo que pintar, WM_PAINT YA, sin pasar por la cola.
extern "win64" fn update_window(h: u64) -> i32 {
    let Some(wp) = proc_de(h) else { return 0 };
    if con(|e| e.cola.por_pintar(h)) {
        llamar(wp, h, WM_PAINT, 0, 0);
    }
    1
}

/// `MSG` de Windows x64: hwnd +0, message +8, wParam +16, lParam +24,
/// time +32, pt +36, lPrivate +44 (48 bytes).
fn escribir_msg(p: *mut u8, m: &Msg) {
    // SAFETY: el `.exe` da un MSG suyo de 48 bytes.
    unsafe {
        (p as *mut u64).write(m.hwnd);
        (p.add(8) as *mut u32).write(m.mensaje);
        (p.add(16) as *mut u64).write(m.wparam);
        (p.add(24) as *mut u64).write(m.lparam);
        (p.add(32) as *mut u32).write(0);
        (p.add(36) as *mut u64).write(0);
        (p.add(44) as *mut u32).write(0);
    }
}

/// `GetMessageW`: el siguiente mensaje; >0 si es uno, 0 si es WM_QUIT. Si no
/// hay nada, se miran los buzones y se duerme: un `.exe` esperando teclas no
/// gasta CPU.
/// Lo que llego a los buzones de las ventanas, a la cola.
fn bombear() {
    let p = plataforma();
    let vivas: Vec<(u64, Superficie)> = con(|e| e.ventanas.iter().filter(|v| v.viva && v.mostrada).map(|v| (v.hwnd, v.sup)).collect());
    for (hwnd, sup) in vivas {
        loop {
            let ev = (p.evento)(&sup);
            if ev == 0 {
                break;
            }
            if let Some(m) = de_evento(hwnd, ev) {
                con(|e| e.cola.publicar(m));
            }
        }
    }
}

extern "win64" fn get_message_w(msg: *mut u8, h: u64, min: u32, max: u32) -> i32 {
    if msg.is_null() {
        return -1;
    }
    if h != 0 || min != 0 || max != 0 {
        aviso("GetMessageW con filtro: todavia no filtra, da el siguiente de todos");
    }
    let p = plataforma();
    loop {
        bombear();
        if let Some(m) = con(|e| e.cola.sacar()) {
            escribir_msg(msg, &m);
            return if m.mensaje == WM_QUIT { 0 } else { 1 };
        }
        // Sin mensajes: primero otros hilos del `.exe` (P4); si ninguno puede
        // seguir, dormir.
        if !crate::hilos::ceder() {
            (p.dormir)();
        }
    }
}

const PM_REMOVE: u32 = 1;

/// `PeekMessageW` (P3c1): como GetMessageW pero SIN esperar: 1 si habia
/// uno (y con PM_REMOVE se saca), 0 si no. Un bucle de juego vive de esto;
/// cuando no hay nada se cede el turno a otro hilo del `.exe` (si lo hay).
extern "win64" fn peek_message_w(msg: *mut u8, h: u64, min: u32, max: u32, quitar: u32) -> i32 {
    if msg.is_null() {
        return 0;
    }
    if h != 0 || min != 0 || max != 0 {
        aviso("PeekMessageW con filtro: todavia no filtra, da el siguiente de todos");
    }
    bombear();
    let m = if quitar & PM_REMOVE != 0 { con(|e| e.cola.sacar()) } else { con(|e| e.cola.mirar()) };
    match m {
        Some(m) => {
            escribir_msg(msg, &m);
            1
        }
        None => {
            crate::hilos::ceder();
            0
        }
    }
}

/// `AdjustWindowRect(Ex)`: el marco lo pinta el escritorio de BMO-X y la
/// ventana ES su area de cliente (P2): el rectangulo no cambia.
extern "win64" fn adjust_window_rect(r: *mut i32, _estilo: u32, _menu: i32) -> i32 {
    (!r.is_null()) as i32
}

extern "win64" fn adjust_window_rect_ex(r: *mut i32, _estilo: u32, _menu: i32, _ex: u32) -> i32 {
    (!r.is_null()) as i32
}

/// `LoadCursorW`: un handle; el cursor lo pinta el escritorio.
extern "win64" fn load_cursor_w(_inst: u64, id: u64) -> u64 {
    0x5A1D_C000_0000 | (id & 0xFFFF)
}

/// `SetWindowTextW`: si la ventana existe, si. El titulo lo pone el
/// escritorio de BMO-X; el de Windows no se muestra todavia.
extern "win64" fn set_window_text_w(h: u64, _t: *const u16) -> i32 {
    con(|e| e.ventanas.iter().any(|v| v.hwnd == h && v.viva)) as i32
}

/// `TranslateMessage`: aqui no traduce -- las letras ya llegan cocinadas del
/// escritorio como WM_CHAR. 0, que en Windows es "no genero nada".
extern "win64" fn translate_message(_msg: *const u8) -> i32 {
    0
}

/// `DispatchMessageW`: a la WndProc de su ventana.
extern "win64" fn dispatch_message_w(msg: *const u8) -> i64 {
    if msg.is_null() {
        return 0;
    }
    // SAFETY: un MSG suyo de 48 bytes (ver `escribir_msg`).
    let (h, m, w, l) = unsafe {
        ((msg as *const u64).read(), (msg.add(8) as *const u32).read(), (msg.add(16) as *const u64).read(), (msg.add(24) as *const u64).read())
    };
    match proc_de(h) {
        Some(wp) => llamar(wp, h, m, w, l),
        None => 0,
    }
}

/// `DefWindowProcW`: WM_CLOSE cierra la ventana (DestroyWindow); WM_PAINT la
/// VALIDA (lo que hace el BeginPaint/EndPaint de Windows por dentro: sin
/// esto, un `.exe` que pinta con D3D12 y deja WM_PAINT al sistema recibiria
/// WM_PAINT sin fin); lo demas, 0.
extern "win64" fn def_window_proc_w(h: u64, m: u32, _w: u64, _l: u64) -> i64 {
    match m {
        WM_CLOSE => {
            destroy_window(h);
        }
        WM_PAINT => con(|e| e.cola.validar(h)),
        _ => {}
    }
    0
}

/// `DestroyWindow`: WM_DESTROY a su WndProc y la ventana muere, con lo que le
/// quedara en la cola. La superficie se queda hasta que el proceso se vaya.
extern "win64" fn destroy_window(h: u64) -> i32 {
    let Some(wp) = proc_de(h) else { return 0 };
    llamar(wp, h, WM_DESTROY, 0, 0);
    con(|e| {
        if let Some(v) = e.ventanas.iter_mut().find(|v| v.hwnd == h) {
            v.viva = false;
        }
        e.cola.olvidar(h);
    });
    1
}

extern "win64" fn post_quit_message(codigo: i32) {
    con(|e| e.cola.salir(codigo));
}

/// `InvalidateRect`: entera (el rectangulo no se mira: se repinta todo). Con
/// `h = 0`, todas las del hilo, como en Windows.
extern "win64" fn invalidate_rect(h: u64, _rect: *const u8, _borrar: i32) -> i32 {
    con(|e| {
        let hs: Vec<u64> = e.ventanas.iter().filter(|v| v.viva && (h == 0 || v.hwnd == h)).map(|v| v.hwnd).collect();
        for x in &hs {
            e.cola.invalidar(*x);
        }
        !hs.is_empty() as i32
    })
}

/// `BeginPaint`: valida la ventana y rellena el `PAINTSTRUCT` (hdc +0,
/// fErase +8, rcPaint +12, el resto a cero; 72 bytes). Devuelve el HDC.
extern "win64" fn begin_paint(h: u64, ps: *mut u8) -> u64 {
    let Some(sup) = superficie_de(h) else { return 0 };
    con(|e| e.cola.validar(h));
    let hdc = h | BIT_HDC;
    if !ps.is_null() {
        // SAFETY: un PAINTSTRUCT suyo de 72 bytes.
        unsafe {
            core::ptr::write_bytes(ps, 0, 72);
            (ps as *mut u64).write(hdc);
            (ps.add(20) as *mut i32).write(sup.ancho as i32);
            (ps.add(24) as *mut i32).write(sup.alto as i32);
        }
    }
    hdc
}

/// `EndPaint`: el dibujo esta entero, y ahora se MUESTRA.
extern "win64" fn end_paint(h: u64, _ps: *const u8) -> i32 {
    let Some(sup) = superficie_de(h) else { return 0 };
    (plataforma().presentar)(&sup);
    1
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "RegisterClassExW" => dir!(register_class_ex_w),
        "CreateWindowExW" => dir!(create_window_ex_w),
        "ShowWindow" => dir!(show_window),
        "UpdateWindow" => dir!(update_window),
        "GetMessageW" => dir!(get_message_w),
        "PeekMessageW" => dir!(peek_message_w),
        "AdjustWindowRect" => dir!(adjust_window_rect),
        "AdjustWindowRectEx" => dir!(adjust_window_rect_ex),
        "LoadCursorW" => dir!(load_cursor_w),
        "SetWindowTextW" => dir!(set_window_text_w),
        "TranslateMessage" => dir!(translate_message),
        "DispatchMessageW" => dir!(dispatch_message_w),
        "DefWindowProcW" => dir!(def_window_proc_w),
        "DestroyWindow" => dir!(destroy_window),
        "PostQuitMessage" => dir!(post_quit_message),
        "InvalidateRect" => dir!(invalidate_rect),
        "BeginPaint" => dir!(begin_paint),
        "EndPaint" => dir!(end_paint),
        _ => return None,
    })
}
