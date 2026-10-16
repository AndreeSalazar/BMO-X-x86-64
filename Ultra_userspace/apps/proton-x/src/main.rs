//! **PROTON-X en el Ryzen** (P1c, 27-09): un `.exe` de Windows en Ring 3.
//!
//! `run sys/proton-x.bex window/hola.exe` (sin argumento, `window/hola.exe`;
//! los `.exe` de Windows viven en `window/` desde el 27-09):
//!
//! ```text
//!    1  las cabeceras del .exe, sin traerlo       un bloque, que se suelta
//!    2  el veredicto y la forma (bmo-proton-x)    solo PE32+ x86-64
//!    3  partir, y las DLL del juego junto a el    una IMAGEN DECLARADA (P0.4b)
//!    4  colocar, del disco a su RVA; relocalizar la base es la de su parte
//!    5  registrar las DLL y resolver              la casa y las DLL del juego
//!    6  SELLAR el codigo de cada uno              R+X sin W; los datos, sin X
//!    7  los DllMain, y saltar a su entrada, como `extern "win64"`
//! ```
//!
//! **La imagen declarada** (P0.4b, 30-09; ver `cargador.rs`): el `.exe` y sus
//! DLL van en partes que el kernel juzga UNA vez contra la RAM libre de ese
//! momento (`bmo::Imagen`), no en bloques de 64 MiB. Lo de abajo sobre los
//! bloques sigue valiendo para el monton, las cabeceras y el censo.
//!
//! **Lo que el kernel pone y este fichero cuenta con ello** (ring0/obj/
//! memory.rs): cada bloque que se pide cae JUSTO DETRAS del anterior en las
//! direcciones del proceso (el cursor solo avanza), y un proceso tiene como
//! mucho OCHO bloques vivos (`MAX_PETICIONES`; eran cuatro hasta el 20-09,
//! cuando llego `MEM_OP_SOLTAR`). Por eso el orden: monton (1); cabeceras
//! (2), que se sueltan; codigo (3) y datos (4), seguidos. Despues, la
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
//! escritorio, `personal censo <ruta>`. El censo es COMPLETO: sigue tambien
//! las DLL del juego junto al `.exe` y lista las que se cargan en vivo.
//!
//! **El DIARIO** (30-09): `--diario <ruta>` ejecuta como siempre y deja en
//! `informe/diario.txt` cada funcion de Windows que el `.exe` llama por
//! primera vez, en orden: `run sys/proton-x.bex --diario window/hola.exe`.
//!
//! **El modo 3060** (12-10): `--3060 <ruta>` es `--diario` y ademas dice al
//! arrancar si la 3060 esta lista para los lotes (el escritorio la prepara
//! con `gpu verrano`), el censo de la 3060 (LI0) cada 10 s y cuantos lotes
//! dibujo ella. En el escritorio: `personal 3060 <ruta>`.
//!
//! **Sin obreros** (07-10): `--sin-obreros` apaga los sub-directores (el
//! dibujo de la CPU en franjas por los otros nucleos, `obreros.rs`) aunque
//! haya `smp all`. Va con `--diario` en cualquier orden.
//!
//! **EXPROPIAR** (07-10): antes de saltar, la ALARMA del kernel
//! (`bmo::alarma`): cada 4 ms visita la puerta de la casa, que le quita el
//! turno al hilo del juego que no lo suelta (`bmo_proton_x_casa::expropiar`).
//! SIEMPRE, sin orden para apagarla: el kernel es el orquestador, y un hilo
//! que no suelta el nucleo nunca tiene razon (asi Windows y Linux).
//!
//! **El GS de Windows** (P1d, 27-09): antes de saltar, un TEB y un PEB en el
//! monton y el GS del hilo apuntando al TEB (`TASK_OP_PON_GS`). Un `.exe`
//! encuentra ahi su pila, su base, su LastError y sus ids, como en Windows
//! (`run sys/proton-x.bex apps/teb.exe` lo comprueba).

#![no_std]
#![no_main]

extern crate alloc;

mod cargador;
mod censo;
/// 05-10: lo que la casa no supo hacer, en un cartel rojo en la ventana.
mod cartel;
mod el_nulo;
mod en_vivo;
mod entorno_windows;
mod la3060;
/// LX2 (15-10): lo que falta, con el binario del juego al lado.
mod lo_que_falta;
/// LI2f (13-10): el computo del juego por la 3060.
mod computo3060;

/// **El monton**: lo que el cargador anote, y lo que piden las DLL de la
/// casa, que desde P3b2 son back buffers de 1280x720 (3.5 MiB cada uno) y
/// los buferes del `.exe`.
fn poner_monton() {
    let Some(bloque) = bmo::Memoria::request(PARA_MONTON) else {
        di("PROTON-X: NO -- sin memoria para el monton del cargador\n");
        bmo::salir();
    };
    // SAFETY: el bloque es de este proceso y no se suelta nunca (forget).
    unsafe { MONTON.poner(bloque.base() as usize, PARA_MONTON as usize, bloque.handle()) };
    core::mem::forget(bloque);
    // 07-10: y lleno, que CREZCA por su tramo de la reserva (solo suyo).
    // SAFETY: el tramo lo quita `plataforma::RESERVA` de lo que ve la casa.
    unsafe { MONTON.crecer(plataforma::TRAMO_MONTON_BASE as usize, plataforma::TRAMO_MONTON as usize, plataforma::monton_hacer) };
}
mod monton;
mod obreros;
mod opciones;
mod plataforma;
mod perfil;
mod sonido;

use alloc::format;
use alloc::vec::Vec;
use bmo_proton_x::{importaciones, leer_cabeceras, resolver, tls, tramos, Permiso};
use bmo_userland as bmo;

#[global_allocator]
static MONTON: monton::Monton = monton::Monton::vacio();

/// El monton del cargador y de la casa (P0.4a: el `.exe` ya no pasa por el).
/// 64 MiB, el tope de un bloque; desde el 03-10 lo soltado se reusa.
const PARA_MONTON: u64 = 64 << 20;
/// Lo que se lee del principio del `.exe` para juzgarlo: sus cabeceras.
const CABECERAS: u64 = 64 << 10;

fn di(s: &str) {
    bmo::consola(s);
}

/// [`di`], y la misma linea en el DIARIO (04-10): lo del arranque que hay
/// que leer despues no puede depender de las 200 lineas de la consola.
fn di_y_diario(s: &str) {
    di(s);
    bmo_proton_x_casa::diario::apuntar_arranque(s);
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
/// Cada cuanto visita la alarma del kernel (EXPROPIAR): el cuanto de la
/// casa (`hilos::CUANTO_NS`).
const EXPROPIAR_MS: u32 = 4;

/// Donde queda el diario (`--diario`).
const RUTA_DIARIO: &[u8] = b"informe/diario.txt";
/// Lo que se lee de D: entre dos cesiones del turno (8 MiB: ~16 ms a la
/// velocidad del SSD; con 2 MiB eran cuatro veces mas llamadas).
const TROZO: u64 = 8 << 20;

/// Lo que se sabe de UN fichero (el `.exe` o una DLL): lo que importa, y
/// los nombres de DLL que aparecen en sus datos (candidatas a `LoadLibrary`).
struct Mirado {
    /// Sus cabeceras: quien las mira no las vuelve a leer (P0.4d, cada
    /// fichero se abre una vez por carga).
    pub(crate) pe: bmo_proton_x::Pe,
    imps: Vec<bmo_proton_x::Importacion>,
    /// Las RETRASADAS (`/DELAYLOAD`): se resuelven en su primera llamada.
    retrasadas: Vec<bmo_proton_x::Importacion>,
    /// Por que no se pudieron leer las retrasadas (el resto del fichero SI
    /// cuenta: antes, un fallo aqui se llevaba el fichero entero).
    fallo_retrasadas: Option<alloc::string::String>,
    en_vivo: Vec<alloc::string::String>,
    mide: u64,
    /// Como se parte su imagen (`tramos`): lo que el cargador le declara al
    /// kernel.
    partes: Result<Vec<bmo_proton_x::Tramo>, alloc::string::String>,
}

/// **Mirar un fichero PE** sin traerlo entero: las cabeceras y la seccion de
/// sus importaciones, cada una en su bloque, que se suelta al acabar (un
/// proceso tiene OCHO bloques vivos como mucho).
fn mirar(ruta: &[u8]) -> Result<Mirado, alloc::string::String> {
    let a = bmo::Archivo::reflejar(ruta).map_err(|_| alloc::string::String::from("no esta"))?;
    let mide = a.size();
    let n = (64u64 << 10).min(mide);
    let hb = bmo::Memoria::request(n.max(1)).ok_or("sin memoria para las cabeceras")?;
    let k = a.leer_en(&hb, 0, n);
    // SAFETY: `k` bytes que el kernel acaba de escribir en un bloque nuestro.
    let cab: Vec<u8> =
        unsafe { core::slice::from_raw_parts(hb.base() as *const u8, k as usize) }.to_vec();
    hb.soltar();
    let pe = bmo_proton_x::leer_cabeceras(&cab, mide).map_err(|f| format!("{f}"))?;
    let partes = tramos(&pe).map_err(|f| format!("{f}"));
    let rva = pe.importaciones.rva;
    let seccion_de = |rva: u32| {
        pe.secciones
            .iter()
            .find(|s| rva != 0 && (s.rva..s.rva + s.tam_en_fichero).contains(&rva))
    };
    let Some(sec) = seccion_de(rva) else {
        return Ok(Mirado {
            pe: pe.clone(),
            imps: Vec::new(),
            retrasadas: Vec::new(),
            fallo_retrasadas: None,
            en_vivo: Vec::new(),
            mide,
            partes,
        });
    };
    let (imps, retrasadas, en_vivo) = con_seccion(&a, sec, |trozo| {
        let imps = bmo_proton_x::importaciones_de_seccion(&pe, trozo, sec.rva)
            .map_err(|f| format!("{f}"))?;
        // Las retrasadas suelen vivir en la misma seccion (.rdata).
        let ret = seccion_de(pe.retrasadas.rva)
            .filter(|s| s.rva == sec.rva)
            .map(|_| bmo_proton_x::retrasadas_de_seccion(&pe, trozo, sec.rva));
        Ok((imps, ret, nombres_de_dll(trozo)))
    })?;
    // Si no estaban ahi, o sus nombres caen en otra seccion (libxess.dll,
    // metal 30-09): sobre una VENTANA con todas las secciones de datos, cada
    // una en su RVA. Y si tampoco, el fallo se apunta y el resto cuenta.
    let (retrasadas, fallo_retrasadas) = match retrasadas {
        Some(Ok(r)) => (r, None),
        _ if pe.retrasadas.rva == 0 => (Vec::new(), None),
        _ => match con_ventana(&a, &pe, |v, desde| {
            Ok(bmo_proton_x::retrasadas_de_seccion(&pe, v, desde))
        }) {
            Ok(Ok(r)) => (r, None),
            Ok(Err(f)) => (Vec::new(), Some(format!("{f}"))),
            Err(f) => (Vec::new(), Some(f)),
        },
    };
    Ok(Mirado {
        pe,
        imps,
        retrasadas,
        fallo_retrasadas,
        en_vivo,
        mide,
        partes,
    })
}

/// **Todas las secciones de DATOS en un bloque**, cada una en su RVA (lo de
/// en medio, a cero), y `f` sobre el bloque y la RVA donde empieza. Para las
/// tablas que apuntan de una seccion a otra.
fn con_ventana<R>(
    a: &bmo::Archivo,
    pe: &bmo_proton_x::Pe,
    f: impl FnOnce(&[u8], u32) -> Result<R, alloc::string::String>,
) -> Result<R, alloc::string::String> {
    let datos: Vec<&bmo_proton_x::Seccion> = pe
        .secciones
        .iter()
        .filter(|s| s.permiso() != bmo_proton_x::Permiso::Codigo && s.tam_en_fichero > 0)
        .collect();
    let (Some(desde), Some(hasta)) = (
        datos.iter().map(|s| s.rva).min(),
        datos.iter().map(|s| s.rva + s.tam_en_fichero).max(),
    ) else {
        return Err(alloc::string::String::from("sin secciones de datos"));
    };
    let tam = (hasta - desde) as u64;
    if tam > TOPE_SECCION {
        return Err(format!(
            "las secciones de datos pasan de 64 MiB ({} MiB)",
            tam >> 20
        ));
    }
    let b = bmo::Memoria::request(tam.max(1)).ok_or("sin memoria para la ventana de datos")?;
    // SAFETY: un bloque nuestro de `tam` bytes: a cero lo que ninguna llena.
    unsafe { core::ptr::write_bytes(b.base(), 0, tam as usize) };
    for s in &datos {
        let mut hecho = 0u64;
        let n = s.tam_en_fichero as u64;
        while hecho < n {
            let k = TROZO.min(n - hecho);
            let pos = s.desde as u64 + hecho;
            if a.saltar(pos) != pos || a.leer_en(&b, (s.rva - desde) as u64 + hecho, k) != k {
                b.soltar();
                return Err(format!("la seccion {} no se leyo entera", s.nombre));
            }
            hecho += k;
            bmo::yield_screen();
        }
    }
    // SAFETY: `tam` bytes de un bloque nuestro, ya escritos.
    let v = unsafe { core::slice::from_raw_parts(b.base() as *const u8, tam as usize) };
    let r = f(v, desde);
    b.soltar();
    r
}

/// **Una seccion del fichero en un bloque**, a trozos (cediendo el turno), y
/// `f` sobre sus bytes; el bloque se suelta al acabar.
fn con_seccion<R>(
    a: &bmo::Archivo,
    sec: &bmo_proton_x::Seccion,
    f: impl FnOnce(&[u8]) -> Result<R, alloc::string::String>,
) -> Result<R, alloc::string::String> {
    let tam = sec.tam_en_fichero as u64;
    if tam > TOPE_SECCION {
        return Err(format!("la seccion {} pasa de 64 MiB", sec.nombre));
    }
    let b = bmo::Memoria::request(tam.max(1)).ok_or("sin memoria para una seccion")?;
    // A TROZOS, cediendo el turno entre uno y otro: de una vez, 64 MiB de D:
    // tuvieron el CPU 1112 ms y el bus USB llego 1107 ms tarde (metal 29-09
    // 15:04, `latido tarde`: el raton y el teclado, congelados).
    let mut hecho = 0u64;
    while hecho < tam {
        let k = TROZO.min(tam - hecho);
        let pos = sec.desde as u64 + hecho;
        if a.saltar(pos) != pos || a.leer_en(&b, hecho, k) != k {
            b.soltar();
            return Err(format!("la seccion {} no se leyo entera", sec.nombre));
        }
        hecho += k;
        bmo::yield_screen();
    }
    // SAFETY: `tam` bytes que el kernel acaba de escribir en un bloque nuestro.
    let trozo = unsafe { core::slice::from_raw_parts(b.base() as *const u8, tam as usize) };
    let r = f(trozo);
    b.soltar();
    r
}

/// **Los nombres de DLL escritos en unos datos** (`"d3d12.dll"`, en ASCII o
/// en UTF-16): lo que el programa puede cargar EN VIVO con `LoadLibrary`, que
/// su tabla de importaciones no dice. Sin repetir, sin mayusculas que cuenten.
fn nombres_de_dll(d: &[u8]) -> Vec<alloc::string::String> {
    use alloc::string::String;
    let vale = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.';
    let mut v: Vec<String> = Vec::new();
    let mut poner = |n: String| {
        if n.len() > 4 && !v.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
            v.push(n);
        }
    };
    // ASCII: ".dll" y hacia atras mientras sea un nombre.
    let mut i = 0;
    while i + 4 <= d.len() {
        if i & 0xF_FFFF == 0 {
            bmo::yield_screen();
        }
        if d[i..i + 4].eq_ignore_ascii_case(b".dll") && d.get(i + 4).is_none_or(|&c| !vale(c)) {
            let mut a = i;
            while a > 0 && vale(d[a - 1]) && i - a < 64 {
                a -= 1;
            }
            if a < i {
                poner(String::from_utf8_lossy(&d[a..i + 4]).into_owned());
            }
            i += 4;
        } else {
            i += 1;
        }
    }
    // UTF-16: ".\0d\0l\0l\0".
    let mut i = 0;
    while i + 8 <= d.len() {
        if i & 0xF_FFFE == 0 {
            bmo::yield_screen();
        }
        let w = &d[i..i + 8];
        if w[1] == 0
            && w[3] == 0
            && w[5] == 0
            && w[7] == 0
            && w[0] == b'.'
            && w[2] | 0x20 == b'd'
            && w[4] | 0x20 == b'l'
            && w[6] | 0x20 == b'l'
        {
            let mut a = i;
            while a >= 2 && d[a - 1] == 0 && vale(d[a - 2]) && (i - a) / 2 < 64 {
                a -= 2;
            }
            if a < i {
                let n: String = (a..i + 8).step_by(2).map(|k| d[k] as char).collect();
                poner(n);
            }
            i += 8;
        } else {
            i += 2;
        }
    }
    v
}


#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut arg = [0u8; 96];
    let n = bmo::argumentos(&mut arg);
    let todo: &[u8] = if n == 0 {
        b"window/hola.exe"
    } else {
        &arg[..n]
    };
    // ** EL CENSO (29-09): `--censo <ruta>` no ejecuta nada: dice que DLL y que
    // funciones de Windows pide el `.exe` y cuantas tiene ya la casa.
    if let Some(r) = todo.strip_prefix(b"--censo ") {
        censo::censo(partir_ruta(r).0);
    }
    // Las opciones de delante (`--diario`, `--3060`, `--sin-obreros`), en
    // cualquier orden: `opciones.rs`.
    let (todo, con_diario) = opciones::leer(todo);
    // P4e: `window/x.exe lo de detras` -- la ruta hasta el primer espacio; lo
    // demas es la linea de ordenes del `.exe` (GetCommandLineW). N2: o entre
    // comillas, que las rutas de D: llevan espacios (`"d:Cyberpunk 2077/..."`).
    // ** LI2f (13-10): `--prueba-computo` no carga ningun `.exe`: corre el CS
    // de nBody por la puerta del computo de la 3060 y por la CPU, y los
    // compara byte a byte (la prueba del metal sin un juego).
    if todo == b"--prueba-computo" {
        poner_monton();
        computo3060::prueba();
    }
    let (ruta, resto) = partir_ruta(todo);
    let nombre = core::str::from_utf8(ruta).unwrap_or("?");
    let linea = core::str::from_utf8(resto).unwrap_or("");

    // -- 1. El fichero, SIN traerlo (P0.4a, 30-09): el de Cyberpunk son 57
    // MiB, y antes se leia entero, se copiaba al monton y se colocaba en una
    // tercera copia. Ahora solo las cabeceras pasan por el monton; cada
    // seccion va del disco a su sitio en los bloques de la imagen.
    let Ok(a) = bmo::Archivo::reflejar(ruta) else {
        di("PROTON-X: NO -- no encuentro ");
        di(nombre);
        di(" en el volumen\n");
        bmo::salir();
    };
    let mide = a.size();
    if mide == 0 {
        di("PROTON-X: NO -- el .exe esta vacio\n");
        bmo::salir();
    }

    poner_monton();
    // Y cada foto del pulso dice como va (si se llena de golpe o gotea).
    bmo_proton_x_casa::pulso::medir_monton(|| (MONTON.gastado(), MONTON.pico(), MONTON.crecido()));
    if con_diario {
        bmo_proton_x_casa::diario::diario(Some(RUTA_DIARIO));
    }
    // LI2j (12-10): la pantalla que ve el juego, antes de que pregunte.
    la3060::poner_pantalla();
    if la3060::en_modo_3060() {
        la3060::decir_si_esta_lista();
    }

    // -- 2 y 3. El veredicto, la forma, y como se parte: de las cabeceras.
    let n = CABECERAS.min(mide);
    let Some(hb) = bmo::Memoria::request(n) else {
        fin("sin memoria para las cabeceras")
    };
    let k = a.leer_en(&hb, 0, n);
    // SAFETY: `k` bytes que el kernel acaba de escribir en un bloque nuestro.
    let cab: Vec<u8> =
        unsafe { core::slice::from_raw_parts(hb.base() as *const u8, k as usize) }.to_vec();
    hb.soltar();
    let pe = leer_cabeceras(&cab, mide).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
    drop(cab);
    let partes = tramos(&pe).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));

    // La entrada tiene que caer en lo que se va a sellar: saltar a los datos
    // seria un #PF por NX, y saltar fuera, peor.
    if pe.entrada == 0
        || !pe.secciones.iter().any(|s| {
            s.permiso() == Permiso::Codigo
                && (s.rva..s.rva + s.tam_en_imagen()).contains(&pe.entrada)
        })
    {
        fin(&format!(
            "{nombre}: la entrada ({:#x}) no cae en una seccion de codigo",
            pe.entrada
        ));
    }

    // -- 3b y 4. P0.4b: el `.exe` Y SUS DLL, en UNA imagen declarada al
    // kernel (ver `cargador.rs`): se juzga una vez contra la RAM libre, y cada
    // seccion va del disco a su sitio. Sin topes de bloque.
    drop(a);
    let (imagen, modulos, tiempos) = cargador::declarar_y_colocar(ruta, pe, partes);
    let exe = &modulos[0];
    let base = exe.base;
    // P0.4b.8: el MAPA, con el diario: un `rip` de la autopsia (que solo
    // conoce el .bex) se lee aqui como modulo + RVA.
    if con_diario {
        let mut t = alloc::string::String::from("PROTON-X: mapa:");
        for m in &modulos {
            let nombre = m.nombre.rsplit('/').next().unwrap_or(&m.nombre);
            t.push_str(&format!(" {nombre} {:#x}+{:#x};", m.base, m.imagen().len()));
        }
        di(&format!("{t}\n"));
    }

    // -- 5. La casa lista (antes de registrar nada en ella), el TEB y el GS.
    entorno_windows::poner_teb(base);
    // SAFETY: un solo `.exe` por proceso, y todavia no se ha saltado.
    unsafe { bmo_proton_x_casa::empezar(plataforma::de_bmo()) };
    // Las DLL del juego, registradas (sus exportaciones) en el orden en que se
    // cargaron: dependencias primero. Asi el `.exe` y ellas se resuelven
    // contra la casa Y contra las otras.
    for m in &modulos[1..] {
        use bmo_proton_x::dll::{self, Destino};
        let exps = dll::exportaciones(&m.pe, m.imagen())
            .unwrap_or_else(|f| fin(&format!("{}: {f}", m.nombre)));
        let dadas = exps
            .into_iter()
            .map(|e| {
                let d = match &e.destino {
                    Destino::Rva(r) => m.base + *r as u64,
                    Destino::Reenvio { dll, funcion } => {
                        bmo_proton_x_casa::tabla(dll, funcion).unwrap_or(0)
                    }
                };
                (e.nombre, e.ordinal, d)
            })
            .collect();
        let entrada = if m.pe.entrada != 0 {
            m.base + m.pe.entrada as u64
        } else {
            0
        };
        bmo_proton_x_casa::modulos::registrar_dll(&m.nombre, m.base, entrada, dadas);
    }
    // Resolver cada uno; el TLS del `.exe`, leido de su imagen ya relocalizada.
    let mut funciones = 0usize;
    let mut tls_del_exe = None;
    let mut faltan: Vec<(alloc::string::String, bmo_proton_x::Importacion)> = Vec::new();
    let t_resolver = plataforma::ahora_ns();
    for (k, m) in modulos.iter().enumerate() {
        let img = m.imagen();
        let imps = importaciones(&m.pe, img).unwrap_or_else(|f| fin(&format!("{}: {f}", m.nombre)));
        if k == 0 {
            tls_del_exe =
                tls::leer(&m.pe, img, m.base).unwrap_or_else(|f| fin(&format!("{nombre}: {f}")));
        } else if let Some(t) =
            tls::leer(&m.pe, img, m.base).unwrap_or_else(|f| fin(&format!("{}: {f}", m.nombre)))
        {
            // P0.4b.9: el TLS de las DLL del juego (libxess_fg.dll lo tiene).
            if con_diario {
                di(&format!(
                    "PROTON-X: TLS de {}: {} B por hilo, {} callback(s)\n",
                    m.nombre,
                    t.bytes(),
                    t.callbacks.len()
                ));
            }
            bmo_proton_x_casa::hilos::registrar_tls_dll(t, m.base);
        }
        // P0.4b.7: lo que falta se APUNTA y se sigue con el siguiente: un
        // viaje al metal dice TODO lo que falta, no solo lo de la primera DLL.
        // 04-10: una DLL que el juego carga EN VIVO no lo para por lo que la
        // casa no tiene: eso va con TRAMPA (`en_vivo::resolver`).
        let r = if m.en_vivo {
            en_vivo::resolver(m, img, &imps)
        } else {
            resolver(img, &imps, bmo_proton_x_casa::tabla)
        };
        match r {
            Ok(()) => {}
            Err(bmo_proton_x::Fallo::Faltan(v)) => {
                faltan.extend(v.into_iter().map(|i| (m.nombre.clone(), i)))
            }
            Err(f) => fin(&format!("{}: {f}", m.nombre)),
        }
        funciones += imps.len();
    }
    if !faltan.is_empty() {
        let mut t = format!(
            "no arranca: faltan {} funcion(es) en la tabla de la casa:",
            faltan.len()
        );
        for (quien, i) in &faltan {
            t.push_str(&format!("\n  {quien} pide {}!{}", i.dll, i.funcion));
        }
        // ** LX2: y DESDE DONDE las pide el juego, y que hacia justo antes.
        for m in &modulos {
            let de_este: Vec<(alloc::string::String, u32)> = faltan
                .iter()
                .filter(|(quien, _)| *quien == m.nombre)
                .map(|(_, i)| (format!("{}!{}", i.dll, i.funcion), i.ranura))
                .collect();
            if !de_este.is_empty() {
                t.push_str(&lo_que_falta::contar(m, &de_este));
            }
        }
        fin(&t);
    }
    let t_resolver_ns = plataforma::ahora_ns() - t_resolver;

    // -- 6. SELLAR el codigo de cada uno: sin esto, saltar seria un #PF por NX.
    for m in &modulos {
        let t: Vec<(u64, bool)> = m
            .tramos
            .iter()
            .map(|t| (t.bytes as u64, t.codigo))
            .collect();
        bmo_proton_x_casa::memoria::registrar_tramos(m.base, &t);
        for i in m.de_codigo() {
            if let Err(no) = imagen.sellar(i) {
                fin(&format!(
                    "{}: SELLAR dice NO ({}): su codigo no se ejecuta sin sellar",
                    m.nombre,
                    no.frase()
                ));
            }
        }
    }
    di(&format!(
        "PROTON-X: {nombre}: {} B, PE32+ x86-64; en {:#x} (el enlazador queria {:#x}); {} DLL del juego; {} funcion(es) resueltas; codigo SELLADO, datos sin X; monton {} B en uso\n",
        mide,
        base,
        exe.pe.base,
        modulos.len() - 1,
        funciones,
        MONTON.gastado()
    ));
    // N4.4: si la corrida anterior murio saltando a 0, que era esa casilla.
    el_nulo::mirar(&modulos);
    // P4d: su directorio actual es el suyo (`window` para `window/x.exe`).
    bmo_proton_x_casa::ficheros::poner_directorio(
        nombre.rsplit_once('/').map(|(d, _)| d).unwrap_or(""),
    );
    // P4e: su nombre (GetModuleFileNameW) y su linea de ordenes; y si es un
    // juego de D:, su perfil de Windows en ESTRATOS (relevo 01-10, paso 3).
    let perfil = perfil::en_estratos(nombre);
    // A9 (06-10): los .bsf de la 3060 de este juego, en su carpeta de ESTRATOS.
    la3060::poner_carpeta_bsf(perfil.as_ref().map(|p| p.bsf.clone()));
    // A9b: y los mapas de la CPU (lo compilado de cada PSO), en la suya.
    bmo_proton_x_casa::enlaces::poner_carpeta(perfil.as_ref().map(|p| p.mapas.clone()));
    // M1 (14-10): y los sombreadores que la casa no sabe correr, guardados.
    bmo_proton_x_casa::rechazados::poner_carpeta(perfil.as_ref().map(|p| p.rechazados.clone()));
    bmo_proton_x_casa::proceso::poner_exe_con_perfil(
        nombre,
        linea,
        perfil.as_ref().map(|p| p.windows.as_str()),
    );
    bmo_proton_x_casa::ficheros::poner_capa(perfil.as_ref().map(|p| p.capa.as_str()));
    // -- 6d. P4: el TLS del hilo principal y los callbacks con PROCESS_ATTACH,
    // antes de la entrada, como el cargador de Windows.
    if let Some(t) = &tls_del_exe {
        di(&format!(
            "PROTON-X: TLS: {} B por hilo, {} callback(s)\n",
            t.bytes(),
            t.callbacks.len()
        ));
    }
    // SAFETY: el GS ya esta en el TEB, `empezar` hecho, la imagen sellada.
    unsafe { bmo_proton_x_casa::hilos::preparar_tls(tls_del_exe, base) };
    // -- 6e. P5a: los DllMain de las DLL del juego, con el GS puesto, en su
    // orden (dependencias primero).
    let ms = |ns: u64| ns / 1_000_000;
    di(&format!(
        "PROTON-X: tiempos: cabeceras {} ms; colocar {} ms (disco {} ms para {} MiB, {} MiB/s; el resto, copiar y relocalizar); resolver {} ms\n",
        ms(tiempos.cabeceras),
        ms(tiempos.colocar),
        ms(tiempos.disco),
        tiempos.bytes >> 20,
        (tiempos.bytes >> 20) * 1000 / ms(tiempos.disco).max(1),
        ms(t_resolver_ns)
    ));
    let mut t_dllmain = 0u64;
    if modulos.len() > 1 {
        di(&format!(
            "PROTON-X: los DllMain de {} DLL del juego\n",
            modulos.len() - 1
        ));
        // SAFETY: sus entradas son codigo sellado de DLL que acabamos de cargar.
        // Con el diario, cada una se dice ANTES: si una se cae, la ultima
        // linea (y la autopsia) dice cual.
        let t_dll = plataforma::ahora_ns();
        if let Err(f) = unsafe {
            bmo_proton_x_casa::modulos::iniciar_dlls_con(|n| {
                if con_diario {
                    di(&format!("PROTON-X: DllMain de {n}\n"))
                }
            })
        } {
            fin(&f);
        }
        t_dllmain = plataforma::ahora_ns() - t_dll;
    }
    let entrada = base + exe.pe.entrada as u64;
    di(&format!(
        "PROTON-X: los DllMain tardaron {} ms\n",
        t_dllmain / 1_000_000
    ));
    // -- 6f. EXPROPIAR (07-10): el codigo del `.exe` es del juego (las DLL
    // ya lo dijeron al registrarse), y la alarma, armada: cada 4 ms el
    // kernel visita la puerta de la casa.
    bmo_proton_x_casa::expropiar::juego(base, exe.pe.tam_imagen as u64);
    let (inicio, fin, buzon) = bmo_proton_x_casa::expropiar::puerta();
    match bmo::alarma::armar(inicio, fin, buzon, EXPROPIAR_MS) {
        Ok(()) => di(&format!("PROTON-X: EXPROPIAR: la alarma del kernel, cada {EXPROPIAR_MS} ms (puerta {inicio:#x}, {} B)\n", fin - inicio)),
        Err(no) => di(&format!("PROTON-X: EXPROPIAR: el kernel dijo NO a la alarma ({no:?}): los hilos solo sueltan el turno en las puertas\n")),
    }
    di("PROTON-X: salto a su entrada ----------------------------------\n");
    // La imagen vive hasta que el proceso muera (no hay soltar).
    core::mem::forget(imagen);

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
    //
    // Y con SU pila (tanda 29, 01-10): la de BMO-X para cualquier programa son
    // 64 KiB, y Windows le da al hilo principal la que pide el `.exe` en su
    // cabecera (SizeOfStackReserve). Cyberpunk desbordo la de 64 KiB en el
    // catch de C++ de Galaxy. El TEB dice la nueva (StackBase, StackLimit):
    // el desenrollado de la casa lee la pila por ahi.
    let tope = entorno_windows::pila_del_exe(base);
    let r: u64;
    // SAFETY: la entrada cae en una seccion de codigo del bloque sellado
    // (comprobado arriba) y la imagen esta colocada y resuelta. `r12` es
    // no volatil en Windows x64: el `.exe` lo devuelve como estaba. `tope`
    // es el final de una pila nuestra, R+W, alineado a 64 KiB.
    unsafe {
        core::arch::asm!(
            "mov r12, rsp",
            "mov rsp, {tope}",
            "sub rsp, 32",
            "call {entrada}",
            "mov rsp, r12",
            tope = in(reg) tope,
            entrada = in(reg) entrada,
            out("r12") _,
            lateout("rax") r,
            clobber_abi("win64"),
        );
    }
    let r = r as u32;
    fin_del_exe(r)
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    // H4.3: en una parte de un obrero (Ring 3, sin puerta) decirlo es un
    // syscall, o sea otro #UD; el `ud2` ya dice "esta parte fallo" y la casa
    // la rehace.
    if monton::en_parte() {
        // SAFETY: una excepcion a proposito; el obrero vuelve al kernel.
        unsafe { core::arch::asm!("ud2", options(noreturn)) };
    }
    // ** UN PANICO DENTRO DEL PANICO se va sin mas (metal 12-10): decir el
    // motivo o volcar el diario puede volver a fallar, y cada vuelta era otro
    // marco en la pila hasta desbordarla (`PILA DESBORDADA: 304 B bajo el
    // fondo`), con el primer motivo enterrado bajo diez copias.
    static EN_PANICO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    if EN_PANICO.swap(true, core::sync::atomic::Ordering::SeqCst) {
        bmo::salir();
    }
    // El motivo y el sitio, ENTEROS, en un bufer de la pila: el monton puede
    // ser justo lo que se acabo (metal 30-09: el primer contacto decia solo
    // "panico en el cargador", porque el mensaje no era un texto fijo).
    struct Bufer {
        b: [u8; 512],
        n: usize,
    }
    impl core::fmt::Write for Bufer {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let k = s.len().min(self.b.len() - self.n);
            self.b[self.n..self.n + k].copy_from_slice(&s.as_bytes()[..k]);
            self.n += k;
            Ok(())
        }
    }
    let mut t = Bufer { b: [0; 512], n: 0 };
    let _ = core::fmt::write(
        &mut t,
        format_args!("PROTON-X: panico en el cargador: {}", info.message()),
    );
    if let Some(l) = info.location() {
        let _ = core::fmt::write(&mut t, format_args!(" ({}:{})", l.file(), l.line()));
    }
    let _ = core::fmt::write(&mut t, format_args!("; monton {} B en uso (pico {}) de {} (crecio {} MiB por la reserva)\n", MONTON.gastado(), MONTON.pico(), MONTON.medida(), MONTON.crecido() >> 20));
    di(core::str::from_utf8(&t.b[..t.n]).unwrap_or("PROTON-X: panico en el cargador\n"));
    // Y al DIARIO, si esta encendido (03-10): lo apuntado, la ultima foto del
    // pulso y este motivo, sin pedir memoria. En el metal (02-10) el panico
    // se llevo el diario sin su final.
    bmo_proton_x_casa::diario::al_morir(&t.b[..t.n]);
    bmo::salir();
}
