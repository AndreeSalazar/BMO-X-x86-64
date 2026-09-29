//! **PROTON-X en el Ryzen** (P1c, 27-09): un `.exe` de Windows en Ring 3.
//!
//! `run sys/proton-x.bex window/hola.exe` (sin argumento, `window/hola.exe`;
//! los `.exe` de Windows viven en `window/` desde el 27-09):
//!
//! ```text
//!    1  leer el .exe del volumen, entero          un bloque, que se suelta
//!    2  el veredicto y la forma (bmo-proton-x)    solo PE32+ x86-64
//!    3  partir: codigo delante, datos detras      dos bloques SEGUIDOS
//!    4  colocar en SU direccion, relocalizar      la base es la del bloque
//!    5  resolver contra la tabla de la casa       bmo-proton-x-casa, o no arranca
//!    6  SELLAR el codigo (MEM_OP_SELLAR)          R+X sin W; los datos, sin X
//!    7  saltar a su entrada, como `extern "win64"`
//! ```
//!
//! **Lo que el kernel pone y este fichero cuenta con ello** (ring0/obj/
//! memory.rs): cada bloque que se pide cae JUSTO DETRAS del anterior en las
//! direcciones del proceso (el cursor solo avanza), y un proceso tiene como
//! mucho OCHO bloques vivos (`MAX_PETICIONES`; eran cuatro hasta el 20-09,
//! cuando llego `MEM_OP_SOLTAR`). Por eso el orden: fichero (1) y monton (2);
//! se suelta el fichero (1); codigo (2) y datos (3), seguidos. Despues, la
//! superficie de la ventana, el codigo de los sombreadores (P3b3b) y la arena del
//! monton de Windows (P4e, 64 MiB, al primer HeapAlloc). Que esten seguidos
//! se COMPRUEBA, no se supone: si un dia el kernel dejara un hueco, esto lo
//! dice y no salta.
//!
//! **N2 y el CENSO** (29-09): una ruta de D: va entre comillas (lleva
//! espacios): `run sys/proton-x.bex "d:Cyberpunk 2077/bin/x64/x.exe"`. Y
//! `--censo <ruta>` NO ejecuta: dice que DLL y funciones de Windows pide el
//! `.exe` y cuantas tiene ya la casa, leyendo solo sus cabeceras y la seccion
//! de sus importaciones (un `.exe` de 60 MB no se trae entero). En el
//! escritorio, `personal censo <ruta>`.
//!
//! **El GS de Windows** (P1d, 27-09): antes de saltar, un TEB y un PEB en el
//! monton y el GS del hilo apuntando al TEB (`TASK_OP_PON_GS`). Un `.exe`
//! encuentra ahi su pila, su base, su LastError y sus ids, como en Windows
//! (`run sys/proton-x.bex apps/teb.exe` lo comprueba).

#![no_std]
#![no_main]

extern crate alloc;

mod monton;
mod plataforma;
mod la3060;

use alloc::format;
use alloc::vec::Vec;
use bmo_proton_x::{colocar, importaciones, leer, partir, resolver, teb, tls, Permiso};
use bmo_userland as bmo;

#[global_allocator]
static MONTON: monton::Monton = monton::Monton::vacio();

/// Lo mas grande que se lee hoy: un `.exe` de P1 son KiB.
const TOPE_EXE: u64 = 16 << 20;

fn di(s: &str) {
    bmo::consola(s);
}

fn fin(motivo: &str) -> ! {
    di(&format!("PROTON-X: NO -- {motivo}\n"));
    bmo::salir();
}

/// El final del `.exe`, por `ExitProcess` o porque su entrada volvio.
pub(crate) fn fin_del_exe(codigo: u32) -> ! {
    // Sin `format!`: el `.exe` pudo gastar lo que quisiera del monton.
    let mut b = [0u8; 48];
    let pre = b"PROTON-X: el .exe salio con ";
    b[..pre.len()].copy_from_slice(pre);
    let mut n = pre.len();
    let mut d = [0u8; 10];
    let mut k = 0;
    let mut v = codigo;
    loop {
        d[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    while k > 0 {
        k -= 1;
        b[n] = d[k];
        n += 1;
    }
    b[n] = b'\n';
    di(core::str::from_utf8(&b[..n + 1]).unwrap_or("PROTON-X: el .exe salio\n"));
    bmo::salir();
}

/// **La ruta y lo de detras**: hasta el primer espacio, o entre comillas.
fn partir_ruta(todo: &[u8]) -> (&[u8], &[u8]) {
    if let Some(r) = todo.strip_prefix(b"\"") {
        let k = r.iter().position(|&c| c == b'"').unwrap_or(r.len());
        return (&r[..k], r.get(k + 1..).unwrap_or(&[]));
    }
    let corte = todo.iter().position(|&c| c == b' ').unwrap_or(todo.len());
    todo.split_at(corte)
}

/// Lo mas grande que el censo lee de golpe: la seccion de las importaciones
/// (el tope de un bloque del kernel, `obj/memory.rs`).
const TOPE_SECCION: u64 = 64 << 20;
/// Donde queda la lista entera.
const RUTA_CENSO: &[u8] = b"informe/censo.txt";

/// **EL CENSO de un `.exe`** (29-09, *"lo del Cyberpunk que faltan MAS para
/// completar?"*): sin ejecutarlo y sin traerlo entero -- las cabeceras y la
/// seccion de sus importaciones --, cada funcion de Windows que pide contra
/// la tabla de la casa. Las DLL que viven junto al `.exe` son DEL JUEGO: esas
/// no las pone la casa, se cargan (P5a) y piden lo suyo. Resumen por la
/// consola; la lista entera, en `informe/censo.txt`.
fn censo(ruta: &[u8]) -> ! {
    use alloc::string::String;
    let nombre = core::str::from_utf8(ruta).unwrap_or("?");
    let Some(bloque) = bmo::Memoria::request(16 << 20) else { fin("sin memoria para el censo") };
    // SAFETY: el bloque es de este proceso y no se suelta nunca (forget).
    unsafe { MONTON.poner(bloque.base() as usize, 16 << 20) };
    core::mem::forget(bloque);
    let Ok(a) = bmo::Archivo::reflejar(ruta) else { fin(&format!("censo: no encuentro {nombre}")) };
    let mide = a.size();
    // Las cabeceras: los primeros 64 KiB bastan (un PE las pide en 4). De una
    // llamada a un bloque (`read` va de siete en siete bytes).
    let n = (64u64 << 10).min(mide);
    let Some(hb) = bmo::Memoria::request(n.max(1)) else { fin("sin memoria para las cabeceras") };
    let k = a.leer_en(&hb, 0, n);
    // SAFETY: `k` bytes que el kernel acaba de escribir en un bloque nuestro.
    let cab: Vec<u8> = unsafe { core::slice::from_raw_parts(hb.base() as *const u8, k as usize) }.to_vec();
    hb.soltar();
    let pe = bmo_proton_x::leer_cabeceras(&cab, mide).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    let rva = pe.importaciones.rva;
    let Some(sec) = pe.secciones.iter().find(|s| rva != 0 && (s.rva..s.rva + s.tam_en_fichero).contains(&rva)) else {
        fin(&format!("{nombre}: no pide nada, o sus importaciones no caen en una seccion"));
    };
    let tam = sec.tam_en_fichero as u64;
    if tam > TOPE_SECCION {
        fin(&format!("{nombre}: la seccion {} mide {} MiB, pasa del tope de un bloque", sec.nombre, tam >> 20));
    }
    let Some(b) = bmo::Memoria::request(tam) else { fin("sin memoria para la seccion de importaciones") };
    if a.saltar(sec.desde as u64) != sec.desde as u64 || a.leer_en(&b, 0, tam) != tam {
        fin(&format!("{nombre}: la seccion {} no se leyo entera", sec.nombre));
    }
    // SAFETY: `tam` bytes que el kernel acaba de escribir en un bloque nuestro.
    let trozo = unsafe { core::slice::from_raw_parts(b.base() as *const u8, tam as usize) };
    let imps = bmo_proton_x::importaciones_de_seccion(&pe, trozo, sec.rva).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    drop(a);
    // La carpeta del `.exe`: donde viven las DLL del juego.
    let dir = match ruta.iter().rposition(|&c| c == b'/') {
        Some(k) => &ruta[..k + 1],
        None => &ruta[..0],
    };
    // Por DLL, en el orden en que las pide: (nombre, cuantas, las tiene la casa).
    let mut dlls: Vec<(String, u32, u32)> = Vec::new();
    let mut lista = String::new();
    for i in &imps {
        let hay = bmo_proton_x_casa::tabla(&i.dll, &i.funcion).is_some();
        match dlls.iter_mut().find(|d| d.0.eq_ignore_ascii_case(&i.dll)) {
            Some(d) => {
                d.1 += 1;
                d.2 += hay as u32;
            }
            None => dlls.push((i.dll.clone(), 1, hay as u32)),
        }
        if !hay {
            lista.push_str(&format!("{} {}\n", i.dll, i.funcion));
        }
    }
    let total = imps.len() as u32;
    let tiene: u32 = dlls.iter().map(|d| d.2).sum();
    di(&format!("CENSO de {nombre}: {} MiB, {} DLL, {total} funciones importadas; la casa ya tiene {tiene} ({}%)\n", mide >> 20, dlls.len(), tiene * 100 / total.max(1)));
    let mut del_juego = 0u32;
    for (dll, n, si) in &dlls {
        let mut junto: Vec<u8> = dir.to_vec();
        junto.extend_from_slice(dll.as_bytes());
        let es_del_juego = bmo::Archivo::reflejar(&junto).is_ok();
        del_juego += es_del_juego as u32;
        let que = if es_del_juego {
            "DEL JUEGO: vive junto al .exe, se carga como DLL (P5a)"
        } else if si == n {
            "la casa la tiene ENTERA"
        } else if *si == 0 {
            "la casa NO la tiene"
        } else {
            "a medias"
        };
        di(&format!("  {dll:<28} {si:>4} de {n:<4} {que}\n"));
        lista.push_str(&format!("# {dll}: {si} de {n}; {que}\n"));
    }
    di(&format!("  {} DLL del juego; lo que falta, funcion a funcion: {}\n", del_juego, core::str::from_utf8(RUTA_CENSO).unwrap_or("")));
    guardar_censo(lista.as_bytes());
    bmo::salir();
}

/// La lista entera a `informe/censo.txt`, de una llamada.
fn guardar_censo(bytes: &[u8]) {
    let Ok(f) = bmo::Archivo::create(RUTA_CENSO) else {
        di("  (no se pudo crear informe/censo.txt)\n");
        return;
    };
    let escritos = match bmo::Memoria::request(bytes.len().max(1) as u64) {
        Some(b) => {
            // SAFETY: un bloque nuestro de al menos `bytes.len()` bytes.
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), b.base(), bytes.len()) };
            f.escribir_de(&b, 0, bytes.len() as u64) as usize
        }
        None => f.write(bytes),
    };
    if escritos != bytes.len() || !f.close() {
        di("  (informe/censo.txt no salio entero)\n");
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut arg = [0u8; 96];
    let n = bmo::argumentos(&mut arg);
    let todo: &[u8] = if n == 0 { b"window/hola.exe" } else { &arg[..n] };
    // ** EL CENSO (29-09): `--censo <ruta>` no ejecuta nada: dice que DLL y que
    // funciones de Windows pide el `.exe` y cuantas tiene ya la casa.
    if let Some(r) = todo.strip_prefix(b"--censo ") {
        censo(partir_ruta(r).0);
    }
    // P4e: `window/x.exe lo de detras` -- la ruta hasta el primer espacio; lo
    // demas es la linea de ordenes del `.exe` (GetCommandLineW). N2: o entre
    // comillas, que las rutas de D: llevan espacios (`"d:Cyberpunk 2077/..."`).
    let (ruta, resto) = partir_ruta(todo);
    let nombre = core::str::from_utf8(ruta).unwrap_or("?");
    let linea = core::str::from_utf8(resto).unwrap_or("");

    // -- 1. El fichero, entero.
    let Ok(a) = bmo::Archivo::leer_de(ruta) else {
        di("PROTON-X: NO -- no encuentro ");
        di(nombre);
        di(" en el volumen\n");
        bmo::salir();
    };
    let mide = a.size();
    if mide == 0 || mide > TOPE_EXE {
        di("PROTON-X: NO -- el .exe esta vacio o pasa de 16 MiB\n");
        bmo::salir();
    }
    let Some(fichero) = bmo::Memoria::request(mide) else {
        di("PROTON-X: NO -- sin memoria para leer el .exe\n");
        bmo::salir();
    };
    if a.leer_en(&fichero, 0, mide) != mide {
        di("PROTON-X: NO -- el .exe no se leyo entero\n");
        bmo::salir();
    }
    drop(a);

    // El monton: el .exe copiado, la imagen y lo que el cargador anote; y lo
    // que piden las DLL de la casa, que desde P3b2 son back buffers de
    // 1280x720 (3.5 MiB cada uno) y los buferes del `.exe`. 32 MiB de mas.
    let para_monton = (3 * mide + (32 << 20)).min(64 << 20);
    let Some(bloque) = bmo::Memoria::request(para_monton) else {
        di("PROTON-X: NO -- sin memoria para el monton del cargador\n");
        bmo::salir();
    };
    // SAFETY: el bloque es de este proceso y no se suelta nunca (forget).
    unsafe { MONTON.poner(bloque.base() as usize, para_monton as usize) };
    core::mem::forget(bloque);
    // SAFETY: `mide` bytes que el kernel acaba de escribir en un bloque nuestro.
    let exe: Vec<u8> = unsafe { core::slice::from_raw_parts(fichero.base() as *const u8, mide as usize) }.to_vec();
    fichero.soltar();

    // -- 2 y 3. El veredicto, la forma, y como se parte.
    let pe = leer(&exe).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    let partes = partir(&pe).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));

    // La entrada tiene que caer en lo que se va a sellar: saltar a los datos
    // seria un #PF por NX, y saltar fuera, peor.
    if pe.entrada == 0 || pe.entrada >= partes.codigo || !pe.secciones.iter().any(|s| s.permiso() == Permiso::Codigo && (s.rva..s.rva + s.tam_en_imagen()).contains(&pe.entrada)) {
        fin(&format!("{nombre}: la entrada ({:#x}) no cae en una seccion de codigo", pe.entrada));
    }

    // -- Los dos bloques, SEGUIDOS (se comprueba).
    let Some(codigo) = bmo::Memoria::request(partes.codigo as u64) else { fin("sin memoria para el codigo") };
    let datos = if partes.datos > 0 {
        let Some(d) = bmo::Memoria::request(partes.datos as u64) else { fin("sin memoria para los datos") };
        Some(d)
    } else {
        None
    };
    let base = codigo.base() as u64;
    if let Some(d) = datos.as_ref() {
        if d.base() as u64 != base + partes.codigo as u64 {
            fin(&format!("los bloques no quedaron seguidos ({:#x} y {:#x}): la imagen no se puede partir", base, d.base() as u64));
        }
    }

    // -- 4 y 5. Colocar en SU direccion y resolver contra la casa.
    let mut img = colocar(&pe, &exe, base).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    let imps = importaciones(&pe, &img).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    // P4: su TLS (`__declspec(thread)` y los callbacks), leido de la imagen
    // YA relocalizada: sus direcciones son las de aqui.
    let tls_del_exe = tls::leer(&pe, &img, base).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    resolver(&mut img, &imps, bmo_proton_x_casa::tabla).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    let (delante, detras) = img.split_at(partes.codigo as usize);
    // SAFETY: cada bloque mide lo que `partir` dijo, y es nuestro.
    unsafe {
        core::ptr::copy_nonoverlapping(delante.as_ptr(), codigo.base(), delante.len());
        if let Some(d) = datos.as_ref() {
            core::ptr::copy_nonoverlapping(detras.as_ptr(), d.base(), detras.len().min(partes.datos as usize));
        }
    }

    // -- 6. SELLAR: de datos a codigo. Sin esto, saltar seria un #PF por NX.
    if let Err(m) = codigo.sellar() {
        fin(&format!("SELLAR dice NO (motivo {m}): el codigo del .exe no se ejecuta sin sellar"));
    }
    di(&format!(
        "PROTON-X: {nombre}: {} B, PE32+ x86-64; en {:#x} (el enlazador queria {:#x}); {} funcion(es) de la casa; codigo {} KiB SELLADO, datos {} KiB sin X; monton {} B\n",
        exe.len(),
        base,
        pe.base,
        imps.len(),
        partes.codigo / 1024,
        partes.datos / 1024,
        MONTON.gastado()
    ));
    // -- 6b. P1d: el TEB y el PEB, y el GS del hilo apuntando al TEB.
    poner_teb(base);
    // -- 6c. P2: las DLL de la casa, con BMO-X debajo (la consola, las
    // superficies del escritorio y su buzon).
    // SAFETY: un solo `.exe` por proceso, y todavia no se ha saltado.
    unsafe { bmo_proton_x_casa::empezar(plataforma::de_bmo()) };
    // P4d: su directorio actual es el suyo (`window` para `window/x.exe`).
    bmo_proton_x_casa::ficheros::poner_directorio(nombre.rsplit_once('/').map(|(d, _)| d).unwrap_or(""));
    // P4e: su nombre (GetModuleFileNameW) y su linea de ordenes.
    bmo_proton_x_casa::proceso::poner_exe(nombre, linea);
    // -- 6d. P4: el TLS del hilo principal y los callbacks con PROCESS_ATTACH,
    // antes de la entrada, como el cargador de Windows. Corren YA en el
    // codigo sellado; la casa los llama con la pila alineada.
    if let Some(t) = &tls_del_exe {
        di(&format!("PROTON-X: TLS: {} B por hilo, {} callback(s)\n", t.bytes(), t.callbacks.len()));
    }
    // SAFETY: el GS ya esta en el TEB, `empezar` hecho, la imagen sellada.
    unsafe { bmo_proton_x_casa::hilos::preparar_tls(tls_del_exe, base) };
    di("PROTON-X: salto a su entrada ----------------------------------\n");
    // La imagen vive hasta que el proceso muera: sin `Drop`, que la soltaria.
    core::mem::forget(codigo);
    core::mem::forget(datos);

    // -- 7. Saltar, con la pila COMO LA DEJA WINDOWS: alineada a 16 antes del
    // `call` y con la sombra de 32 bytes.
    //
    // *** NO SE LE PUEDE DEJAR AL COMPILADOR (P2 en el metal, 27-09). El kernel
    // arranca Ring 3 con `rsp = USER_STACK_TOP`, alineado a 16 JUSTO al entrar
    // en `_start`, y Rust da por hecho lo que deja un `call` (16 + 8): toda esta
    // app corre desalineada 8 bytes y a ella le da igual (sin SSE en Ring 3).
    // Pero un `extern "win64"` llamado desde aqui le pasaba al `.exe` la pila
    // torcida, y el compilador de Microsoft guarda xmm6..xmm15 con `movaps`:
    // `ventana.exe` murio con un #GP en su primera instruccion, y `hola` y
    // `teb`, sin `movaps`, no se enteraron. El banco del anfitrion tampoco: su
    // trampolin ya alineaba. Una vez alineada la entrada, todo lo que el `.exe`
    // llama (la casa) y lo que la casa le devuelve (su WndProc) sale derecho.
    let r: u64;
    // SAFETY: la entrada cae en una seccion de codigo del bloque sellado
    // (comprobado arriba) y la imagen esta colocada y resuelta. `r12` es
    // no volatil en Windows x64: el `.exe` lo devuelve como estaba.
    unsafe {
        core::arch::asm!(
            "mov r12, rsp",
            "and rsp, -16",
            "sub rsp, 32",
            "call {entrada}",
            "mov rsp, r12",
            entrada = in(reg) base + pe.entrada as u64,
            out("r12") _,
            lateout("rax") r,
            clobber_abi("win64"),
        );
    }
    let r = r as u32;
    fin_del_exe(r)
}

/// La pila de Ring 3 de BMO-X. Espejo de `USER_STACK_TOP` y `USER_STACK_SIZE`
/// (`Ultra_kernel_x86-64/kernel/src/ring0/mm/vmm/verde.rs`): no estan en el
/// ABI, asi que [`poner_teb`] COMPRUEBA con su propio `rsp` que caen donde
/// dicen, y si no, no salta.
const PILA_TOPE: u64 = 0x8000_0000;
const PILA_BYTES: u64 = 0x1_0000;

/// **P1d: el TEB y el PEB, y el GS** (PLAN_PROTON_X, 3). Un `.exe` de Windows
/// lee `gs:[0x30]` sin avisar --lo pone el compilador de Microsoft, y el CRT
/// lo hace al arrancar--, asi que el GS se pone SIEMPRE, lo lea o no.
///
/// Van en el monton (R+W, sin X). Dice donde, y lo que le costo al kernel
/// poner el GS: ese `wrmsr` es lo que paga un relevo entre este hilo y uno
/// con otro GS. Y lo pide otra vez con el mismo valor: tiene que ser 0,
/// porque el kernel solo escribe el MSR si CAMBIA.
fn poner_teb(base: u64) {
    let rsp: u64;
    // SAFETY: leer un registro.
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags)) };
    let fondo = PILA_TOPE - PILA_BYTES;
    if !(fondo < rsp && rsp <= PILA_TOPE) {
        fin(&format!("la pila no esta donde el kernel la pone ({rsp:#x} fuera de {fondo:#x}..{PILA_TOPE:#x}): el TEB mentiria"));
    }
    let bytes = teb::TEB_BYTES + teb::PEB_BYTES;
    let Ok(forma) = core::alloc::Layout::from_size_align(bytes, 4096) else { fin("la forma del TEB") };
    // SAFETY: `forma` no mide cero.
    let mem = unsafe { alloc::alloc::alloc_zeroed(forma) };
    if mem.is_null() {
        fin("sin monton para el TEB y el PEB");
    }
    let h = teb::Hilo {
        teb: mem as u64,
        peb: mem as u64 + teb::TEB_BYTES as u64,
        pila_tope: PILA_TOPE,
        pila_fondo: fondo,
        proceso: bmo::pid(),
        hilo: bmo::tid(),
        base_imagen: base,
    };
    // SAFETY: `bytes` recien pedidos al monton, nuestros y vivos para siempre.
    let t = unsafe { core::slice::from_raw_parts_mut(mem, bytes) };
    let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
    teb::escribir_teb(tb, &h);
    teb::escribir_peb(pb, &h);
    let ciclos = bmo::poner_gs(h.teb).unwrap_or_else(|c| fin(&format!("el kernel no pone el GS (codigo {c}): un .exe de Windows no encontraria su TEB")));
    let otra = bmo::poner_gs(h.teb).unwrap_or(u64::MAX);
    di(&format!(
        "PROTON-X: TEB en {:#x}, PEB en {:#x}; GS -> TEB: el wrmsr costo {} ciclos (el mismo otra vez: {}, no se toca)\n",
        h.teb, h.peb, ciclos, otra
    ));
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    di("PROTON-X: panico en el cargador\n");
    if let Some(s) = info.message().as_str() {
        di(s);
        di("\n");
    }
    bmo::salir();
}
