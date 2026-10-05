//! **Las tandas 1 a 13 de Cyberpunk** del banco de `corre.rs`, movidas tal
//! cual el 05-10 (aquel volvio a pasar de las 1000 lineas de codigo: L6a).
//! Usan `tanda` de `corre.rs`, y sus `.exe` siguen alli (los busca el
//! guardian PX3).

use super::*;

/// **La tanda 1 de Cyberpunk en el anfitrion** (29-09): `tanda1.exe` -- el C
/// runtime que piden Cyberpunk2077.exe y sus DLL (cadenas, numeros, mates
/// con el double en xmm0, qsort, printf y scanf _s, FILE y descriptores en
/// modo texto, rutas, la hora, el entorno, un hilo y lo de C++), importado
/// de sus DLL de verdad: los api-ms-win-crt-*, msvcrt.dll y vcruntime140.dll.
#[test]
fn tanda1_exe_tiene_el_c_runtime_de_cyberpunk() {
    tanda_y(TANDA1, None, 55, "tanda1.exe: el C runtime de Cyberpunk es el de Windows", || {
        assert_eq!(std::fs::read(volumen().join("window/tanda1.txt")).unwrap(), b"uno\r\ndos\r\n", "el fichero, en modo texto de Windows");
    });
}

/// **La tanda 2 de Cyberpunk en el anfitrion** (29-09): `tanda2.exe` -- lo
/// que hay debajo de std::mutex, std::condition_variable, std::thread,
/// std::call_once y <chrono>, importado de msvcp140.dll: dos hilos que
/// suman con un mutex, productor y consumidor, y un timedwait que vence.
#[test]
fn tanda2_exe_tiene_los_hilos_de_la_biblioteca_de_cpp() {
    tanda(TANDA2, None, 13, "tanda2.exe: los hilos de la biblioteca de C++ son los de Windows");
}

/// **La tanda 3 de Cyberpunk en el anfitrion** (29-09): `tanda3.exe` --
/// kernel32, pasos 1 a 3: la hora, lo que dice del sistema y las "A"
/// (entorno, rutas, buscar ficheros, modulos, eventos, consola).
#[test]
fn tanda3_exe_tiene_lo_de_kernel32() {
    tanda(TANDA3, Some("window/tanda3.exe"), 25, "tanda3.exe: kernel32 dice lo de Windows");
}

/// **La tanda 3 de Cyberpunk, paso 4a** (29-09): `tanda3b.exe` -- el pool
/// de hilos (trabajos, relojes, esperas, RegisterWaitForSingleObject),
/// InitOnce y las SList; los callbacks en hilos de la casa.
#[test]
fn tanda3b_exe_tiene_el_pool_de_hilos() {
    tanda(TANDA3B, Some("window/tanda3b.exe"), 18, "tanda3b.exe: el pool de hilos dice lo de Windows");
}

/// **La tanda 3 de Cyberpunk, paso 4b** (29-09): `tanda3c.exe` -- el mapeo
/// de ficheros, los puertos de finalizacion, los Open* por nombre,
/// OpenProcess/OpenThread/ReadProcessMemory, WaitForMultipleObjectsEx,
/// GetOverlappedResultEx, CancelIoEx y FormatMessageA.
#[test]
fn tanda3c_exe_tiene_el_mapeo_y_los_puertos() {
    tanda(TANDA3C, Some("window/tanda3c.exe"), 16, "tanda3c.exe: el mapeo y los puertos dicen lo de Windows");
}

/// **La tanda 4 de Cyberpunk** (29-09): `tanda4.exe` -- las excepciones de
/// C++ de MSVC (throw, catch por valor, referencia y puntero, bases con
/// herencia multiple, catch(...), throw; y un throw dentro de un catch).
#[test]
fn tanda4_exe_tiene_las_excepciones_de_cpp() {
    tanda(TANDA4, Some("window/tanda4.exe"), 11, "tanda4.exe: las excepciones de C++ son las de Windows");
}

/// **La tanda 5 de Cyberpunk** (29-09): `tanda5.exe` -- user32, grupo 1:
/// los rectangulos, las medidas, el DPI, los monitores y la geometria de
/// una ventana.
#[test]
fn tanda5_exe_tiene_las_medidas_de_user32() {
    tanda(TANDA5, Some("window/tanda5.exe"), 22, "tanda5.exe: las medidas de user32 dicen lo de Windows");
}

/// **La tanda 6 de Cyberpunk** (29-09): `tanda6.exe` -- user32, grupo 2:
/// las ventanas y sus mensajes (los longs, subclasificar, la cola filtrada,
/// los temporizadores, MsgWaitFor..., los nombres, la posicion, las A).
#[test]
fn tanda6_exe_tiene_las_ventanas_y_sus_mensajes() {
    tanda(TANDA6, Some("window/tanda6.exe"), 29, "tanda6.exe: las ventanas y sus mensajes dicen lo de Windows");
}

/// **La tanda 7 de Cyberpunk** (29-09): `tanda7.exe` -- user32, grupo 3:
/// el teclado, el cursor, la captura, el raw input y el portapapeles.
#[test]
fn tanda7_exe_tiene_el_teclado_el_raton_y_el_portapapeles() {
    tanda(TANDA7, Some("window/tanda7.exe"), 29, "tanda7.exe: el teclado, el raton y el portapapeles dicen lo de Windows");
}

/// **La tanda 8 de Cyberpunk** (30-09): `tanda8.exe` -- el locale de
/// kernel32: comparar y cambiar texto, lo que sabe en-US y los formatos de
/// fecha, hora, numero y moneda (siempre en-US: cualquier Windows lo dice).
#[test]
fn tanda8_exe_tiene_el_locale_de_kernel32() {
    tanda(TANDA8, None, 25, "tanda8.exe: el locale de kernel32 es el de Windows");
}

/// **La tanda 9 de Cyberpunk** (30-09): `tanda9.exe` -- lo que quedaba de
/// kernel32: fibras, SuspendThread, APC, TerminateThread, Toolhelp, psapi,
/// el procesador, la pila, discos y tokens (solo relaciones: cualquier
/// Windows lo dice).
#[test]
fn tanda9_exe_tiene_lo_que_quedaba_de_kernel32() {
    tanda(TANDA9, None, 30, "tanda9.exe: lo que quedaba de kernel32 es lo de Windows");
}

/// **La tanda 10 de Cyberpunk** (30-09): `tanda10.exe` -- lo que quedaba de
/// user32: un dialogo de una plantilla en memoria, el HDC de una ventana,
/// DrawText midiendo, LoadString del STRINGTABLE propio, la estacion de
/// ventanas y los avisos de dispositivos.
#[test]
fn tanda10_exe_tiene_lo_que_quedaba_de_user32() {
    tanda(TANDA10, Some("window/tanda10.exe"), 25, "tanda10.exe: lo que quedaba de user32 es lo de Windows");
}

/// **La tanda 11 de Cyberpunk** (30-09): `tanda11.exe` -- ADVAPI32: el
/// registro, GetUserName, CryptoAPI (MD5, SHA-1 y SHA-256 de verdad), el
/// visor de eventos, ETW y los servicios.
#[test]
fn tanda11_exe_tiene_advapi32() {
    tanda(TANDA11, None, 35, "tanda11.exe: ADVAPI32 es lo de Windows");
}

/// **La tanda 12 de Cyberpunk** (30-09): `tanda12.exe` -- la red y la cripto
/// sin red: ws2_32 POR ORDINAL (lo puro de verdad; lo de red, antes de
/// WSAStartup), crypt32 (Base64, hex, un almacen vacio) y bcrypt (MD5,
/// SHA-1, SHA-256, HMAC y el azar).
#[test]
fn tanda12_exe_tiene_la_red_y_la_cripto_sin_red() {
    tanda(TANDA12, None, 24, "tanda12.exe: la red y la cripto sin red son lo de Windows");
}

/// **La tanda 13 de Cyberpunk** (30-09): `tanda13.exe` -- lo que lanza
/// msvcp140 (las _X..., system_error, future_error), cogido por el nombre de
/// su clase; exception_ptr, uncaught_exceptions y _Lockit.
#[test]
fn tanda13_exe_tiene_lo_que_lanza_msvcp140() {
    tanda(TANDA13, None, 16, "tanda13.exe: lo que lanza msvcp140 es lo de Windows");
}
