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
    // Con el diario, la casa dice la ruta que el .exe no encontro (01-10):
    // diario.exe abre a proposito uno que no esta, y es lo unico que dice.
    let nota = "PROTON-X: CreateFileW(\"no_esta_diario.txt\"): no (error 2)\n";
    assert!(texto.contains(nota), "{texto}");
    let texto = texto.replace(nota, "");
    assert!(!texto.contains("  MAL   ") && !texto.contains("PROTON-X:"), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 4, "{texto}");
    assert!(texto.ends_with("diario.exe: los trampolines no se notan\r\n[salio 0x0]"), "{texto}");
    let texto = std::fs::read_to_string(&ruta).unwrap();
    let funciones: Vec<&str> = texto.lines().filter(|l| !l.starts_with('#') && !l.contains(" <- ")).map(|l| l.rsplit(' ').next().unwrap()).collect();
    assert_eq!(
        funciones,
        ["pow", "GetStdHandle", "WriteFile", "GetModuleHandleW", "GetProcAddress", "GetTickCount", "GetCurrentProcessId", "CreateFileA", "GetLastError", "ExitProcess"],
        "{texto}"
    );
    let numeros: Vec<&str> = texto.lines().filter(|l| !l.starts_with('#') && !l.contains(" <- ")).map(|l| l.split_whitespace().next().unwrap()).collect();
    assert_eq!(numeros, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], "{texto}");
    // EL ANILLO (01-10): al salir, las ultimas llamadas (todas, no solo la
    // primera de cada una), la ultima ExitProcess, cada una con quien la hizo.
    let anillo: Vec<&str> = texto.lines().filter(|l| !l.starts_with('#') && l.contains(" <- ")).collect();
    assert!(anillo.len() > 10, "mas llamadas que funciones distintas: {texto}");
    assert!(anillo.last().unwrap().contains("ExitProcess <- "), "{texto}");
    assert!(anillo.iter().all(|l| l.contains("prueba.exe+0x")), "quien llamo: el .exe, con su RVA: {texto}");
    // EL PULSO (02-10): al salir, tambien la foto -- el reloj, lo de D3D12 y
    // los hilos (el principal, con su ultima llamada); y cada llamada del
    // anillo dice su hilo.
    let foto = &texto[texto.find("# EN VIVO").unwrap_or_else(|| panic!("sin foto: {texto}"))..];
    assert!(foto.contains("ms desde la entrada del .exe") && foto.contains("# d3d12: 0 PSO"), "{foto}");
    let hilo = foto.lines().find(|l| l.contains("[principal]")).unwrap_or_else(|| panic!("{foto}"));
    assert!(hilo.contains("CORRIENDO; ultima: kernel32.dll ExitProcess <- prueba.exe+0x"), "{hilo}");
    let id = hilo.trim_start_matches('#').split_whitespace().next().unwrap();
    assert!(anillo.iter().all(|l| l.split_whitespace().next() == Some(id)), "cada llamada, del hilo {id}: {texto}");
}

/// **Al morir de panico, el diario se queda con todo** (03-10): lo
/// apuntado, la foto y el motivo. En el metal (02-10) PROTON-X murio sin
/// memoria y el diario no tenia ni su final.
#[test]
fn el_diario_al_morir_guarda_lo_que_tenia_y_el_motivo() {
    struct Apagar;
    impl Drop for Apagar {
        fn drop(&mut self) {
            bmo_proton_x_casa::diario::diario(None);
        }
    }
    let uno = uno_a_la_vez();
    let _apagar = Apagar;
    let ruta = volumen().join("morir.txt");
    let _ = std::fs::remove_file(&ruta);
    bmo_proton_x_casa::diario::diario(Some(b"morir.txt"));
    let (salio, _, _) = correr_exe(&uno, DIARIO, true, &[]);
    assert_eq!(salio, 0);
    bmo_proton_x_casa::diario::al_morir(b"PROTON-X: panico en el cargador: memory allocation of 48 bytes failed\n");
    let texto = std::fs::read_to_string(&ruta).unwrap();
    assert!(texto.contains("KERNEL32.dll GetTickCount") || texto.contains(" GetTickCount\n"), "lo apuntado: {texto}");
    assert!(texto.contains("# EN VIVO"), "la foto: {texto}");
    assert!(texto.ends_with("# PANICO: PROTON-X: panico en el cargador: memory allocation of 48 bytes failed\n"), "{texto}");
}

/// **Las trampas con nombre** (03-10, N4.1): GetProcAddress de algo que la
/// casa no tiene en una DLL que si tiene da una TRAMPA, no un NULL; llamarla
/// lo DICE y devuelve 0. En lo del sistema sigue el NULL (preguntar "esta?"
/// es lo normal). Y las dos que Cyberpunk pedia, de verdad.
#[test]
fn getprocaddress_da_trampas_con_nombre_y_no_nulos() {
    use bmo_proton_x::Funcion;
    let _uno = uno_a_la_vez();
    DICHO.lock().unwrap().clear();
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    // Un TEB en el GS, como `correr_exe`: la casa escribe ahi el LastError.
    let bytes = (teb::TEB_BYTES + teb::PEB_BYTES) as u64;
    let mem = mmap(bytes);
    let rsp: u64;
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
    let h = teb::Hilo { teb: mem, peb: mem + teb::TEB_BYTES as u64, pila_tope: rsp + (64 << 10), pila_fondo: rsp - (256 << 10), proceso: 7, hilo: 42, base_imagen: 0 };
    {
        let t = unsafe { core::slice::from_raw_parts_mut(mem as *mut u8, bytes as usize) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
    }
    poner_gs(mem);
    struct Quitar(u64, u64);
    impl Drop for Quitar {
        fn drop(&mut self) {
            poner_gs(0);
            munmap(self.0, self.1);
        }
    }
    let _quitar = Quitar(mem, bytes);
    let f = |d: &str, n: &str| bmo_proton_x_casa::tabla(d, &Funcion::Nombre(n.into())).unwrap();
    type Carga = extern "win64" fn(*const u8) -> u64;
    type Busca = extern "win64" fn(u64, *const u8) -> u64;
    let cargar: Carga = unsafe { core::mem::transmute(f("kernel32.dll", "LoadLibraryA")) };
    let buscar: Busca = unsafe { core::mem::transmute(f("kernel32.dll", "GetProcAddress")) };
    let crypt32 = cargar(b"crypt32.dll\0".as_ptr());
    let iphlpapi = cargar(b"iphlpapi.dll\0".as_ptr());
    let kernel32 = cargar(b"kernel32.dll\0".as_ptr());
    assert!(crypt32 != 0 && iphlpapi != 0 && kernel32 != 0);

    // Las dos de verdad.
    let cerrar = buscar(crypt32, b"CryptMsgClose\0".as_ptr());
    assert_ne!(cerrar, 0);
    let cerrar: extern "win64" fn(u64) -> i32 = unsafe { core::mem::transmute(cerrar) };
    assert_eq!(cerrar(0), 1, "cerrar un NULL es TRUE, como en Windows");
    let indice = buscar(iphlpapi, b"if_nametoindex\0".as_ptr());
    assert_ne!(indice, 0);
    let indice: extern "win64" fn(*const u8) -> u32 = unsafe { core::mem::transmute(indice) };
    assert_eq!(indice(b"eth0\0".as_ptr()), 0);

    // Una que no existe en una DLL de la casa: trampa, la misma dos veces.
    let t = buscar(crypt32, b"CryptNoExisteJamas\0".as_ptr());
    assert_ne!(t, 0, "una trampa, no un NULL");
    assert_eq!(buscar(crypt32, b"CryptNoExisteJamas\0".as_ptr()), t);
    let otra = buscar(crypt32, b"CryptTampocoExiste\0".as_ptr());
    assert!(otra != 0 && otra != t, "cada nombre, la suya");
    let llamar: extern "win64" fn(u64, u64) -> u64 = unsafe { core::mem::transmute(t) };
    assert_eq!(llamar(0, 0), 0);
    assert_eq!(llamar(0, 0), 0);
    let dicho = String::from_utf8(DICHO.lock().unwrap().clone()).unwrap();
    assert!(dicho.contains("GetProcAddress(crypt32.dll, \"CryptNoExisteJamas\"): la casa no la tiene; el .exe recibe una TRAMPA"), "{dicho}");
    assert_eq!(dicho.matches("el .exe LLAMO a crypt32.dll!CryptNoExisteJamas").count(), 1, "se dice UNA vez: {dicho}");
    assert!(!dicho.contains("CryptTampocoExiste, que"), "la que no se llamo no se dice: {dicho}");

    // En lo del sistema, el NULL es la respuesta.
    assert_eq!(buscar(kernel32, b"FuncionDeWindows99\0".as_ptr()), 0);
}

/// **Un hilo que espera DANDO VUELTAS** (02-10): `vueltas.exe` despierta a
/// un trabajador y lo espera mirando QueryPerformanceCounter en un bucle,
/// como un motor. Con hilos cooperativos el trabajador no corria nunca;
/// ahora preguntar la hora cede el turno cada 64 veces (`hilos::sondeo`).
#[test]
fn vueltas_exe_el_que_espera_mirando_el_reloj_deja_correr_a_los_demas() {
    tanda(VUELTAS, None, 4, "vueltas.exe: un hilo que espera dando vueltas deja correr a los demas");
}

/// **El pulso con el diario encendido** (02-10): `crt.exe` (mas de dos mil
/// llamadas: el latido de cada 1024 corre dos veces dentro del trampolin) y
/// `hilos.exe` (el planificador sin nadie listo) dicen LO MISMO que sin el
/// diario, y la foto de la salida cuenta las llamadas y los hilos.
#[test]
fn el_pulso_no_se_nota_y_su_foto_dice_hilos_y_llamadas() {
    struct Apagar;
    impl Drop for Apagar {
        fn drop(&mut self) {
            bmo_proton_x_casa::diario::diario(None);
            *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
        }
    }
    let uno = uno_a_la_vez();
    let _apagar = Apagar;
    // 07-10: el monton de la casa, en cada foto (un medidor de mentira).
    bmo_proton_x_casa::pulso::medir_monton(|| (3 << 20, 5 << 20, 64 << 20));
    let ruta = volumen().join("pulso.txt");
    for (exe, nombre, bien, fin) in [(CRT, ("window/crt.exe", "-nivel 3"), 0, "crt.exe"), (HILOS, ("window/prueba.exe", ""), 19, "hilos.exe: los hilos son los de Windows")] {
        let _ = std::fs::remove_file(&ruta);
        bmo_proton_x_casa::diario::diario(Some(b"pulso.txt"));
        *NOMBRE.lock().unwrap() = nombre;
        let (salio, dicho, _) = correr_exe(&uno, exe, true, &[]);
        let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
        assert!(!texto.contains("MAL") && salio == 0, "{texto}");
        if bien > 0 {
            assert_eq!(texto.matches("  bien  ").count(), bien, "{texto}");
        }
        assert!(texto.contains(fin), "{texto}");
        let diario = std::fs::read_to_string(&ruta).unwrap();
        let foto = &diario[diario.find("# EN VIVO").unwrap_or_else(|| panic!("sin foto: {diario}"))..];
        let llamadas: u64 = foto.split("; ").nth(1).and_then(|x| x.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("{foto}"));
        let hilos = foto.lines().filter(|l| l.starts_with("#   ") && l.contains("; ultima: ")).count();
        assert!(foto.contains("# el monton de la casa: 3 MiB en uso (pico 5 MiB), crecio 64 MiB por la reserva\n"), "{foto}");
        // V5 (07-10): y el codigo nativo (ni crt.exe ni hilos.exe crean PSO).
        assert!(foto.contains("# el codigo nativo: 0 sello(s), 0 KiB de codigo, 0 PSO traducidos\n"), "{foto}");
        assert!(foto.contains("# el turno prestado al sonido: "), "{foto}");
        if exe == CRT {
            assert!(llamadas > 2048, "{foto}");
        } else {
            assert!(hilos > 1, "hilos.exe crea hilos: {foto}");
        }
    }
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
    // La vuelta, aunque no corra un .exe: `tabla` ESCRIBE en la casa (la
    // imagen de cada DLL, el diario). Sin ella, este `push` y el del .exe de
    // la prueba de al lado realojaban el mismo Vec a la vez: el SIGSEGV de
    // una vuelta de cada muchas del banco (03-10, "double free in tcache").
    let _uno = uno_a_la_vez();
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

/// **La tanda 27 de Cyberpunk** (30-09): `tanda27.exe` --
/// PEB->ProcessParameters y su Flags, desde el principal y desde un hilo. El
/// primer hilo del juego moria en la UCRT leyendo ese puntero a cero.
#[test]
fn tanda27_exe_los_parametros_del_proceso() {
    tanda(TANDA27, None, 8, "tanda27.exe: PEB->ProcessParameters es el de Windows");
}

/// **La tanda 28 de Cyberpunk** (30-09): `tanda28.exe` -- RtlUnwindEx con
/// STATUS_UNWIND_CONSOLIDATE (el catch de C++ del CRT enlazado DENTRO de
/// REDGalaxy64.dll). La casa volvia a TargetIp con rax = 0.
#[test]
fn tanda28_exe_desenrollar_consolidando() {
    tanda(TANDA28, None, 6, "tanda28.exe: RtlUnwindEx CONSOLIDA como Windows (el catch de C++)");
}

/// **La tanda 29** (01-10): `tanda29.exe` -- CreateDirectoryW,
/// RemoveDirectoryW, DeleteFileW y MoveFileExW de verdad (relevo de PROTON-X,
/// paso 4b: el perfil de un juego en ESTRATOS). El banco lo hace con su
/// volumen; BMO-X, con ESTRATOS.
#[test]
fn tanda29_exe_carpetas_que_se_crean_mueven_y_borran() {
    tanda(TANDA29, None, 18, "tanda29.exe: carpetas y ficheros que se crean, se mueven y se borran como en Windows");
}

/// **La tanda 30** (01-10): `tanda30.exe` -- Winsock ARRANCA sin cable
/// (Galaxy lo pide en su Init): WSAStartup 2.2 da 0, getaddrinfo y
/// GetAddrInfoW de una IPv4 en numeros, el servicio que no existe y
/// WSACleanup que cuenta.
#[test]
fn tanda30_exe_winsock_arranca_sin_cable() {
    tanda(TANDA30, None, 14, "tanda30.exe: Winsock arranca sin cable y contesta una IPv4 en numeros");
}

/// **La tanda 31** (01-10): `tanda31.exe` -- la red local sin cable: el par
/// TCP de Galaxy en 127.0.0.1 (bind, getsockname, listen, connect, accept,
/// send y recv de un byte), FIONREAD, recv no bloqueante, select, el cierre
/// del otro, el puerto sin nadie y el UDP del logger con un datagrama local.
#[test]
fn tanda31_exe_la_red_local_sin_cable() {
    tanda(TANDA31, None, 30, "tanda31.exe: la red local de Galaxy, sin cable, como en Windows");
}

/// **La tanda 32** (01-10): `tanda32.exe` -- las rutas de Boost.Filesystem
/// (Galaxy): WideCharToMultiByte con WC_NO_BEST_FIT_CHARS y
/// MultiByteToWideChar con MB_PRECOMPOSED en la pagina del sistema; con
/// CP_UTF8 explicito, los mismos rechazos que Windows.
#[test]
fn tanda32_exe_las_rutas_de_boost() {
    tanda(TANDA32, None, 10, "tanda32.exe: las rutas de Boost.Filesystem, de ancho a estrecho y vuelta");
}

/// **La tanda 33** (01-10): `tanda33.exe` -- Galaxy arranca libcurl, que carga secur32 por la
/// ruta del sistema y exige la tabla de InitSecurityInterfaceW.
#[test]
fn tanda33_exe_secur32_y_la_tabla_sspi() {
    tanda(TANDA33, None, 8, "tanda33.exe: secur32 por su ruta y la tabla SSPI que pide curl");
}

/// **La tanda 34** (01-10): `tanda34.exe` -- Streamline (sl.interposer.dll)
/// lee la cabecera de dxgi.dll por su HMODULE: los modulos de la casa son
/// imagenes PE de verdad, con sus exportaciones recorribles.
#[test]
fn tanda34_exe_un_hmodule_se_puede_leer() {
    tanda(TANDA34, None, 9, "tanda34.exe: un HMODULE es una imagen PE que se puede leer");
}

/// **La tanda 35** (01-10): `tanda35.exe` -- Cyberpunk pide GetDesc (el
/// hueco 8) del adaptador: dice lo mismo que GetDesc1.
#[test]
fn tanda35_exe_getdesc_del_adaptador() {
    tanda(TANDA35, None, 8, "tanda35.exe: IDXGIAdapter::GetDesc, lo mismo que GetDesc1");
}

/// **La tanda 36** (01-10): `tanda36.exe` -- el aviso de Cyberpunk salio en
/// arabe: el STRINGTABLE se elige por idioma (el ingles), no el primero.
#[test]
fn tanda36_exe_el_idioma_de_un_recurso() {
    tanda(TANDA36, None, 6, "tanda36.exe: LoadStringW elige el ingles, no el primer idioma");
}

/// **La tanda 37** (01-10): `tanda37.exe` -- Cyberpunk mira su
/// `final.redscripts` solo con GetFileAttributesExW: las fechas de un fichero
/// no son de 1601 y son las mismas por los cuatro caminos de Windows.
#[test]
fn tanda37_exe_las_fechas_de_un_fichero() {
    // Se mira a si mismo: tiene que estar en el volumen, como en Windows.
    std::fs::write(volumen().join("window/tanda37.exe"), TANDA37).unwrap();
    tanda(TANDA37, Some("window/tanda37.exe"), 9, "tanda37.exe: las fechas de un fichero, las mismas por los cuatro caminos");
}

/// **La tanda 38** (01-10): `tanda38.exe` -- con el dispositivo hecho,
/// Cyberpunk pregunta al adaptador por sus monitores (EnumOutputs) y la casa
/// no lo tenia: salio con 0xC0DE0C07.
#[test]
fn tanda38_exe_el_monitor_del_adaptador() {
    tanda(TANDA38, None, 13, "tanda38.exe: IDXGIAdapter::EnumOutputs, el monitor");
}

/// **La tanda 39** (02-10): `tanda39.exe` -- Cyberpunk pregunta a
/// VirtualQuery por una direccion que no es de VirtualAlloc; la casa decia 0
/// y el juego leia `tabla[-1]`.
#[test]
fn tanda39_exe_virtualquery_de_toda_direccion() {
    tanda(TANDA39, None, 8, "tanda39.exe: VirtualQuery de toda direccion");
}

/// **La tanda 41** (02-10): `tanda41.exe` -- Cyberpunk monta una cola por
/// nucleo sin el principal y vacia la ultima con `cuantas - 1`; con UN
/// procesador le salian cero y leia `tabla[-1]`.
#[test]
fn tanda41_exe_cuantos_procesadores() {
    tanda(TANDA41, None, 10, "tanda41.exe: cuantos procesadores, igual por todas partes");
}

/// **La tanda 42** (02-10): `tanda42.exe` -- Cyberpunk pregunta
/// CheckFeatureSupport (niveles, opciones) y pide IDXGIAdapter2; la casa
/// decia que no y era un adaptador de software sin memoria.
#[test]
fn tanda42_exe_lo_que_la_tarjeta_dice_de_si() {
    tanda(TANDA42, None, 13, "tanda42.exe: lo que la tarjeta dice de si");
}

/// **La tanda 43** (02-10): `tanda43.exe` -- Cyberpunk pide
/// ID3D12Device::CreateHeap (hueco 28) y la casa no lo tenia; luego coloca
/// recursos en el monton y pregunta OPTIONS2 a OPTIONS7.
#[test]
fn tanda43_exe_montones_de_memoria() {
    tanda(TANDA43, None, 12, "tanda43.exe: montones de memoria");
}

/// **La tanda 44** (02-10): `tanda44.exe` -- Cyberpunk pidio un bufer de
/// 192 MiB y el cargador entro en panico (cada bufer era un `Vec` de su
/// monton de 48 MiB). Ahora la memoria es del proceso, y los buferes
/// colocados viven en la de su monton: dos en el mismo sitio se ven.
#[test]
fn tanda44_exe_buferes_grandes_y_montones_de_verdad() {
    tanda(TANDA44, None, 10, "tanda44.exe: buferes grandes y montones de verdad");
}

/// **La tanda 45** (02-10): `tanda45.exe` -- Cyberpunk pide ID3D12Device1,
/// 4, 8 y 10 y la casa decia que no. Ahora es el mismo objeto con los 79
/// huecos, y lo de cada version que un motor sin rayos ni malla usa;
/// CreatePipelineState compila el HLSL de cubo12 (sus .cso, como cubo12.exe).
/// Y una textura de 16 MiB sale de la ventana de reserva (tanda 45: las
/// texturas ya no son del monton del cargador).
#[test]
fn tanda45_exe_id3d12device1_a_10() {
    let uno = uno_a_la_vez();
    let dir = volumen().join("window/sombras");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for h in ["f3ef42a0", "4d67f5e4"] {
        std::fs::copy(format!("../proton-x/prueba/sombras/{h}.cso"), dir.join(format!("{h}.cso"))).unwrap();
    }
    let h0 = HECHOS.load(Ordering::SeqCst);
    let (salio, dicho, _) = correr_exe(&uno, TANDA45, true, &[]);
    let hechos = HECHOS.load(Ordering::SeqCst) - h0;
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.contains("PROTON-X:"), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 15, "{texto}");
    assert!(texto.ends_with("tanda45.exe: ID3D12Device1 a 10\r\n[salio 0x0]"), "{texto}");
    // La textura de 16 MiB son paginas de la ventana de reserva, no del
    // monton del cargador (que en el banco seria el asignador de std).
    assert!(hechos >= 16 << 20, "la ventana hizo {hechos} bytes");
}

/// **A la carta, en el banco** (01-10): con umbral CERO, todo fichero que se
/// abre solo para leer va por trozos -- las tandas que leen ficheros prueban
/// asi el camino que en BMO-X lleva los ficheros grandes.
pub(crate) const TROZOS_DEL_BANCO: bmo_proton_x_casa::Trozos = bmo_proton_x_casa::Trozos {
    medida: |r| {
        let m = std::fs::metadata(super::volumen().join(std::str::from_utf8(r).ok()?)).ok()?;
        m.is_file().then(|| m.len())
    },
    leer: |r, desde, dst| {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = std::fs::File::open(super::volumen().join(std::str::from_utf8(r).ok()?)).ok()?;
        f.seek(SeekFrom::Start(desde)).ok()?;
        let mut n = 0;
        while n < dst.len() {
            match f.read(&mut dst[n..]).ok()? {
                0 => break,
                k => n += k,
            }
        }
        Some(n)
    },
    umbral: 0,
};

/// **La tanda 46** (02-10): `tanda46.exe` -- Cyberpunk, tras dos
/// VirtualAlloc, pide OpenExistingHeapFromAddress (hueco 48) y salia con
/// 0xC0DE0030. El monton ES la memoria del VirtualAlloc.
#[test]
fn tanda46_exe_un_monton_sobre_memoria_del_exe() {
    tanda(TANDA46, None, 9, "tanda46.exe: un monton sobre memoria del .exe");
}

/// **La tanda 47** (02-10): `tanda47.exe` -- el inventario contra
/// vkd3d-proton: nombres y datos privados, GetDevice/GetParent, el resto de
/// la fabrica y el adaptador, vistas CBV, consultas y copias de buferes.
#[test]
fn tanda47_exe_lo_que_vkd3d_proton_tiene() {
    tanda(TANDA47, None, 21, "tanda47.exe: lo que vkd3d-proton tiene y la casa no tenia");
}

/// **La tanda 48** (02-10): `tanda48.exe` -- la base de PROTON-X, llena:
/// cada hueco de las 17 interfaces es algo que la casa hace o una falla
/// documentada. Las medidas de GetResourceAllocationInfo y lo que cabe en un
/// monton (el inventario de Cyberpunk en vkd3d-proton), la lista 2 y 4
/// (WriteBufferImmediate, BeginRenderPass con CLEAR), Fence1, Resource2 y
/// Factory7. Dice UN aviso, el de la falla documentada: en el banco eso es
/// lo que se espera, no un fallo.
#[test]
fn tanda48_exe_la_base_de_proton_x_llena() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, TANDA48, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    let avisos: Vec<&str> = texto.lines().filter(|l| l.contains("PROTON-X:")).collect();
    assert_eq!(avisos.len(), 1, "un aviso, el de la falla documentada: {texto}");
    assert!(avisos[0].starts_with("PROTON-X: ID3D12Device::OpenExistingHeapFromFileMapping (hueco 49): falla documentada, HRESULT 0x80004001 -- "), "{texto}");
    assert_eq!(texto.matches("  bien  ").count(), 14, "{texto}");
    assert!(texto.ends_with("tanda48.exe: la base de PROTON-X, llena\r\n[salio 0x0]"), "{texto}");
}

/// **Una falla documentada dice su NOMBRE, devuelve el error de Windows y NO
/// sale** (tanda 48). Hasta la tanda 47 aqui se saltaba a un hueco que la
/// casa no tenia y el proceso salia con `0xC0DE....`; desde la 48 no queda
/// ninguno en las 17 interfaces (el inventario contra vkd3d-proton), y los que
/// Windows deja fallar son fallas documentadas (`fallas.rs`). Se salta al
/// hueco 49 del dispositivo, `OpenExistingHeapFromFileMapping`: Cyberpunk lo
/// llama sin mirar `D3D12_FEATURE_EXISTING_HEAPS`, y tiene que poder seguir.
#[test]
fn una_falla_documentada_dice_cual_es_y_no_sale() {
    let _uno = uno_a_la_vez();
    DICHO.lock().unwrap().clear();
    // SAFETY: nada corre; se pone la plataforma de mentira.
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    let dir = bmo_proton_x_casa::tabla("d3d12.dll", &Funcion::Nombre("D3D12CreateDevice".into())).unwrap();
    let crear: extern "win64" fn(u64, u32, *const [u8; 16], *mut u64) -> i32 = unsafe { core::mem::transmute(dir as usize) };
    let mut disp = 0u64;
    assert_eq!(crear(0, 0xb000, &bmo_proton_x_casa::com::IID_DEVICE, &mut disp), 0);
    // SAFETY: `disp` es un objeto de la casa: su primer puntero es la vtabla.
    let hueco49 = unsafe { (*(disp as *const *const u64)).add(49).read() };
    let abrir: extern "win64" fn(u64, u64, *const [u8; 16], *mut u64) -> i32 = unsafe { core::mem::transmute(hueco49 as usize) };
    let mut monton = 0xDEAD_BEEFu64;
    assert_eq!(abrir(disp, 0, &bmo_proton_x_casa::com::IID_MEMORIA, &mut monton) as u32, 0x8000_4001, "E_NOTIMPL, como vkd3d-proton");
    assert_eq!(monton, 0, "el puntero de salida, a NULL");
    let dicho = String::from_utf8_lossy(&DICHO.lock().unwrap()).into_owned();
    assert!(dicho.starts_with("PROTON-X: ID3D12Device::OpenExistingHeapFromFileMapping (hueco 49): falla documentada, HRESULT 0x80004001 -- "), "{dicho}");
}
