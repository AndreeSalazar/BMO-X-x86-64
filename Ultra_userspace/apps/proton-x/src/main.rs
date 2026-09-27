//! **PROTON-X en el Ryzen** (P1c, 27-09): un `.exe` de Windows en Ring 3.
//!
//! `run sys/proton-x.bex apps/hola.exe` (sin argumento, `apps/hola.exe`):
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
//! superficie de la ventana y el codigo de los sombreadores (P3b3b). Que esten seguidos
//! se COMPRUEBA, no se supone: si un dia el kernel dejara un hueco, esto lo
//! dice y no salta.
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

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut arg = [0u8; 96];
    let n = bmo::argumentos(&mut arg);
    let ruta: &[u8] = if n == 0 { b"apps/hola.exe" } else { &arg[..n] };
    let nombre = core::str::from_utf8(ruta).unwrap_or("?");

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
