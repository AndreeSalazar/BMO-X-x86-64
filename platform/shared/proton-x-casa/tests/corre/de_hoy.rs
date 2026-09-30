//! **Las pruebas del 30-09 en adelante**, del banco de `corre.rs` (que paso
//! de las 1000 lineas de codigo): el diario y las tandas 14 a 16 de
//! Cyberpunk. Usan los mismos ayudantes (`tanda`, `correr_exe`...) y sus `.exe` (los
//! `include_bytes` siguen en `corre.rs`: ahi los busca el guardian PX3).

use super::*;
use std::sync::atomic::AtomicU64;

// -- P0.4c: la RESERVA del banco, como la del kernel -----------------------

/// 8 GiB de direcciones sin RAM (PROT_NONE, MAP_NORESERVE): HACER es
/// `mprotect` a R+W (Linux da ceros al tocar), DESHACER es `madvise
/// (DONTNEED)` y otra vez PROT_NONE (la proxima vez, ceros otra vez).
pub(super) fn reserva_del_banco() -> bmo_proton_x_casa::Reserva {
    static BASE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    const BYTES: u64 = 8 << 30;
    let base = *BASE.get_or_init(|| {
        // MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, PROT_NONE, alineada a 64 KiB.
        let r = unsafe { syscall6(9, 0, BYTES + (1 << 16), 0, 0x4022, u64::MAX, 0) };
        assert!(r < (-4096i64) as u64, "mmap de la reserva dijo {r:#x}");
        (r + 0xFFFF) & !0xFFFF
    });
    fn hacer(va: u64, n: u64) -> bool {
        mprotect(va, n, PROT_LEE | PROT_ESCRIBE);
        HECHOS.fetch_add(n, Ordering::SeqCst);
        true
    }
    fn deshacer(va: u64, n: u64) {
        // MADV_DONTNEED: la proxima vez que se haga, a cero.
        unsafe { syscall6(28, va, n, 4, 0, 0, 0) };
        mprotect(va, n, 0);
        DESHECHOS.fetch_add(n, Ordering::SeqCst);
    }
    fn ram() -> (u64, u64) {
        (16 << 30, 12 << 30)
    }
    bmo_proton_x_casa::Reserva { base, bytes: BYTES, hacer, deshacer, ram }
}

/// Lo que el banco hizo y deshizo de la reserva (para las pruebas).
pub(super) static HECHOS: AtomicU64 = AtomicU64::new(0);
pub(super) static DESHECHOS: AtomicU64 = AtomicU64::new(0);


/// **El diario, apagado** (P0.3, 30-09): `diario.exe` dice lo de Windows
/// sin trampolines por medio.
#[test]
fn diario_exe_sin_diario_dice_lo_de_windows() {
    tanda(DIARIO, None, 4, "diario.exe: los trampolines no se notan");
}

/// **El diario, encendido**: los trampolines no se notan (el mismo `bien`:
/// doubles en xmm, argumentos en la pila, GetProcAddress), y el fichero
/// tiene cada funcion UNA vez, en el orden de su primera llamada.
#[test]
fn diario_exe_con_diario_apunta_cada_funcion_una_vez_y_en_orden() {
    /// Apagar el diario pase lo que pase: las demas pruebas no lo quieren.
    struct Apagar;
    impl Drop for Apagar {
        fn drop(&mut self) {
            bmo_proton_x_casa::diario::diario(None);
        }
    }
    // Primero la vuelta (un .exe a la vez) y DESPUES el diario: encendido
    // antes, las pruebas de al lado resolverian con trampolines.
    let uno = uno_a_la_vez();
    let _apagar = Apagar;
    let ruta = volumen().join("diario.txt");
    let _ = std::fs::remove_file(&ruta);
    bmo_proton_x_casa::diario::diario(Some(b"diario.txt"));
    let (salio, dicho, _) = correr_exe(&uno, DIARIO, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   ") && !texto.contains("PROTON-X:"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 4, "{texto}");
    assert!(texto.ends_with("diario.exe: los trampolines no se notan\r\n[salio 0x0]"), "{texto}");
    let texto = std::fs::read_to_string(&ruta).unwrap();
    let funciones: Vec<&str> = texto.lines().filter(|l| !l.starts_with('#')).map(|l| l.rsplit(' ').next().unwrap()).collect();
    assert_eq!(
        funciones,
        ["pow", "GetStdHandle", "WriteFile", "GetModuleHandleW", "GetProcAddress", "GetTickCount", "GetCurrentProcessId", "CreateFileA", "GetLastError", "ExitProcess"],
        "{texto}"
    );
    let numeros: Vec<&str> = texto.lines().filter(|l| !l.starts_with('#')).map(|l| l.split_whitespace().next().unwrap()).collect();
    assert_eq!(numeros, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], "{texto}");
}

/// **La tanda 14a de Cyberpunk** (30-09): `tanda14.exe` -- las DLL chicas
/// del censo (DURAS): winmm, shlwapi, shell32, ntdll, gdi32, powrprof,
/// normaliz, ole32, rpcrt4, el ETW de advapi32 y XInput.
#[test]
fn tanda14_exe_tiene_las_dll_chicas_del_censo() {
    tanda(TANDA14, None, 27, "tanda14.exe: las DLL chicas del censo dicen lo de Windows");
}

/// **`__CxxFrameHandler4`** (30-09): `tanda4m.exe` -- la MISMA tanda4.cpp,
/// compilada con `cl` de MSVC 19.44 en el Windows del propietario: sus
/// tablas son las comprimidas de FH4 (las de Cyberpunk). Dice lo mismo que
/// la de clang (FH3).
#[test]
fn tanda4m_exe_tiene_las_excepciones_de_cpp_de_fh4() {
    tanda(TANDA4M, None, 11, "tanda4.exe: las excepciones de C++ son las de Windows");
}

/// **La tanda 14b de Cyberpunk** (30-09): `tanda14b.exe` -- VERSION (lee
/// su PROPIO recurso de version: el `.exe` tiene que estar en el volumen,
/// con su nombre) y la seguridad de los ficheros.
#[test]
fn tanda14b_exe_tiene_version_y_seguridad() {
    std::fs::write(volumen().join("window").join("tanda14b.exe"), TANDA14B).unwrap();
    tanda(TANDA14B, Some("window/tanda14b.exe"), 12, "tanda14b.exe: VERSION y la seguridad dicen lo de Windows");
}

/// **La tanda 15 de Cyberpunk** (30-09): `tanda15.exe` -- COM lo justo
/// (ole32, y OLEAUT32 por ordinal). Su CoCreateInstance de una clase que no
/// hay se dice por la consola: es el UNICO aviso que se espera.
#[test]
fn tanda15_exe_tiene_com_lo_justo() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, TANDA15, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert_eq!(texto.matches("PROTON-X:").count(), 1, "{texto}");
    assert!(texto.contains("PROTON-X: CoCreateInstance {584F4D42-1515-1515-B00B-5A1E15151515}: la casa no tiene esa clase"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 11, "{texto}");
    assert!(texto.ends_with("tanda15.exe: COM lo justo dice lo de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **La tanda 16 de Cyberpunk** (30-09): `tanda16.exe` -- los aparatos que
/// no hay (HID, SETUPAPI, CFGMGR32) y los 18 ordinales de WLDAP32 del
/// juego, cargados.
#[test]
fn tanda16_exe_tiene_los_aparatos_que_no_hay() {
    tanda(TANDA16, None, 9, "tanda16.exe: los aparatos que no hay dicen lo de Windows");
}

/// **La tanda 17 de Cyberpunk** (30-09): `tanda17.exe` -- el RTTI de C++:
/// dynamic_cast (tambien cruzado y a una referencia), typeid (tambien de un
/// nulo) y __unDName.
#[test]
fn tanda17_exe_tiene_el_rtti_de_cpp() {
    tanda(TANDA17, None, 11, "tanda17.exe: el RTTI de C++ es el de Windows");
}

/// **La tanda 18 de Cyberpunk** (30-09): `tanda18.exe` -- el locale de
/// msvcp140 como lo usa el codigo inline de MSVC: los `id` (DATOS), el
/// locale global, _Locimp, facet, _Locinfo, _Yarn, ctype<char> y los dos
/// codecvt.
#[test]
fn tanda18_exe_tiene_el_locale_de_msvcp() {
    tanda(TANDA18, None, 16, "tanda18.exe: el locale de msvcp140 es el de Windows");
}

/// **La tanda 19 de Cyberpunk** (30-09): `tanda19.exe` -- los flujos de
/// msvcp140: un streambuf propio sobre basic_streambuf, ostream y sus
/// numeros, istream e iostream (con su base virtual), cerr, setw, _Fiopen
/// y time_put (su _Fiopen abre su PROPIO `.exe`: tiene que estar en el
/// volumen, con su nombre).
#[test]
fn tanda19_exe_tiene_los_flujos_de_msvcp() {
    std::fs::write(volumen().join("window").join("tanda19.exe"), TANDA19).unwrap();
    tanda(TANDA19, Some("window/tanda19.exe"), 19, "tanda19.exe: los flujos de msvcp140 son los de Windows");
}

/// **La tanda 19 compilada por MSVC** (30-09): `tanda19m.exe` -- los
/// flujos de msvcp140 con las cabeceras de verdad, como el juego (cl 19.44
/// en el Windows del propietario: 15 de 15). Abre su PROPIO `.exe`.
#[test]
fn tanda19m_exe_de_msvc_tiene_los_flujos() {
    std::fs::write(volumen().join("window").join("tanda19m.exe"), TANDA19M).unwrap();
    tanda(TANDA19M, Some("window/tanda19m.exe"), 15, "tanda19m.exe: los flujos de msvcp140, compilados por MSVC, son los de Windows");
}

/// **La tanda 20 de Cyberpunk** (30-09): `tanda20.exe` -- las ultimas DURAS
/// del censo del metal: << float, << const void*, << long long, read, seekg,
/// tellg, setprecision, _Fiopen ancho (abre su PROPIO `.exe`), el
/// constructor de task_continuation_context y el API set de CFGMGR32.
#[test]
fn tanda20_exe_tiene_las_ultimas_duras() {
    std::fs::write(volumen().join("window").join("tanda20.exe"), TANDA20).unwrap();
    tanda(TANDA20, Some("window/tanda20.exe"), 15, "tanda20.exe: las ultimas DURAS son las de Windows");
}

/// **Los API set "downlevel"** (P0.4b.7, 30-09): `dbghelp.dll` de Cyberpunk
/// importa de `api-ms-win-downlevel-kernel32-l2-1-0.dll`; en el metal
/// faltaron CreateFileMappingA y LocalFree. Son kernel32 con otro traje.
#[test]
fn los_api_set_downlevel_son_su_dll() {
    use bmo_proton_x::Funcion;
    let f = |n: &str| Funcion::Nombre(n.into());
    for n in ["CreateFileMappingA", "LocalFree"] {
        let de = bmo_proton_x_casa::tabla("api-ms-win-downlevel-kernel32-l2-1-0.dll", &f(n));
        assert!(de.is_some(), "{n}: la casa no lo resuelve por el API set downlevel");
        assert_eq!(de, bmo_proton_x_casa::tabla("kernel32.dll", &f(n)), "{n}: el mismo que kernel32");
    }
    assert!(bmo_proton_x_casa::tabla("API-MS-WIN-DOWNLEVEL-ADVAPI32-L1-1-0.dll", &f("RegOpenKeyExW")).is_some());
    assert_eq!(bmo_proton_x_casa::tabla("api-ms-win-downlevel-", &f("LocalFree")), None, "sin DLL detras, nada");
}

/// **La tanda 21 de Cyberpunk** (30-09): `tanda21.exe` -- LoadLibrary de un
/// API set. El CRT del juego, en el metal, pidio
/// `LoadLibraryExW("api-ms-win-core-synch-l1-2-0")` y la casa dijo NULL; en
/// Windows es su anfitrion (kernelbase, ucrtbase) y GetProcAddress busca ahi.
/// Y VirtualProtect sobre la propia imagen, que el juego hizo dos veces.
#[test]
fn tanda21_exe_carga_los_api_set() {
    // Un aviso, y ese: el API set que no existe (la casa lo dice, Windows no).
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, TANDA21, true, &[]);
    let texto = String::from_utf8(dicho).unwrap();
    assert_eq!(salio, 0, "{texto}");
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert_eq!(texto.matches("PROTON-X:").count(), 1, "{texto}");
    assert!(texto.contains("PROTON-X: LoadLibrary(\"api-ms-win-nadie-l1-1-0\"): no es una DLL de la casa"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 17, "{texto}");
    assert!(texto.ends_with("tanda21.exe: LoadLibrary de un API set es lo de Windows\r\n"), "{texto}");
}

/// **La tanda 22 de Cyberpunk** (30-09): `tanda22.exe` y `tanda22d.dll` --
/// el TLS de una DLL. `libxess_fg.dll` del juego tiene `__declspec(thread)`
/// y su DllMain leyo `0+0x8` en el metal: la casa solo daba TLS al `.exe`.
#[test]
fn tanda22_exe_el_tls_de_una_dll() {
    let uno = uno_a_la_vez();
    std::fs::write(volumen().join("window/tanda22d.dll"), TANDA22D).unwrap();
    *NOMBRE.lock().unwrap() = ("window/tanda22.exe", "");
    let (salio, dicho, _) = correr_exe(&uno, TANDA22, true, &[]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 9, "{texto}");
    assert!(texto.ends_with("tanda22.exe: el TLS de una DLL es el de Windows\r\n[salio 0x0]"), "{texto}");
}

/// **La tanda 23 de Cyberpunk** (30-09): `tanda23.exe` -- la memoria EN
/// MARCHA. En el metal el juego pidio con VirtualAlloc mas de 64 MiB de una
/// vez; con la reserva (P0.4c), reservar no gasta y cada COMMIT pide sus
/// paginas. Y el banco ve que las paginas fueron y volvieron.
#[test]
fn tanda23_exe_la_memoria_en_marcha() {
    let (h0, d0) = (HECHOS.load(Ordering::SeqCst), DESHECHOS.load(Ordering::SeqCst));
    tanda(TANDA23, None, 16, "tanda23.exe: la memoria en marcha es la de Windows");
    let (h, d) = (HECHOS.load(Ordering::SeqCst) - h0, DESHECHOS.load(Ordering::SeqCst) - d0);
    assert!(h >= 200 << 20, "se hicieron los 200 MiB por la reserva: {h}");
    assert!(d >= 200 << 20, "y volvieron: {d}");
}

/// **La tanda 24 de Cyberpunk** (30-09): `tanda24.exe` --
/// IsProcessorFeaturePresent contra el CPUID (y XGETBV). El juego lo
/// pregunto justo antes de rendirse y la casa solo decia SSE, SSE2 y NX.
#[test]
fn tanda24_exe_las_capacidades_del_procesador() {
    tanda(TANDA24, None, 12, "tanda24.exe: IsProcessorFeaturePresent es el de Windows");
}

/// **La tanda 25 de Cyberpunk** (30-09): `tanda25.exe` -- SystemFunction036
/// (RtlGenRandom) por su API set y por ADVAPI32. El juego se rendia con
/// abort() en un constructor global: std::random_device -> rand_s -> ella.
#[test]
fn tanda25_exe_el_azar_de_rand_s() {
    tanda(TANDA25, None, 9, "tanda25.exe: SystemFunction036 (el azar de rand_s) es el de Windows");
}

/// **La tanda 26 de Cyberpunk** (30-09): `tanda26.exe` -- GetModuleHandleExW
/// con FROM_ADDRESS. El juego lo pidio dos veces antes de su primer hilo y
/// la casa no sabia que modulo tenia la direccion.
#[test]
fn tanda26_exe_el_modulo_de_una_direccion() {
    tanda(TANDA26, None, 6, "tanda26.exe: GetModuleHandleExW desde una direccion es el de Windows");
}
