//! **Las pruebas del 30-09 en adelante**, del banco de `corre.rs` (que paso
//! de las 1000 lineas de codigo): el diario y las tandas 14 a 16 de
//! Cyberpunk. Usan los mismos ayudantes (`tanda`, `correr_exe`...) y sus `.exe` (los
//! `include_bytes` siguen en `corre.rs`: ahi los busca el guardian PX3).

use super::*;


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
    tanda(TANDA19, Some("window/tanda19.exe"), 16, "tanda19.exe: los flujos de msvcp140 son los de Windows");
}
