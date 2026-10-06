//! **`hola.exe`, `teb.exe` y `ventana.exe` CORREN en esta CPU** (P1b, P1d y
//! P2, 27-09): el banco de verdad, con las DLL de la casa DE VERDAD.
//!
//! Las otras pruebas miran los bytes. Esta EJECUTA el `.exe` de Windows en el
//! anfitrion (Linux, x86-64), como `proton-x.bex` en el Ryzen:
//!
//! ```text
//!    partir     dos zonas seguidas: codigo (R+X, sin W) y datos (R+W, sin X)
//!    colocar    en una base que no es la suya, con las relocalizaciones
//!    resolver   contra `bmo_proton_x_casa::tabla`: LAS MISMAS kernel32,
//!               user32 y gdi32 que corren en BMO-X
//!    el GS      un TEB y un PEB (`bmo_proton_x::teb`) y `arch_prctl`, lo que
//!               en BMO-X es `TASK_OP_PON_GS`
//!    saltar     a su entrada con la convencion de Windows
//! ```
//!
//! Lo unico de mentira es la PLATAFORMA de debajo ([`plataforma`]): la consola
//! es un `Vec`, una superficie es memoria de este proceso y el buzon es un
//! guion de teclas y clics. Si el cargador o una DLL de la casa fallan, aqui
//! no sale un "distinto": sale un fallo de pagina.
//!
//! La memoria se pide con `mmap`/`mprotect` por `syscall`, sin `libc`.
//! `ExitProcess` no vuelve (en BMO-X es `salir`); aqui el proceso es el de las
//! pruebas y no puede morir, asi que la plataforma salta de vuelta a donde se
//! llamo a la entrada, con la pila de entonces: [`correr`].

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::sync::{Mutex, MutexGuard};

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bmo_proton_x::*;
use bmo_proton_x_casa::{Plataforma, Superficie};

const HOLA: &[u8] = include_bytes!("../../proton-x/prueba/hola.exe");
const TEB: &[u8] = include_bytes!("../../proton-x/prueba/teb.exe");
const VENTANA: &[u8] = include_bytes!("../../proton-x/prueba/ventana.exe");
const LIMPIA: &[u8] = include_bytes!("../../proton-x/prueba/limpia.exe");
const CUBO: &[u8] = include_bytes!("../../proton-x/prueba/cubo.exe");
const CUBO12: &[u8] = include_bytes!("../../proton-x/prueba/cubo12.exe");
const HILOS: &[u8] = include_bytes!("../../proton-x/prueba/hilos.exe");
const FICHEROS: &[u8] = include_bytes!("../../proton-x/prueba/ficheros.exe");
const CRT: &[u8] = include_bytes!("../../proton-x/prueba/crt.exe");
const TEXTO: &[u8] = include_bytes!("../../proton-x/prueba/texto.exe");
const ESPERAS: &[u8] = include_bytes!("../../proton-x/prueba/esperas.exe");
const CARPETAS: &[u8] = include_bytes!("../../proton-x/prueba/carpetas.exe");
const SISTEMA: &[u8] = include_bytes!("../../proton-x/prueba/sistema.exe");
const UCRT: &[u8] = include_bytes!("../../proton-x/prueba/ucrt.exe");
const STDIO: &[u8] = include_bytes!("../../proton-x/prueba/stdio.exe");
const PEEK: &[u8] = include_bytes!("../../proton-x/prueba/peek.exe");
const COMPILA: &[u8] = include_bytes!("../../proton-x/prueba/compila.exe");
const USADLL: &[u8] = include_bytes!("../../proton-x/prueba/usadll.exe");
const SALUDO_DLL: &[u8] = include_bytes!("../../proton-x/prueba/saludo.dll");
const SEH: &[u8] = include_bytes!("../../proton-x/prueba/seh.exe");
const TANDA1: &[u8] = include_bytes!("../../proton-x/prueba/tanda1.exe");
const TANDA2: &[u8] = include_bytes!("../../proton-x/prueba/tanda2.exe");
const TANDA3: &[u8] = include_bytes!("../../proton-x/prueba/tanda3.exe");
const TANDA3B: &[u8] = include_bytes!("../../proton-x/prueba/tanda3b.exe");
const TANDA4: &[u8] = include_bytes!("../../proton-x/prueba/tanda4.exe");
const TANDA3C: &[u8] = include_bytes!("../../proton-x/prueba/tanda3c.exe");
const TANDA5: &[u8] = include_bytes!("../../proton-x/prueba/tanda5.exe");
const TANDA6: &[u8] = include_bytes!("../../proton-x/prueba/tanda6.exe");
const TANDA7: &[u8] = include_bytes!("../../proton-x/prueba/tanda7.exe");
const TANDA8: &[u8] = include_bytes!("../../proton-x/prueba/tanda8.exe");
const TANDA9: &[u8] = include_bytes!("../../proton-x/prueba/tanda9.exe");
const TANDA10: &[u8] = include_bytes!("../../proton-x/prueba/tanda10.exe");
const TANDA11: &[u8] = include_bytes!("../../proton-x/prueba/tanda11.exe");
const TANDA12: &[u8] = include_bytes!("../../proton-x/prueba/tanda12.exe");
const TANDA13: &[u8] = include_bytes!("../../proton-x/prueba/tanda13.exe");
const DIARIO: &[u8] = include_bytes!("../../proton-x/prueba/diario.exe");
const TANDA14: &[u8] = include_bytes!("../../proton-x/prueba/tanda14.exe");
const TANDA4M: &[u8] = include_bytes!("../../proton-x/prueba/tanda4m.exe");
const TANDA14B: &[u8] = include_bytes!("../../proton-x/prueba/tanda14b.exe");
const TANDA15: &[u8] = include_bytes!("../../proton-x/prueba/tanda15.exe");
/// 02-10: el hilo que espera dando vueltas (`de_hoy.rs`).
const VUELTAS: &[u8] = include_bytes!("../../proton-x/prueba/vueltas.exe");
/// E1.1 de la ESCALERA (05-10): `D3D12HelloWindow` de Microsoft, el `.exe`
/// de verdad compilado de su fuente (`prueba/muestras/HACER.txt`).
const HWINDOW: &[u8] = include_bytes!("../../proton-x/prueba/hwindow.exe");
/// E1.2 a E1.6 (05-10): los demas Hello de Microsoft, cada uno en su carpeta
/// del volumen con sus `.cso` (`tests/corre/muestras.rs`).
const HTRIANG: &[u8] = include_bytes!("../../proton-x/prueba/htriang.exe");
const HTEXTURE: &[u8] = include_bytes!("../../proton-x/prueba/htexture.exe");
const HCBUFFER: &[u8] = include_bytes!("../../proton-x/prueba/hcbuffer.exe");
const HFRAMES: &[u8] = include_bytes!("../../proton-x/prueba/hframes.exe");
const HBUNDLES: &[u8] = include_bytes!("../../proton-x/prueba/hbundles.exe");
/// E2.2 (05-10): DynamicIndexing, el bindless (N5.4).
const DYNINDEX: &[u8] = include_bytes!("../../proton-x/prueba/dynindex.exe");
const COMPUTO: &[u8] = include_bytes!("../../proton-x/prueba/computo.exe");
const NBODY: &[u8] = include_bytes!("../../proton-x/prueba/nbody.exe");
const INDIRECT: &[u8] = include_bytes!("../../proton-x/prueba/indirect.exe");
const PREDICA: &[u8] = include_bytes!("../../proton-x/prueba/predica.exe");
/// N5.13 y N5.14 (05-10): las instancias y los buferes de vertices de varias ranuras.
const INSTANCIAS: &[u8] = include_bytes!("../../proton-x/prueba/instancias.exe");
/// N5.3b y N5.3c (05-10): las vistas en la raiz, los UAV de textura y con tipo, y ClearUnorderedAccessView.
const VISTAS_EXE: &[u8] = include_bytes!("../../proton-x/prueba/vistas.exe");
// N5.16 (05-10): los render targets de float.
const HDR_EXE: &[u8] = include_bytes!("../../proton-x/prueba/hdr.exe");
// 05-10: los UAV escritos desde un DIBUJO (de pixeles y de vertices).
const UAVPIXEL_EXE: &[u8] = include_bytes!("../../proton-x/prueba/uavpixel.exe");
// N5.16b (05-10): los floats de un canal, sus UAV y DepthClipEnable = FALSE.
const FLOTANTE1_EXE: &[u8] = include_bytes!("../../proton-x/prueba/flotante1.exe");
// 05-10: el stencil (la fila de la tabla 7.2 de la ESCALERA).
const STENCIL_EXE: &[u8] = include_bytes!("../../proton-x/prueba/stencil.exe");
// 05-10: lo que quedaba: enteros, UAV sin destino y del GS, el plano de stencil y SV_StencilRef.
const RESTOS_EXE: &[u8] = include_bytes!("../../proton-x/prueba/restos.exe");
/// E2.5 (05-10): las olas de verdad (Wave*, Quad*), en el computo y en los pixeles.
const OLAS_EXE: &[u8] = include_bytes!("../../proton-x/prueba/olas.exe");
/// D4.4 (05-10): las derivadas (ddx, ddy, finas y gruesas) y la mip de un muestreo.
const DERIVADAS_EXE: &[u8] = include_bytes!("../../proton-x/prueba/derivadas.exe");
/// E2.1 (05-10): listas grabadas desde varios hilos, y las colas con vallas.
const MULTIHILO_EXE: &[u8] = include_bytes!("../../proton-x/prueba/multihilo.exe");
/// 06-10: los UAV de texturas 3D y de arrays (la niebla volumetrica, las cascadas).
const VOLUMEN_EXE: &[u8] = include_bytes!("../../proton-x/prueba/volumen.exe");
/// 06-10: las root signatures 1.1 y las de dentro del sombreador.
const FIRMAS_EXE: &[u8] = include_bytes!("../../proton-x/prueba/firmas.exe");
/// 06-10: el computo de un posproceso (bindless, SampleLevel, RWTexture2D, Interlocked).
const POSTPRO_EXE: &[u8] = include_bytes!("../../proton-x/prueba/postpro.exe");
/// 06-10: las vistas que cambian el tipo (D2.7): UAV, SRV y render targets de otro formato.
const TIPOS_EXE: &[u8] = include_bytes!("../../proton-x/prueba/tipos.exe");
/// 06-10: ClearUnorderedAccessView en el formato de la vista y con rectangulos (A2).
const LIMPIEZA_EXE: &[u8] = include_bytes!("../../proton-x/prueba/limpieza.exe");
/// 06-10: una escena 3D DURA, contra la imagen que guardo Windows (A11).
const ESCENA_EXE: &[u8] = include_bytes!("../../proton-x/prueba/escena.exe");
const TANDA16: &[u8] = include_bytes!("../../proton-x/prueba/tanda16.exe");
const TANDA17: &[u8] = include_bytes!("../../proton-x/prueba/tanda17.exe");
const TANDA18: &[u8] = include_bytes!("../../proton-x/prueba/tanda18.exe");
const TANDA19: &[u8] = include_bytes!("../../proton-x/prueba/tanda19.exe");
const TANDA19M: &[u8] = include_bytes!("../../proton-x/prueba/tanda19m.exe");
const TANDA20: &[u8] = include_bytes!("../../proton-x/prueba/tanda20.exe");
const TANDA21: &[u8] = include_bytes!("../../proton-x/prueba/tanda21.exe");
const TANDA22: &[u8] = include_bytes!("../../proton-x/prueba/tanda22.exe");
const TANDA22D: &[u8] = include_bytes!("../../proton-x/prueba/tanda22d.dll");
const TANDA23: &[u8] = include_bytes!("../../proton-x/prueba/tanda23.exe");
const TANDA24: &[u8] = include_bytes!("../../proton-x/prueba/tanda24.exe");
const TANDA25: &[u8] = include_bytes!("../../proton-x/prueba/tanda25.exe");
const TANDA26: &[u8] = include_bytes!("../../proton-x/prueba/tanda26.exe");
const TANDA27: &[u8] = include_bytes!("../../proton-x/prueba/tanda27.exe");
const TANDA28: &[u8] = include_bytes!("../../proton-x/prueba/tanda28.exe");
const TANDA29: &[u8] = include_bytes!("../../proton-x/prueba/tanda29.exe");
const TANDA30: &[u8] = include_bytes!("../../proton-x/prueba/tanda30.exe");
const TANDA31: &[u8] = include_bytes!("../../proton-x/prueba/tanda31.exe");
const TANDA32: &[u8] = include_bytes!("../../proton-x/prueba/tanda32.exe");
/// La TANDA 33 (01-10): secur32 por su ruta y la tabla SSPI de curl.
const TANDA33: &[u8] = include_bytes!("../../proton-x/prueba/tanda33.exe");
/// La TANDA 34 (01-10): un HMODULE de la casa es una imagen PE que se lee.
const TANDA34: &[u8] = include_bytes!("../../proton-x/prueba/tanda34.exe");
/// La TANDA 35 (01-10): IDXGIAdapter::GetDesc.
const TANDA35: &[u8] = include_bytes!("../../proton-x/prueba/tanda35.exe");
const TANDA36: &[u8] = include_bytes!("../../proton-x/prueba/tanda36.exe");
const TANDA37: &[u8] = include_bytes!("../../proton-x/prueba/tanda37.exe");
/// La TANDA 38 (01-10): IDXGIAdapter::EnumOutputs, el monitor.
const TANDA38: &[u8] = include_bytes!("../../proton-x/prueba/tanda38.exe");
/// La TANDA 39 (02-10): VirtualQuery de toda direccion.
const TANDA39: &[u8] = include_bytes!("../../proton-x/prueba/tanda39.exe");
/// La TANDA 41 (02-10): cuantos procesadores, igual por todas partes.
const TANDA41: &[u8] = include_bytes!("../../proton-x/prueba/tanda41.exe");
/// La TANDA 42 (02-10): lo que la tarjeta dice de si.
const TANDA42: &[u8] = include_bytes!("../../proton-x/prueba/tanda42.exe");
const TANDA43: &[u8] = include_bytes!("../../proton-x/prueba/tanda43.exe");
/// La TANDA 44 (02-10): buferes grandes y montones de verdad.
const TANDA44: &[u8] = include_bytes!("../../proton-x/prueba/tanda44.exe");
/// La TANDA 45 (02-10): ID3D12Device1 a 10.
const TANDA45: &[u8] = include_bytes!("../../proton-x/prueba/tanda45.exe");
/// La TANDA 46 (02-10): un monton sobre memoria del .exe.
const TANDA46: &[u8] = include_bytes!("../../proton-x/prueba/tanda46.exe");
/// La TANDA 47 (02-10): lo que vkd3d-proton tiene y la casa no tenia.
const TANDA47: &[u8] = include_bytes!("../../proton-x/prueba/tanda47.exe");
const TANDA48: &[u8] = include_bytes!("../../proton-x/prueba/tanda48.exe");

/// Como se llama el `.exe` que corre y lo que se escribio detras (P4e: su
/// GetModuleFileNameW y su GetCommandLineW).
static NOMBRE: Mutex<(&str, &str)> = Mutex::new(("window/prueba.exe", ""));
const CUBO_DATOS_H: &str = include_str!("../../proton-x/prueba/cubo_datos.h");

#[path = "../examples/cubo_datos.rs"]
mod cubo_datos;

/// Los `.exe` comparten la vuelta, lo dicho y la pantalla (estaticos): uno a
/// la vez. Cada prueba lo coge ENTERA, no solo mientras corre su `.exe`: lo
/// que mira despues (la pantalla, los dibujos) otra prueba lo borraria.
static UNO_A_LA_VEZ: Mutex<()> = Mutex::new(());

fn uno_a_la_vez() -> MutexGuard<'static, ()> {
    UNO_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

/// **Ninguna prueba toca la casa sin la vuelta** (03-10).
///
/// La casa es de UNA tarea, como en BMO-X: su estado son estaticos sin
/// cerrojo. Aqui cada prueba es un hilo de Linux, y una que la tocaba sin
/// [`uno_a_la_vez`] (`los_api_set_downlevel_son_su_dll`: solo llamaba a
/// `tabla`, que parece leer y ESCRIBE) hacia `push` en el mismo Vec que el
/// `.exe` de al lado. Una vuelta de cada muchas del banco, SIGSEGV o "double
/// free"; con su `tabla` repetida unos segundos, 4 vueltas de 6.
///
/// Esto lee las fuentes de las pruebas y no deja pasar otra igual. Mira el
/// cuerpo de cada `#[test]`: si nombra la casa, tiene que coger la vuelta
/// (directa, o por `tanda`/`tanda_y`, que la cogen).
#[test]
fn ninguna_prueba_toca_la_casa_sin_la_vuelta() {
    let casa = concat!("bmo_proton_x_", "casa::");
    for (fichero, fuente) in [("corre.rs", include_str!("corre.rs")), ("corre/de_hoy.rs", include_str!("corre/de_hoy.rs"))] {
        for trozo in fuente.split("\n#[test]\n").skip(1) {
            let cuerpo = &trozo[..trozo.find("\n}\n").map_or(trozo.len(), |i| i + 3)];
            let nombre = cuerpo.split("fn ").nth(1).and_then(|s| s.split('(').next()).unwrap_or("?");
            let toca = cuerpo.contains(casa);
            let vuelta = ["uno_a_la_vez()", "tanda(", "tanda_y("].iter().any(|m| cuerpo.contains(m));
            assert!(!toca || vuelta, "{fichero}: `{nombre}` toca la casa sin `uno_a_la_vez()` -- en paralelo con otro .exe, corrompe su estado");
        }
    }
}

const PROT_LEE: u64 = 1;
const PROT_ESCRIBE: u64 = 2;
const PROT_EJECUTA: u64 = 4;

unsafe fn syscall6(n: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    let r: u64;
    core::arch::asm!("syscall", inlateout("rax") n => r, in("rdi") a, in("rsi") b, in("rdx") c,
        in("r10") d, in("r8") e, in("r9") f, lateout("rcx") _, lateout("r11") _, options(nostack));
    r
}

fn mmap(bytes: u64) -> u64 {
    // MAP_PRIVATE | MAP_ANONYMOUS
    let r = unsafe { syscall6(9, 0, bytes, PROT_LEE | PROT_ESCRIBE, 0x22, u64::MAX, 0) };
    assert!(r < (-4096i64) as u64, "mmap dijo {r:#x}");
    r
}

fn mprotect(dir: u64, bytes: u64, prot: u64) {
    let r = unsafe { syscall6(10, dir, bytes, prot, 0, 0, 0) };
    assert_eq!(r, 0, "mprotect dijo {r:#x}");
}

fn munmap(dir: u64, bytes: u64) {
    unsafe { syscall6(11, dir, bytes, 0, 0, 0, 0) };
}

// ============================ LA PLATAFORMA DE MENTIRA ============================

static DICHO: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Las superficies que pidio el `.exe`: (pixeles, ancho, alto), y cuantas
/// veces se mostraron y se presentaron.
static PANTALLA: Mutex<Vec<(Vec<u32>, u32, u32)>> = Mutex::new(Vec::new());
static MOSTRADAS: AtomicU32 = AtomicU32::new(0);
static PRESENTADAS: AtomicU32 = AtomicU32::new(0);
static DORMIDAS: AtomicU32 = AtomicU32::new(0);
/// La huella (`bmo_cubo::referencia::huella`) de cada superficie PRESENTADA,
/// en orden: lo que se vio en la ventana, fotograma a fotograma.
static VISTAS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
/// Cuantos Present deja el banco antes de sacar al `.exe` con 0xF00D. Las
/// muestras de Microsoft solo salen cerrando su ventana (WM_CLOSE), que en
/// BMO-X es la X del escritorio: la que la pone, la devuelve a 1000.
static TOPE_PRESENTES: AtomicU32 = AtomicU32::new(1000);
/// Los Present (0, 1...) de los que el banco guarda los PIXELES, no solo la
/// huella: un juez que mira la imagen (E1.2 de la ESCALERA, 05-10). Lo pone
/// la prueba; `correr_exe` vacia lo guardado.
static GUARDAR_FOTOS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
/// `(present, pixeles 0x00RRGGBB, ancho, alto)` de cada uno de esos.
static FOTOS: Mutex<Vec<(u32, Vec<u32>, u32, u32)>> = Mutex::new(Vec::new());

/// **El guion del buzon.** Cada `evento` saca el siguiente; un 0 del guion es
/// "ahora no hay nada", y se gasta. Asi se ve lo que hace `GetMessageW` cuando
/// la cola se queda libre: pintar.
static GUION: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::new());
/// Las lineas de EL REGISTRO (`[registro] ... fps ...`), apartadas de lo
/// dicho: dependen del reloj, y lo dicho se compara letra a letra.
static REGISTRO: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn escribir(b: &[u8]) {
    DICHO.lock().unwrap().extend_from_slice(b);
}

fn superficie(ancho: u32, alto: u32) -> Option<Superficie> {
    let mut p = PANTALLA.lock().unwrap();
    p.push((vec![0u32; (ancho * alto) as usize], ancho, alto));
    let i = p.len() - 1;
    // El Vec no se mueve mientras viva la prueba: su puntero vale.
    Some(Superficie { pixeles: p[i].0.as_mut_ptr(), ancho, alto, stride: ancho, dato: i as u64 })
}

fn mostrar(_s: &Superficie) -> bool {
    MOSTRADAS.fetch_add(1, Ordering::SeqCst);
    true
}

/// Un `.exe` que presenta para siempre (un bucle de juego al que no le llega
/// su tecla de salir) tampoco cuelga el banco: a los mil Present (o a los de
/// [`TOPE_PRESENTES`]), fuera con 0xF00D.
fn presentar(s: &Superficie) {
    let n = PRESENTADAS.fetch_add(1, Ordering::SeqCst);
    if n >= TOPE_PRESENTES.load(Ordering::SeqCst) {
        salir(0xF00D);
    }
    // SAFETY: la superficie es un Vec de PANTALLA (ver `superficie`), vivo.
    let px = unsafe { core::slice::from_raw_parts(s.pixeles, (s.stride * s.alto) as usize) };
    VISTAS.lock().unwrap().push(bmo_cubo::referencia::huella(px));
    if GUARDAR_FOTOS.lock().unwrap().contains(&n) {
        FOTOS.lock().unwrap().push((n, px.to_vec(), s.ancho, s.alto));
    }
}

fn evento(_s: &Superficie) -> u64 {
    GUION.lock().unwrap().pop_front().unwrap_or(0)
}

/// Un `.exe` que espera para siempre no puede colgar el banco: a las mil
/// siestas, fuera con 0xDEAD.
fn dormir() {
    if DORMIDAS.fetch_add(1, Ordering::SeqCst) > 1000 {
        salir(0xDEAD);
    }
    // De verdad, como en BMO-X (4 ms alli): los plazos de P4 (Sleep, WaitFor*
    // con tiempo) cuentan con que el reloj avance mientras se duerme.
    std::thread::sleep(std::time::Duration::from_micros(500));
}

/// Donde volver al salir, y con que pila: los deja [`correr`].
static mut PILA: u64 = 0;
static mut VUELTA: u64 = 0;
static mut MARCO: u64 = 0;
static mut SALIO: u32 = u32::MAX;

fn salir(codigo: u32) -> ! {
    // Como la app: con el diario, el anillo al fichero (con el GS del .exe
    // todavia puesto).
    bmo_proton_x_casa::diario::al_salir(codigo);
    unsafe {
        SALIO = codigo;
        core::arch::asm!(
            "mov rsp, [rip + {pila}]",
            "jmp qword ptr [rip + {vuelta}]",
            pila = sym PILA,
            vuelta = sym VUELTA,
            options(noreturn)
        );
    }
}

fn plataforma() -> Plataforma {
    Plataforma { escribir, salir, superficie, mostrar, presentar, evento, dormir, poner_gs, ahora_ns, dibujar: dibujar_y_la_3060, sellar_codigo, soltar_codigo, leer_fichero, escribir_fichero, memoria, fecha, listar, carpetas: Some(CARPETAS_DEL_BANCO), reserva: Some(de_hoy::reserva_del_banco()), trozos: Some(de_hoy::TROZOS_DEL_BANCO), sonido: None }
}

/// Codigo SELLADO, como `MEM_OP_SELLAR`: memoria nueva, los bytes, y de
/// R+W a R+X (sin W). Cuantos se sellaron y cuantos se soltaron, para el banco.
static SELLADOS: AtomicU32 = AtomicU32::new(0);
static SOLTADOS: AtomicU32 = AtomicU32::new(0);

fn sellar_codigo(bytes: &[u8]) -> Option<u64> {
    let n = bytes.len().div_ceil(4096) as u64 * 4096;
    let base = mmap(n);
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), base as *mut u8, bytes.len()) };
    mprotect(base, n, PROT_LEE | PROT_EJECUTA);
    SELLADOS.fetch_add(1, Ordering::SeqCst);
    Some(base)
}

fn soltar_codigo(base: u64, bytes: usize) {
    munmap(base, bytes.div_ceil(4096) as u64 * 4096);
    SOLTADOS.fetch_add(1, Ordering::SeqCst);
}

/// El volumen del banco: un directorio del anfitrion, uno por proceso de
/// pruebas (las rutas de la casa van relativas a el).
fn volumen() -> std::path::PathBuf {
    static RAIZ: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    RAIZ.get_or_init(|| {
        let r = std::env::temp_dir().join(format!("proton-x-volumen-{}", std::process::id()));
        std::fs::create_dir_all(r.join("window")).unwrap();
        r
    })
    .clone()
}

fn leer_fichero(ruta: &[u8]) -> Option<Vec<u8>> {
    std::fs::read(volumen().join(std::str::from_utf8(ruta).ok()?)).ok()
}

/// Crear, quitar y renombrar en el volumen del banco (lo que en BMO-X hace
/// ESTRATOS, relevo 01-10 paso 4b).
const CARPETAS_DEL_BANCO: bmo_proton_x_casa::Carpetas = bmo_proton_x_casa::Carpetas {
    crear: |r| std::str::from_utf8(r).is_ok_and(|r| std::fs::create_dir(volumen().join(r)).is_ok()),
    quitar: |r| {
        std::str::from_utf8(r).is_ok_and(|r| {
            let p = volumen().join(r);
            if p.is_dir() { std::fs::remove_dir(p).is_ok() } else { std::fs::remove_file(p).is_ok() }
        })
    },
    renombrar: |r, nuevo| match (std::str::from_utf8(r), std::str::from_utf8(nuevo)) {
        (Ok(r), Ok(n)) => {
            let p = volumen().join(r);
            p.parent().is_some_and(|d| std::fs::rename(&p, d.join(n)).is_ok())
        }
        _ => false,
    },
};

fn escribir_fichero(ruta: &[u8], bytes: &[u8]) -> bool {
    let Ok(r) = std::str::from_utf8(ruta) else { return false };
    std::fs::write(volumen().join(r), bytes).is_ok()
}

/// Una arena del monton de Windows (P4e): memoria del anfitrion a ceros, que
/// no se suelta (vive lo que el proceso, como en BMO-X).
fn memoria(bytes: usize) -> Option<u64> {
    let forma = std::alloc::Layout::from_size_align(bytes, 1 << 16).ok()?;
    // SAFETY: una forma de medida no nula.
    let p = unsafe { std::alloc::alloc_zeroed(forma) };
    (!p.is_null()).then_some(p as u64)
}

/// La fecha del banco: la del anfitrion (P4f2).
fn fecha() -> Option<u64> {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Lo que hay en una carpeta del volumen del banco (P4f3).
fn listar(ruta: &[u8]) -> Option<Vec<bmo_proton_x::ficheros::Entrada>> {
    let r = volumen().join(std::str::from_utf8(ruta).ok()?);
    let mut v = Vec::new();
    for e in std::fs::read_dir(r).ok()? {
        let e = e.ok()?;
        let m = e.metadata().ok()?;
        // Las fechas del anfitrion en FILETIME (100 ns desde 1601), como las da
        // el NTFS del disco Personal (01-10).
        let ft = |t: std::io::Result<std::time::SystemTime>| t.ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64 / 100 + 116_444_736_000_000_000);
        let creado = ft(m.created());
        let fechas = [if creado != 0 { creado } else { ft(m.modified()) }, ft(m.modified()), ft(m.accessed())];
        v.push(bmo_proton_x::ficheros::Entrada { nombre: e.file_name().to_string_lossy().into_owned(), carpeta: m.is_dir(), bytes: m.len(), fechas, atributos: if m.is_dir() { 0x10 } else { 0x20 } });
    }
    Some(v)
}

/// La hora del banco: la del anfitrion, desde que empezo el proceso.
fn ahora_ns() -> u64 {
    static ORIGEN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGEN.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

/// **Saltar a la entrada** con la pila alineada y su sombra, y volver aqui
/// tanto si la entrada vuelve como si llama a `ExitProcess`. Todo registro que
/// el `.exe` pueda tocar se da por perdido; `rbx` y `rbp` se guardan a mano
/// (el compilador no deja nombrarlos).
unsafe fn correr(entrada: u64) -> u32 {
    SALIO = u32::MAX;
    core::arch::asm!(
        "push rbx",
        "push rbp",
        "mov [rip + {marco}], rsp",
        "and rsp, -16",
        "sub rsp, 32",
        "mov [rip + {pila}], rsp",
        "lea rax, [rip + 3f]",
        "mov [rip + {vuelta}], rax",
        "call r11",
        "3:",
        "mov rsp, [rip + {marco}]",
        "pop rbp",
        "pop rbx",
        in("r11") entrada,
        marco = sym MARCO,
        pila = sym PILA,
        vuelta = sym VUELTA,
        clobber_abi("sysv64"),
        out("r12") _, out("r13") _, out("r14") _, out("r15") _,
    );
    SALIO
}

/// `arch_prctl(ARCH_SET_GS)`: el GS de ESTE hilo de Linux.
fn poner_gs(v: u64) {
    let r = unsafe { syscall6(158, 0x1001, v, 0, 0, 0, 0) };
    assert_eq!(r, 0, "arch_prctl(ARCH_SET_GS) dijo {r:#x}");
}

/// **Cargar y correr un `.exe`** como `proton-x.bex`: partir, colocar en una
/// base que no es la suya, resolver, codigo R+X, datos sin X; y si `con_teb`,
/// un TEB y un PEB en el GS. Devuelve (con que salio, lo que dijo, la base).
/// **P5a: cargar una DLL propia** de la carpeta del `.exe` (y, antes, las que
/// ELLA pide), como el cargador de Windows: colocar donde caiga, relocalizar,
/// resolver, sellar el codigo y decir a la casa lo que exporta. Su DllMain
/// corre luego, en `iniciar_dlls`.
fn cargar_dll(dll: &str, dir: &str) {
    use bmo_proton_x::dll::{self, Destino};
    if bmo_proton_x_casa::modulos::es_de_la_casa(dll) || bmo_proton_x_casa::modulos::cargada(dll) {
        return;
    }
    let f = dll::fichero(dll);
    let d = std::fs::read(volumen().join(dir).join(&f)).unwrap_or_else(|_| panic!("{f}: no esta junto al .exe"));
    let pe = leer(&d).unwrap();
    assert!(pe.es_dll, "{f} no es una DLL");
    let partes = partir(&pe).unwrap();
    let base = mmap((partes.codigo + partes.datos) as u64);
    let mut img = colocar(&pe, &d, base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    for i in &imps {
        cargar_dll(&i.dll, dir);
    }
    resolver(&mut img, &imps, bmo_proton_x_casa::tabla).unwrap();
    // P0.4b.9: su TLS, como `proton-x.bex` (libxess_fg.dll lo tiene).
    if let Some(t) = tls::leer(&pe, &img, base).unwrap() {
        bmo_proton_x_casa::hilos::registrar_tls_dll(t, base);
    }
    let exps = dll::exportaciones(&pe, &img).unwrap();
    unsafe { core::ptr::copy_nonoverlapping(img.as_ptr(), base as *mut u8, img.len()) };
    mprotect(base, partes.codigo as u64, PROT_LEE | PROT_EJECUTA);
    let dadas = exps
        .into_iter()
        .map(|e| {
            let dir = match &e.destino {
                Destino::Rva(r) => base + *r as u64,
                Destino::Reenvio { dll, funcion } => bmo_proton_x_casa::tabla(dll, funcion).unwrap_or(0),
            };
            (e.nombre, e.ordinal, dir)
        })
        .collect();
    let entrada = if pe.entrada != 0 { base + pe.entrada as u64 } else { 0 };
    bmo_proton_x_casa::modulos::registrar_dll(&f, base, entrada, dadas);
}

fn correr_exe(_uno: &MutexGuard<'static, ()>, exe: &[u8], con_teb: bool, guion: &[u64]) -> (u32, Vec<u8>, u64) {
    *GUION.lock().unwrap() = guion.iter().copied().collect();
    DICHO.lock().unwrap().clear();
    PANTALLA.lock().unwrap().clear();
    VISTAS.lock().unwrap().clear();
    FOTOS.lock().unwrap().clear();
    for c in [&MOSTRADAS, &PRESENTADAS, &DORMIDAS, &SELLADOS, &SOLTADOS] {
        c.store(0, Ordering::SeqCst);
    }
    // SAFETY: un `.exe` a la vez (el cerrojo de arriba), antes de saltar. Va
    // ANTES de cargar nada: reinicia lo que la casa sabe de las DLL propias.
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    let (nombre, resto) = *NOMBRE.lock().unwrap();
    let dir_exe = nombre.rsplit_once('/').map_or("", |(d, _)| d);
    let pe = leer(exe).unwrap();
    let partes = partir(&pe).unwrap();
    let total = (partes.codigo + partes.datos) as u64;
    let base = mmap(total);
    assert_ne!(base, pe.base, "en una base que NO es la suya");
    let mut img = colocar(&pe, exe, base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    // P5a: las DLL que la casa no tiene, de la carpeta del `.exe`.
    for i in &imps {
        cargar_dll(&i.dll, dir_exe);
    }
    resolver(&mut img, &imps, bmo_proton_x_casa::tabla).unwrap();
    unsafe { core::ptr::copy_nonoverlapping(img.as_ptr(), base as *mut u8, img.len()) };
    mprotect(base, partes.codigo as u64, PROT_LEE | PROT_EJECUTA);
    bmo_proton_x_casa::memoria::registrar_tramos(base, &[(partes.codigo as u64, true), (partes.datos as u64, false)]);
    let hilo_mem = (teb::TEB_BYTES + teb::PEB_BYTES) as u64;
    let mem = mmap(hilo_mem);
    if con_teb {
        // La pila: la de este hilo de prueba, alrededor de donde estamos.
        let rsp: u64;
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let h = teb::Hilo {
            teb: mem,
            peb: mem + teb::TEB_BYTES as u64,
            pila_tope: rsp + (64 << 10),
            pila_fondo: rsp - (256 << 10),
            proceso: 7,
            hilo: 42,
            base_imagen: base,
        };
        // SAFETY: `mem` son TEB_BYTES + PEB_BYTES recien pedidos, R+W.
        let t = unsafe { core::slice::from_raw_parts_mut(mem as *mut u8, hilo_mem as usize) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
        poner_gs(mem);
    }
    // P4: el TLS, como el cargador de Windows (y como `proton-x.bex`).
    let t = tls::leer(&pe, &img, base).unwrap();
    // SAFETY: GS puesto (si hay TEB), `empezar` hecho, la imagen en su sitio.
    unsafe { bmo_proton_x_casa::hilos::preparar_tls(t, base) };
    // P4d: como `run sys/proton-x.bex window/x.exe`, su directorio es `window`.
    bmo_proton_x_casa::ficheros::poner_directorio(dir_exe);
    bmo_proton_x_casa::proceso::poner_exe(nombre, resto);
    // P5a: los DllMain de las DLL propias, con el GS ya puesto.
    unsafe { bmo_proton_x_casa::modulos::iniciar_dlls() }.unwrap();
    let salio = unsafe { correr(base + pe.entrada as u64) };
    if con_teb {
        poner_gs(0);
    }
    munmap(mem, hilo_mem);
    munmap(base, total);
    let todo = DICHO.lock().unwrap().clone();
    let texto = String::from_utf8_lossy(&todo);
    let (reg, resto): (Vec<&str>, Vec<&str>) = texto.split_inclusive('\n').partition(|l| l.starts_with("[registro] "));
    *REGISTRO.lock().unwrap() = reg.iter().map(|l| l.to_string()).collect();
    let dicho = if reg.is_empty() { todo } else { resto.concat().into_bytes() };
    (salio, dicho, base)
}

#[test]
fn hola_exe_corre_en_esta_cpu_y_dice_su_frase() {
    let pe = leer(HOLA).unwrap();
    // hola.exe: cabeceras y .text en dos paginas; .rdata y .reloc en otras dos.
    assert_eq!(partir(&pe).unwrap(), Partes { codigo: 2 * PAGINA, datos: 2 * PAGINA });
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, HOLA, false, &[]);
    assert_eq!(salio, 0, "ExitProcess con 0: escribio la frase entera");
    assert_eq!(dicho.as_slice(), b"hola desde un .exe de Windows\r\n");
}

/// **P1d en el anfitrion**: `teb.exe` lee su TEB, su PEB, su base, su pila y su
/// LastError por `gs:`, y dice `bien` seis veces. Sin el GS puesto, el primer
/// `gs:[0x30]` leeria de la direccion 0x30: un fallo de pagina, no un "MAL".
#[test]
fn teb_exe_encuentra_su_teb_y_su_peb_en_gs() {
    let uno = uno_a_la_vez();
    let (salio, dicho, base) = correr_exe(&uno, TEB, true, &[]);
    let texto = String::from_utf8(dicho).unwrap();
    assert_eq!(texto.matches("  bien  ").count(), 6, "{texto}");
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(texto.contains(&format!("PEB+0x10 es la base de esta imagen 0x{base:016x}")), "{texto}");
    assert!(texto.contains("SetLastError deja su valor en gs:[0x68] 0x0000000000001234"), "{texto}");
    assert!(texto.contains("GetCurrent*Id son los del TEB 0x000000070000002a"), "{texto}");
    assert!(texto.ends_with("teb.exe: el TEB y el PEB son los de Windows\r\n"), "{texto}");
    assert_eq!(salio, 0, "ExitProcess con el numero de fallos: 0");
}

#[test]
fn una_seccion_de_datos_en_las_paginas_del_codigo_no_se_parte() {
    let mut pe = leer(HOLA).unwrap();
    // .rdata movida a la pagina de .text, a proposito.
    pe.secciones[1].rva = pe.secciones[0].rva + 0x100;
    assert_eq!(partir(&pe), Err(Fallo::NoSeParte(".rdata".into())));
}

/// **P2 en el anfitrion**: `ventana.exe`, una ventana Win32 de manual, con el
/// `user32` y el `gdi32` de la casa. El guion: una letra, un clic en (10, 20)
/// y `q`, con un momento libre entre cada uno (ahi Windows pinta).
#[test]
fn ventana_exe_abre_su_ventana_pinta_y_obedece_al_teclado_y_al_raton() {
    const HAY: u64 = 1 << 8;
    const PULSADA: u64 = 1 << 9;
    let letra = |c: u8| 1 << 62 | HAY | PULSADA | c as u64;
    let clic = 1 << 63 | HAY | PULSADA | 1 | 10 << 16 | 20 << 32;
    let guion = [letra(b'b'), 0, 0, clic, 0, 0, letra(b'q')];
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, VENTANA, true, &guion);
    assert_eq!(dicho, b"", "ni un aviso: la casa supo hacer todo lo que pidio");
    // PostQuitMessage((letras << 8) | clics): una letra (la q cierra) y un clic.
    assert_eq!(salio, 0x101);
    assert_eq!(MOSTRADAS.load(Ordering::SeqCst), 1, "ShowWindow la ofrecio UNA vez");
    // Tres dibujos: UpdateWindow, tras la letra y tras el clic. La q destruye
    // la ventana antes de que su ultimo WM_PAINT salga, como en Windows.
    assert_eq!(PRESENTADAS.load(Ordering::SeqCst), 3);
    let p = PANTALLA.lock().unwrap();
    assert_eq!(p.len(), 1);
    let (px, ancho, alto) = (&p[0].0, p[0].1, p[0].2);
    assert_eq!((ancho, alto), (320, 200));
    let en = |x: u32, y: u32| px[(y * ancho + x) as usize];
    assert_eq!(en(0, 0), 0xFFFF_D700, "el marco dorado");
    assert_eq!(en(10, 20), 0xFFFF_FFFF, "el cuadrado blanco donde cayo el clic");
    // El degradado con el tinte 1 (una letra): en (100, 100), r=128 g=0x80 b=79.
    assert_eq!(en(100, 100), 0xFF80_804F);
}

/// **P3a en el anfitrion**: `limpia.exe`, el esqueleto de todo programa D3D12
/// (dispositivo, cola, cadena de intercambio, RTV, lista, valla y evento), con
/// el `d3d12` y el `dxgi` de la casa. Limpia y presenta al arrancar, y otra
/// vez con el color siguiente en cada letra.
#[test]
fn limpia_exe_limpia_su_ventana_con_d3d12_y_presenta_por_dxgi() {
    let letra = |c: u8| 1 << 62 | 1 << 8 | 1 << 9 | c as u64;
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, LIMPIA, true, &[letra(b'b'), 0, letra(b'q')]);
    assert_eq!(String::from_utf8_lossy(&dicho), "", "ni un aviso ni un hueco que falte");
    // PostQuitMessage(presentados): el de arrancar y el de la letra.
    assert_eq!(salio, 2);
    assert_eq!(PRESENTADAS.load(Ordering::SeqCst), 2);
    let p = PANTALLA.lock().unwrap();
    assert_eq!(p.len(), 1, "una ventana");
    let (px, ancho, alto) = (&p[0].0, p[0].1, p[0].2);
    assert_eq!((ancho, alto), (320, 200));
    // El color 2 (0.75, 0.25, 0.0) en R8G8B8A8, presentado en la superficie
    // BGRA: R = 191, G = 64, B = 0. Todos los pixeles.
    assert!(px.iter().all(|&c| c == 0xFFBF_4000), "primero {:#x}", px[0]);
}

/// **`prueba/cubo_datos.h` es la salida de su fabrica**: nadie lo toco a mano,
/// y lleva los bits de `bmo-cubo` y los `.dxil` de hoy.
#[test]
fn cubo_datos_h_es_lo_que_fabrica_su_ejemplo() {
    assert!(cubo_datos::texto() == CUBO_DATOS_H, "rehacer: cargo run -p bmo-proton-x-casa --example cubo_datos > platform/shared/proton-x/prueba/cubo_datos.h");
}

/// **P3b2 y P3b3 en el anfitrion**: `cubo.exe`, un programa D3D12 ENTERO
/// (root signature serializada, PSO con los DXIL de dxc, buferes UPLOAD
/// mapeados, y cada fotograma Reset con el PSO, raiz, CBV, viewport, destino,
/// limpiar, vertices, indices y DrawIndexedInstanced). Dos cosas:
///
/// - lo que cada dibujo VE (la captura de P3b2): los vertices, los indices y
///   las constantes de los fotogramas 0, 30 y 60 son, bit a bit, los de
///   `bmo-cubo` (lo que X4 subio a la 3060);
/// - lo que se VIO (P3b3): la ventana de cada Present tiene la HUELLA de lo
///   que D3D12 dibujo en la 3060 bajo Windows. Los sombreadores de dxc
///   corridos en la CPU y la trama de la casa, sin el juez de por medio.
#[test]
fn cubo_exe_monta_la_tuberia_entera_y_cada_dibujo_ve_lo_de_x1() {
    let letra = |c: u8| 1 << 62 | 1 << 8 | 1 << 9 | c as u64;
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, CUBO, true, &[letra(b'b'), 0, letra(b'b'), 0, letra(b'q')]);
    assert_eq!(
        String::from_utf8_lossy(&dicho),
        "",
        "ni un aviso ni un hueco que falte: la casa supo hacer todo el dibujo"
    );
    let vistas = VISTAS.lock().unwrap().clone();
    let huellas: Vec<u64> = bmo_cubo::referencia::HUELLAS.iter().map(|&(_, h)| h).collect();
    assert_eq!(vistas, huellas, "lo que se vio en cada Present es lo que dibujo la 3060 (fotogramas 0, 30, 60)");
    // PostQuitMessage(presentados): el de arrancar y el de cada letra.
    assert_eq!(salio, 3);
    assert_eq!(PRESENTADAS.load(Ordering::SeqCst), 3);

    // P3b3b: un PSO, un bloque de codigo sellado; y lo que se VIO arriba lo
    // dibujaron sus sombreadores NATIVOS (el ejecutor de la casa).
    assert_eq!(SELLADOS.load(Ordering::SeqCst), 1, "un PSO: un bloque sellado");
    let dibujos = bmo_proton_x_casa::tuberia::dibujos();
    assert_eq!(dibujos.len(), 3, "un DrawIndexedInstanced por fotograma");
    let v = bmo_cubo::vertices();
    let i: Vec<u32> = bmo_cubo::indices().iter().map(|&x| x as u32).collect();
    for (d, f) in dibujos.iter().zip([0u32, 30, 60]) {
        assert_eq!((d.vs.as_str(), d.ps.as_str()), ("vertice", "pixel"), "los puntos de entrada de cubo.hlsl");
        assert_eq!((d.topologia, d.cuantos, d.instancias), (4, 36, 1), "TRIANGLELIST, 36 indices, una instancia");
        assert_eq!((d.descarte, d.antihorario), (3, false), "descarte de las caras de detras, horario delante");
        assert_eq!(d.viewport, [0.0, 0.0, 1280.0, 720.0, 0.0, 1.0]);
        assert_ne!(d.destino, 0);
        // Los vertices, leidos A TRAVES del input layout: pos, normal, color.
        assert_eq!(d.vertices.len(), v.len());
        for (leido, x) in d.vertices.iter().zip(&v) {
            let bits = |s: &[f32]| s.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
            assert_eq!(leido.len(), 3);
            assert_eq!(bits(&leido[0]), bits(&x.pos));
            assert_eq!(bits(&leido[1]), bits(&x.normal));
            assert_eq!(bits(&leido[2]), bits(&x.color));
        }
        assert_eq!(d.indices, i);
        // b0: wvp, world y luz del fotograma `f`, y el resto del bufer a cero.
        let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
        let mut esperado: Vec<u8> = c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect();
        esperado.resize(256, 0);
        assert!(d.constantes == esperado, "las constantes del fotograma {f}");
    }

    // La ventana: 1280x720; la esquina, el fondo de X1 (16, 16, 24) en BGRA.
    let p = PANTALLA.lock().unwrap();
    assert_eq!((p[0].1, p[0].2), (1280, 720));
    assert_eq!(p[0].0[0], 0xFF10_1018);
}

/// **P3c4 en el anfitrion**: `cubo12.exe`, el cubo por el CAMINO de BMOX-12:
/// IDXGIFactory6, EnumAdapterByGpuPreference, GetDesc1, D3D12CreateDevice
/// con el adaptador, CheckFeatureSupport, la cadena como IDXGISwapChain3
/// (GetCurrentBackBufferIndex), D3DCompile del HLSL de BMOX-12 (los .cso que
/// `sombras.exe` compilo en Windows), y la PROFUNDIDAD (D32, DSV,
/// ClearDepthStencilView con su float en xmm3, PSO con LESS). Lo que se ve en
/// cada Present es lo que dibujo la 3060. Y su --fotograma: GetCopyableFootprints,
/// un bufer READBACK y CopyTextureRegion; el .exe saca la huella de lo copiado
/// y la compara con la de la 3060 (sale con 3 solo si las tres cuadran).
#[test]
fn cubo12_exe_va_por_el_camino_de_bmox12_y_se_ve_lo_de_la_3060() {
    let uno = uno_a_la_vez();
    let dir = volumen().join("window/sombras");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for h in ["f3ef42a0", "4d67f5e4"] {
        let cso = std::fs::read(format!("../proton-x/prueba/sombras/{h}.cso")).unwrap();
        std::fs::write(dir.join(format!("{h}.cso")), cso).unwrap();
    }
    *NOMBRE.lock().unwrap() = ("window/cubo12.exe", "");
    let letra = |c: u8| 1 << 62 | 1 << 8 | 1 << 9 | c as u64;
    let (salio, dicho, _) = correr_exe(&uno, CUBO12, true, &[letra(b'b'), 0, letra(b'b'), 0, letra(b'q')]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    assert_eq!(String::from_utf8_lossy(&dicho), "", "ni un aviso ni un hueco que falte");
    let vistas = VISTAS.lock().unwrap().clone();
    let huellas: Vec<u64> = bmo_cubo::referencia::HUELLAS.iter().map(|&(_, h)| h).collect();
    assert_eq!(vistas, huellas, "lo que se vio en cada Present es lo que dibujo la 3060 (fotogramas 0, 30, 60)");
    assert_eq!(salio, 3, "tres Present y las tres huellas LEIDAS cuadran (+0x100 por huella mala; 0xE1xx/0xE2xx, un paso que fallo)");
    let dibujos = bmo_proton_x_casa::tuberia::dibujos();
    assert_eq!(dibujos.len(), 3);
    for d in &dibujos {
        // Un SM5 no guarda el nombre de su entrada (el DXIL si).
        assert_eq!((d.vs.as_str(), d.ps.as_str()), ("", ""));
        assert_eq!((d.descarte, d.cuantos), (3, 36));
    }
}

/// **P3b4c.9 Z1: un fotograma que ya esta en la pantalla no se copia.**
/// El mismo `cubo12.exe`, con un ejecutor que dice `en_pantalla` en cada
/// dibujo de su back buffer: el `.exe` no nota nada (sus lecturas cuadran
/// igual) y `Present` NO toca la superficie -- sigue a cero, fotograma a
/// fotograma. Con el ejecutor de siempre, la prueba de arriba ve lo de la
/// 3060: la diferencia es Z1 y nada mas.
#[test]
fn z1_un_fotograma_en_la_pantalla_no_se_copia_a_la_superficie() {
    let uno = uno_a_la_vez();
    let dir = volumen().join("window/sombras");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for h in ["f3ef42a0", "4d67f5e4"] {
        let cso = std::fs::read(format!("../proton-x/prueba/sombras/{h}.cso")).unwrap();
        std::fs::write(dir.join(format!("{h}.cso")), cso).unwrap();
    }
    *NOMBRE.lock().unwrap() = ("window/cubo12.exe", "");
    EN_PANTALLA.store(true, Ordering::SeqCst);
    let letra = |c: u8| 1 << 62 | 1 << 8 | 1 << 9 | c as u64;
    let (salio, dicho, _) = correr_exe(&uno, CUBO12, true, &[letra(b'b'), 0, letra(b'b'), 0, letra(b'q')]);
    EN_PANTALLA.store(false, Ordering::SeqCst);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    assert_eq!(String::from_utf8_lossy(&dicho), "", "ni un aviso ni un hueco que falte");
    assert_eq!(salio, 3, "el .exe no nota nada: sus tres huellas leidas cuadran");
    let p = PANTALLA.lock().unwrap();
    let cero = bmo_cubo::referencia::huella(&vec![0u32; (p[0].1 * p[0].2) as usize]);
    let vistas = VISTAS.lock().unwrap().clone();
    assert_eq!(vistas.len(), 3);
    assert!(vistas.iter().all(|&h| h == cero), "Present no copio nada: la superficie sigue a cero");
}

/// **P4 en el anfitrion**: `hilos.exe`, hilos, TLS y sincronizacion de
/// Windows con los hilos COOPERATIVOS de la casa. Lo que dice no depende del
/// orden en que corran los hilos: es lo mismo que dice en Windows.
#[test]
fn hilos_exe_tiene_hilos_tls_y_sincronizacion_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, HILOS, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 19, "{texto}");
    assert!(texto.ends_with("hilos.exe: los hilos son los de Windows\r\n[salio 0x0]"), "{texto}");
    assert_eq!(salio, 0, "{texto}");
}

/// **P4d en el anfitrion**: `ficheros.exe` crea, escribe, lee, se mueve y
/// vuelve a crear un fichero junto al `.exe` (`window/pxtest.txt` del volumen
/// del banco), con los errores de Windows donde tocan.
#[test]
fn ficheros_exe_lee_y_escribe_ficheros_como_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, FICHEROS, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 16, "{texto}");
    assert!(texto.ends_with("ficheros.exe: los ficheros son los de Windows\r\n[salio 0x0]"), "{texto}");
    assert_eq!(std::fs::read(volumen().join("window/pxtest.txt")).unwrap(), b"corto", "y en el volumen queda lo ultimo que escribio");
}

/// **P4e en el anfitrion**: `crt.exe` pide y suelta del monton de Windows
/// (dos mil bloques, 8 MiB, un HeapCreate), reserva y hace paginas con
/// VirtualAlloc, y lee su nombre, su linea y su entorno.
#[test]
fn crt_exe_tiene_la_memoria_y_el_proceso_de_windows() {
    let uno = uno_a_la_vez();
    *NOMBRE.lock().unwrap() = ("window/crt.exe", "-nivel 3");
    let (salio, dicho, _) = correr_exe(&uno, CRT, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 35, "{texto}");
    assert!(texto.ends_with("crt.exe: la memoria y el proceso son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f en el anfitrion**: `texto.exe` pasa texto entre UTF-8 y UTF-16,
/// escribe por WriteConsoleW, y carga d3d12.dll y sus funciones por
/// LoadLibraryW + GetProcAddress, sobre la tabla de la casa.
#[test]
fn texto_exe_tiene_el_texto_la_consola_y_los_modulos_de_windows() {
    let uno = uno_a_la_vez();
    *NOMBRE.lock().unwrap() = ("window/texto.exe", "");
    let (salio, dicho, _) = correr_exe(&uno, TEXTO, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert!(texto.contains("  bien  WriteConsoleW escribe UTF-16 en la consola\r\n"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 21, "{texto}");
    assert!(texto.ends_with("texto.exe: el texto y los modulos son los de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f2 en el anfitrion**: `esperas.exe` -- mutex (y abandonado),
/// temporizadores, WaitOnAddress importado de un API set, FLS con sus
/// callbacks, DuplicateHandle y la hora del dia.
#[test]
fn esperas_exe_tiene_las_esperas_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, ESPERAS, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("MAL"), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 24, "{texto}");
    assert!(texto.ends_with("esperas.exe: las esperas son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f3 en el anfitrion**: `carpetas.exe` busca con comodines, abre su
/// carpeta, pregunta a un handle, cambia una medida, copia, y oye los NO de
/// Windows. Dos veces seguidas: lo que deja no le estorba a la segunda.
#[test]
fn carpetas_exe_tiene_las_carpetas_de_windows() {
    let uno = uno_a_la_vez();
    for vez in 0..2 {
        *NOMBRE.lock().unwrap() = ("window/carpetas.exe", "");
        let (salio, dicho, _) = correr_exe(&uno, CARPETAS, true, &[]);
        *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
        let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
        assert!(!texto.contains("  MAL   "), "vuelta {vez}: {texto}");
        assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
        assert_eq!(texto.matches("  bien  ").count(), 34, "{texto}");
        assert!(texto.ends_with("carpetas.exe: las carpetas son las de Windows\r\n[salio 0x0]"), "{texto}");
    }
    assert_eq!(std::fs::read(volumen().join("window/pzc.txt")).unwrap(), b"abc");
}

/// **P4f4 en el anfitrion**: `sistema.exe` -- NtReadFile/NtWriteFile,
/// ProcessPrng, OVERLAPPED, FormatMessageW, SetStdHandle, los nombres del
/// sistema, la red que no esta y los procesos; sale con TerminateProcess.
#[test]
fn sistema_exe_tiene_lo_demas_de_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, SISTEMA, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 22, "{texto}");
    assert!(texto.ends_with("sistema.exe: lo demas es lo de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4f5 en el anfitrion**: `ucrt.exe` -- el arranque del CRT de MSVC
/// (argv, entorno, _initterm), sus tablas de salida, su monton, memoria y
/// cadenas, importado de api-ms-win-crt-* y vcruntime140.dll; y su ultima
/// linea la dice una funcion de _crt_atexit que corre exit().
#[test]
fn ucrt_exe_tiene_el_crt_de_msvc() {
    let uno = uno_a_la_vez();
    *NOMBRE.lock().unwrap() = ("window/ucrt.exe", "-nivel 3");
    let (salio, dicho, _) = correr_exe(&uno, UCRT, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert!(texto.contains("__p___argc y __p___argv: argv[argc] es NULL 0x0000000000000003"), "tres argumentos: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 19, "{texto}");
    assert!(texto.ends_with("ucrt.exe: el CRT es el de Windows (dicho desde _crt_atexit)\r\n[salio 0x0]"), "{texto}");
}

/// **P4f5 en el anfitrion**: `stdio.exe` -- el printf del CRT de MSVC
/// (__stdio_common_* con las banderas de las cabeceras del UCRT), y stdout y
/// stderr en modo texto.
#[test]
fn stdio_exe_tiene_el_printf_de_msvc() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, STDIO, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert!(!texto.replace("\r\n", "").contains('\n'), "modo texto: cada \\n sale \\r\\n: {texto:?}");
    assert_eq!(texto.matches("  bien  ").count(), 15, "{texto}");
    assert!(texto.ends_with("stdio.exe: el printf es el de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **Una tanda de Cyberpunk**: corre `exe` (con ese nombre, si lo dice) y
/// exige `bien` lineas bien, ninguna MAL, ni un aviso, y su ultima linea.
fn tanda(exe: &[u8], nombre: Option<&'static str>, bien: usize, fin: &str) {
    tanda_y(exe, nombre, bien, fin, || ());
}

/// [`tanda`], y luego `despues`, con la vuelta cogida todavia.
fn tanda_y(exe: &[u8], nombre: Option<&'static str>, bien: usize, fin: &str, despues: impl FnOnce()) {
    let uno = uno_a_la_vez();
    if let Some(n) = nombre {
        *NOMBRE.lock().unwrap() = (n, "");
    }
    let (salio, dicho, _) = correr_exe(&uno, exe, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), bien, "{texto}");
    assert!(texto.ends_with(&format!("{fin}\r\n[salio 0x0]")), "{texto}");
    despues();
}

/// **P3c1 en el anfitrion**: `peek.exe` -- lo chico que le faltaba a
/// BMOX-12 (PeekMessageW, AdjustWindowRect, LoadCursorW, LoadLibraryExA,
/// oleaut32, RoOriginateErrorW, ceil y floor con el double en xmm0).
#[test]
fn peek_exe_tiene_lo_chico_de_bmox12() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, PEEK, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 12, "{texto}");
    assert!(texto.ends_with("peek.exe: lo chico de BMOX-12 es lo de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P3c2 en el anfitrion**: `compila.exe` dos veces. La primera no hay
/// compilador: E_FAIL, el blob de errores lo dice, y quedan la fuente y el
/// pedido en `window/sombras/`. Luego se deja un `.cso` por cada uno (lo que
/// haria `sombras.exe` en Windows) y la segunda D3DCompile da esos bytes.
#[test]
fn compila_exe_paga_d3dcompile_una_vez() {
    let uno = uno_a_la_vez();
    let dir = volumen().join("window/sombras");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    *NOMBRE.lock().unwrap() = ("window/compila.exe", "");
    let (salio, dicho, _) = correr_exe(&uno, COMPILA, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert_eq!(texto.matches("sin compilador: E_FAIL").count(), 2, "{texto}");
    let mut pendientes: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    pendientes.sort();
    assert_eq!(pendientes.len(), 4, "dos .hls y dos .ent: {pendientes:?}");
    for p in pendientes.iter().filter(|p| p.ends_with(".ent")) {
        let ent = std::fs::read(dir.join(p)).unwrap();
        assert!(ent.starts_with(b"VSMain\nvs_5_0\n") || ent.starts_with(b"PSMain\nps_5_0\n"), "{:?}", String::from_utf8_lossy(&ent));
        let mut cso = b"DXBC".to_vec();
        cso.extend_from_slice(&[0x5A; 60]);
        std::fs::write(dir.join(p.replace(".ent", ".cso")), cso).unwrap();
    }
    let (salio, dicho, _) = correr_exe(&uno, COMPILA, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "con los .cso, ni un aviso: {texto}");
    assert!(texto.contains("D3DCompile(VSMain, vs_5_0): un DXBC 0x0000000000000040"), "{texto}");
    assert!(texto.contains("D3DCompile(PSMain, ps_5_0): un DXBC 0x0000000000000040"), "{texto}");
    assert!(texto.ends_with("compila.exe: D3DCompile dice la verdad\r\n[salio 0x0]"), "{texto}");
}

/// **P5a en el anfitrion**: `usadll.exe` con su `saludo.dll` al lado (una DLL
/// que NO es de Windows ni de la casa): importaciones resueltas contra ella,
/// su DllMain antes de la entrada, y LoadLibrary / GetModuleHandle /
/// GetProcAddress (por nombre y por ordinal) sobre ella.
#[test]
fn usadll_exe_carga_una_dll_propia() {
    let uno = uno_a_la_vez();
    std::fs::write(volumen().join("window/saludo.dll"), SALUDO_DLL).unwrap();
    *NOMBRE.lock().unwrap() = ("window/usadll.exe", "");
    let (salio, dicho, _) = correr_exe(&uno, USADLL, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 8, "{texto}");
    assert!(texto.ends_with("usadll.exe: la DLL propia es como en Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P4c en el anfitrion**: `seh.exe`, las excepciones estructuradas de C con
/// el despachador de la casa: __except con filtro y constante, __finally al
/// desenrollar, CONTINUE_SEARCH y CONTINUE_EXECUTION, tres marcos arriba con
/// sus registros, RtlVirtualUnwind, una en un hilo, y la ultima sin manejar
/// llega a SetUnhandledExceptionFilter, que sale con ExitProcess(fallos).
#[test]
fn seh_exe_lanza_coge_y_desenrolla_como_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, SEH, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 9, "{texto}");
    assert!(texto.contains("una excepcion en un hilo, cogida en ese hilo 0x0000000000000007"), "{texto}");
    assert!(texto.ends_with("seh.exe: las excepciones son las de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **P3b4a en el banco**: mientras esto este puesto, cada lote que dibuja la
/// casa se traduce TAMBIEN para la 3060 (una vez por PSO y paso) y su
/// vertice se comprueba contra la casa.
struct La3060 {
    almacen: bmo_proton_x_sm86::pso::Almacen,
    lotes: usize,
    vertices: usize,
    datos_max: usize,
    fallos: Vec<String>,
    /// P3b4c: la PUERTA (la receta VRN2 que la app manda al kernel).
    puerta: bmo_proton_x_sm86::puerta::Puerta,
    recetas: usize,
    z_limpias: usize,
    rt_limpios: usize,
    /// Lotes con Z que la puerta de la app SI manda (la sombra del kernel).
    z_mandadas: usize,
}
static LA3060: Mutex<Option<La3060>> = Mutex::new(None);

/// P3b4c.9 Z1: el ejecutor dice que cada dibujo en un back buffer de la
/// cadena quedo EN LA PANTALLA (como la 3060 con la pantalla dada).
static EN_PANTALLA: AtomicBool = AtomicBool::new(false);

/// El ejecutor del banco: el de la casa, y lo de la 3060 si se pide.
fn dibujar_y_la_3060(l: &lote::Lote, d: &mut trama::Destino) -> Result<trama::Cuenta, lote::NoDibuja> {
    if let Some(b) = LA3060.lock().unwrap().as_mut() {
        comprobar_la_3060(b, l);
    }
    let cadena = d.cadena;
    bmo_proton_x_casa::nativo::dibujar(l, d).map(|c| trama::Cuenta { en_pantalla: cadena && EN_PANTALLA.load(Ordering::SeqCst), ..c })
}

/// Traduce el lote para la 3060 y, con el SIMULADOR, corre su cuerpo con los
/// registros que el pegamento cargaria DE LOS DATOS (el cbuffer y el bufer de
/// vertices del juego tal cual): cada salida, los bits de la casa.
fn comprobar_la_3060(b: &mut La3060, l: &lote::Lote) {
    use bmo_proton_x_sm86::simula::{correr, Maquina};
    use bmo_proton_x_sm86::{emitir_con, Abi, Precarga};
    b.lotes += 1;
    let en = l.enlace;
    let t = match b.almacen.dar(en, l.entradas, l.paso as u32) {
        Ok(t) => t.clone(),
        Err(e) => return b.fallos.push(format!("no va a la 3060: {e:?}")),
    };
    let mut datos = vec![0u8; 1 << 16];
    let Some(n) = t.datos(l.cb, l.vertices, &mut datos) else { return b.fallos.push(String::from("los DATOS no se arman")) };
    b.datos_max = b.datos_max.max(n);
    let ev = emitir_con(&en.vs, 64, Abi::Registros).unwrap();
    let els = bmo_proton_x_sm86::pso::elementos(en, l.entradas).unwrap();
    let f32_de = |o: usize| u32::from_le_bytes([datos[o], datos[o + 1], datos[o + 2], datos[o + 3]]);
    let mut ids: Vec<u32> = l.ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let (mut casa, mut regs) = (vec![[0f32; 4]; en.vs.salidas], Vec::new());
    for id in ids {
        let v = 16 * t.filas as usize + id as usize * l.paso;
        let mut m = Maquina::nueva([&[]; 8]);
        for (i, r) in m.r.iter_mut().enumerate() {
            *r = 0x7FC0_0000 | i as u32;
        }
        for &p in &ev.precargas {
            match p {
                Precarga::Entrada { elemento, componente, reg } => {
                    let el = els[elemento as usize];
                    m.r[reg as usize] = if componente < el.componentes { f32_de(v + el.desde as usize + 4 * componente as usize) } else if componente == 3 { 0x3F80_0000 } else { 0 };
                }
                Precarga::Fila { fila, reg } => {
                    for k in 0..4 {
                        m.r[reg as usize + k] = f32_de(16 * fila as usize + 4 * k);
                    }
                }
                Precarga::Asa { .. } => panic!("un de vertice no muestrea"),
            }
        }
        correr(&ev.codigo, &mut m).unwrap();
        // La casa, con SU lectura del input layout (la de `lote::en_cpu`).
        let ent: Vec<[f32; 4]> = en.desde_ia.iter().map(|&f| lote::entrada(&l, f, id, 0)).collect();
        en.vs.correr(&ent, l.cb, &mut casa, &mut regs);
        for (e, s) in casa.iter().enumerate() {
            for k in 0..4 {
                if m.r[4 * e + k] != s[k].to_bits() && !(f32::from_bits(m.r[4 * e + k]).is_nan() && s[k].is_nan()) && en.vs.ops.iter().any(|o| matches!(*o, bmo_proton_x::dxil::programa::Op::Salida { elemento, componente, .. } if elemento as usize == e && componente as usize == k)) {
                    b.fallos.push(format!("vertice {id}, salida {e}.{k}: la 3060 {:08x}, la casa {:08x}", m.r[4 * e + k], s[k].to_bits()));
                }
            }
        }
        b.vertices += 1;
    }
    // *** P3b4c: el mismo lote POR LA PUERTA. La receta que manda la app,
    // leida y pegada como la pega el kernel, sube a la 3060 los MISMOS dos
    // programas (byte a byte) que `pso::traducir` -- los del metal.
    use bmo_gpu_ga10x::receta;
    let blanco = bmo_proton_x_sm86::puerta::Blanco { va: 0x7000_0000, ancho: 1280, alto: 720, bgra: true, cadena: true };
    b.z_limpias += l.limpiar_z.is_some() as usize;
    b.rt_limpios += l.limpiar_rt.is_some() as usize;
    // La puerta de la APP (la de siempre) SI manda un lote con Z: el kernel
    // lo dibuja en su SOMBRA en bloque y la copia al back buffer (P3b4c.6b;
    // sin ella fue Xid 69 en el metal, 28-09). Apagada, lo niega y dice por que.
    if l.reglas.profundidad.is_some() {
        b.z_mandadas += b.puerta.z_a_la_3060 as usize;
        let mut sin_sombra = bmo_proton_x_sm86::puerta::Puerta::nueva();
        sin_sombra.z_a_la_3060 = false;
        match sin_sombra.preparar(l, blanco) {
            Err(e) if e.contains("Z sobre un color PITCH") && e.contains("Xid 69") => {}
            otro => b.fallos.push(format!("sin la sombra, un lote con Z iria a un color pitch: {otro:?}")),
        }
    }
    match b.puerta.preparar(l, blanco) {
        Ok(n) => {
            let r = receta::leer(&b.puerta.caja[..n]).expect("la receta se relee");
            let mut taller = Box::new(receta::Taller::nuevo());
            match receta::pegar(&r, &mut taller) {
                Ok(()) => {
                    if (&taller.vs[..taller.bytes_vs], &taller.ps[..taller.bytes_ps]) != (&t.vs[..], &t.ps[..]) {
                        b.fallos.push(String::from("la puerta pega OTROS programas que pso::traducir"));
                    }
                    if r.dibujo.color != l.limpiar_rt {
                        b.fallos.push(format!("la limpieza del color no viaja en la receta: {:?}", r.dibujo.color));
                    }
                    if r.dibujo.z.is_none() || r.dibujo.descarte != bmo_gpu_ga10x::tuberia::Descarte::Traseras {
                        b.fallos.push(format!("el dibujo de la receta no es el de BMOX-12: {:?}", r.dibujo));
                    }
                    b.recetas += 1;
                }
                Err(e) => b.fallos.push(format!("el kernel no la pegaria: {e:?}")),
            }
            b.puerta.despues(l, true);
        }
        Err(e) => b.fallos.push(format!("la puerta dice que no: {e}")),
    }
}

const BMOX12: &[u8] = include_bytes!("../../proton-x/prueba/bmox12.exe");

/// Prepara `window/sombras` con los .cso del repo (o sin ellos) y corre
/// `bmox12.exe` con esos argumentos y ese guion.
fn correr_bmox12(uno: &MutexGuard<'static, ()>, con_cso: bool, resto: &'static str, guion: &[u64]) -> (u32, String) {
    let dir = volumen().join("window/sombras");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if con_cso {
        for h in ["d7e2992c", "b50c1000"] {
            std::fs::copy(format!("../proton-x/prueba/sombras/{h}.cso"), dir.join(format!("{h}.cso"))).unwrap();
        }
    }
    *NOMBRE.lock().unwrap() = ("window/bmox12.exe", resto);
    let (salio, dicho, _) = correr_exe(uno, BMOX12, true, guion);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    (salio, String::from_utf8_lossy(&dicho).into_owned())
}

/// *** P3c en el anfitrion: `bmox12.exe`, el BMOX-12 de EPICX (estudio_d3d12)
/// compilado por el propietario en SU Windows (rustc 1.97.1, la `std` de Rust
/// y el CRT de MSVC), SIN TOCAR. Sin los .cso de su HLSL (va con CRLF: huella
/// d7e2992c) arranca entero -- el CRT, la fabrica 6, el adaptador, la
/// cadena, la profundidad y sus cifras de memoria -- hasta D3DCompile: la casa
/// deja el pedido, D3DCompile da E_FAIL y el programa sale con su error (1).
#[test]
fn bmox12_exe_sin_sus_cso_arranca_y_deja_el_pedido() {
    let uno = uno_a_la_vez();
    let esc = 1 << 8 | 1 << 9 | 0x01; // la tecla ESC (scancode 1): WM_KEYDOWN VK_ESCAPE
    let (salio, texto) = correr_bmox12(&uno, false, "", &[0, 0, 0, esc]);
    assert!(texto.starts_with("[Estudio D3D12] GPU: NVIDIA GeForce RTX 3060\n"), "{texto}");
    assert!(texto.contains("[Estudio D3D12] tearing (VSync apagado de verdad en flip model): no\n"), "{texto}");
    assert!(texto.contains("[memoria] depth D32 (DEFAULT)          pedidos   3686400 B"), "{texto}");
    assert!(texto.contains("D3DCompile(VSMain, vs_5_0) sin compilar todavia: dejado en window/sombras/d7e2992c.hls"), "{texto}");
    assert!(texto.contains("HRESULT(0x80004005)"), "el error de D3DCompile, como lo imprime el crate windows: {texto}");
    assert_eq!(salio, 1, "main devuelve Err: sale con 1, sin romperse");
    let dir = volumen().join("window/sombras");
    assert_eq!(std::fs::read(dir.join("d7e2992c.ent")).unwrap(), std::fs::read("../proton-x/prueba/sombras/d7e2992c.ent").unwrap());
    assert_eq!(std::fs::read(dir.join("d7e2992c.hls")).unwrap(), std::fs::read("../proton-x/prueba/sombras/d7e2992c.hls").unwrap());
}

/// *** P3c: BMOX-12 SIN TOCAR, CON sus .cso (sombras.exe en el Windows del
/// propietario), DIBUJA EN LA CASA LO QUE DIBUJO LA 3060. En interactivo
/// (VSync, un paso por fotograma, ESC sale) se le dejan 61 fotogramas: los
/// Present 0, 30 y 60 tienen las huellas de la 3060, sin un aviso.
#[test]
fn bmox12_exe_con_sus_cso_dibuja_lo_de_la_3060() {
    let uno = uno_a_la_vez();
    let esc = 1 << 8 | 1 << 9 | 0x01; // la tecla ESC (scancode 1): WM_KEYDOWN VK_ESCAPE
    let mut guion = vec![0u64; 70]; // de sobra: cada PeekMessage vacio gasta uno
    guion.push(esc);
    let (salio, texto) = correr_bmox12(&uno, true, "", &guion);
    assert!(!texto.contains("PROTON-X:"), "ni un aviso de la casa: {texto}");
    assert!(texto.ends_with("[Estudio D3D12] VSync s\u{ed}. ESC para salir.\n"), "{texto}");
    assert_eq!(salio, 0, "ESC: main devuelve Ok");
    let vistas = VISTAS.lock().unwrap().clone();
    assert!(vistas.len() >= 61, "{} Present", vistas.len());
    // EL REGISTRO: una linea por segundo, con los fps y los tiempos (en el
    // banco, en debug, unos pocos fps: lo que se comprueba es que ESTA).
    let reg = REGISTRO.lock().unwrap().clone();
    assert!(!reg.is_empty(), "sin registro en {} fotogramas", vistas.len());
    for l in &reg {
        assert!(l.contains(" fps  fotograma ") && l.contains(" ms  dibujar ") && l.contains(" ms  presentar ") && l.ends_with(")\n"), "{l}");
    }
    for (f, esperada) in bmo_cubo::referencia::HUELLAS {
        assert_eq!(vistas[f as usize], esperada, "fotograma {f}");
    }
}

/// *** P3b4a: BMOX-12 SIN TOCAR y su PSO de verdad (el input layout, el
/// paso y el cbuffer que pone EL JUEGO) traducidos para la 3060 UNA vez --
/// emisor, pegamento y juez: PERFECTO --, y en cada lote de 31 fotogramas el
/// cuerpo de su de vertice, con los registros que el pegamento cargaria de
/// los DATOS, da los bits de la casa en cada vertice. La CPU solo copia.
#[test]
fn bmox12_exe_su_pso_va_a_la_3060_pagando_una_vez() {
    let uno = uno_a_la_vez();
    let esc = 1 << 8 | 1 << 9 | 0x01;
    let mut guion = vec![0u64; 40];
    guion.push(esc);
    *LA3060.lock().unwrap() = Some(La3060 { almacen: bmo_proton_x_sm86::pso::Almacen::nuevo(), lotes: 0, vertices: 0, datos_max: 0, fallos: Vec::new(), puerta: bmo_proton_x_sm86::puerta::Puerta::nueva(), recetas: 0, z_limpias: 0, rt_limpios: 0, z_mandadas: 0 });
    let (salio, texto) = correr_bmox12(&uno, true, "", &guion);
    let b = LA3060.lock().unwrap().take().unwrap();
    assert_eq!(salio, 0, "{texto}");
    assert!(b.fallos.is_empty(), "{:?}", &b.fallos[..b.fallos.len().min(5)]);
    assert!(b.lotes >= 30, "{} lotes", b.lotes);
    assert_eq!(b.almacen.traducciones, 1, "UNA traduccion para todos los fotogramas");
    assert_eq!(b.vertices, 24 * b.lotes, "los 24 vertices del cubo en cada lote");
    assert_eq!(b.recetas, b.lotes, "cada lote, por la puerta, con los mismos programas");
    assert_eq!(b.z_limpias, b.lotes, "BMOX-12 limpia la Z antes de cada fotograma: la 3060 la limpia con el");
    assert_eq!(b.rt_limpios, b.lotes, "y el back buffer: lo limpia la 3060, no la CPU");
    assert_eq!(b.z_mandadas, b.lotes, "la puerta de la app manda la Z: el kernel la dibuja en su sombra");
    eprintln!("bmox12 para la 3060: {} lotes, {} vertices comprobados, DATOS de {} B", b.lotes, b.vertices, b.datos_max);
}

/// *** P3c: `bmox12.exe --fotograma 30`, como se sacaron las huellas de la
/// 3060: la ventana OCULTA, un solo dibujo capturado por READBACK
/// (GetCopyableFootprints, CopyTextureRegion) y guardado como PNG con la
/// `std::fs` de Rust -- CreateFileW, WriteFile -- junto al `.exe`. El PNG
/// guarda la imagen que la 3060 dibujo: su huella, desempaquetado, es la de
/// ese fotograma.
#[test]
fn bmox12_exe_fotograma_30_guarda_el_png_de_la_3060() {
    let uno = uno_a_la_vez();
    let png = volumen().join("window/cubo_f0030.png");
    let _ = std::fs::remove_file(&png);
    let (salio, texto) = correr_bmox12(&uno, true, "--fotograma 30", &[]);
    assert!(!texto.contains("PROTON-X:"), "ni un aviso de la casa: {texto}");
    assert!(texto.ends_with("[Estudio D3D12] fotograma 30 -> cubo_f0030.png\n"), "{texto}");
    assert_eq!(salio, 0);
    let d = std::fs::read(&png).unwrap();
    // Un PNG de 1280x720 RGB (IHDR); sus pixeles, descomprimidos fuera del
    // banco (sin zlib aqui), dan 0x2b3985e93e1a6574: la huella de la 3060.
    assert_eq!(&d[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&d[12..24], b"IHDR\x00\x00\x05\x00\x00\x00\x02\xd0");
    // Y lo que presento (el mismo dibujo) es el fotograma 30 de la 3060.
    let vistas = VISTAS.lock().unwrap().clone();
    assert_eq!(vistas.first().copied(), bmo_cubo::referencia::de_la_3060(30));
}

// Las pruebas del 30-09 en adelante (el diario y las tandas 14 a 16), en su
// fichero: este paso de las 1000 lineas de codigo (L6a).
#[path = "corre/de_hoy.rs"]
mod de_hoy;
#[path = "corre/muestras.rs"]
mod muestras;
// Las tandas 1 a 13 (05-10, L6a otra vez).
#[path = "corre/tandas.rs"]
mod tandas;
