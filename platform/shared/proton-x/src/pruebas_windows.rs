//! El banco de lo que un `.exe` de Windows pide a la casa desde P4e (27-09):
//! el monton, las regiones de VirtualAlloc, el proceso, el texto, las
//! carpetas, los mensajes, el `printf` de C. Salio de `pruebas.rs` el 28-09,
//! cuando paso de las 1000 lineas de codigo (L6a).

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::ficheros::NoRuta;
use crate::hilos::{Objeto, Planificador, Turno, WAIT_OBJECT_0};

// -- P4e: el monton, las regiones de VirtualAlloc y el proceso ---------------------

use crate::monton::{self, Bloque, Monton, Palabras};
use crate::proceso::{self, Entorno};
use crate::regiones::{self, Consulta, NoVirtual, Regiones, MEM_COMMIT, MEM_RESERVE};

/// Una memoria de mentira: `base` y palabras de 8 bytes.
struct Mem {
    base: u64,
    w: Vec<u64>,
}
impl Palabras for Mem {
    fn leer(&self, d: u64) -> u64 {
        assert!(d % 8 == 0 && d >= self.base, "lectura torcida o fuera: {d:#x}");
        self.w[((d - self.base) / 8) as usize]
    }
    fn poner(&mut self, d: u64, v: u64) {
        assert!(d % 8 == 0 && d >= self.base, "escritura torcida o fuera: {d:#x}");
        self.w[((d - self.base) / 8) as usize] = v;
    }
}

fn arena(bytes: u64) -> (Mem, Monton) {
    let mut m = Mem { base: 0x10_0000, w: vec![0; (bytes / 8) as usize] };
    let mut h = Monton::nuevo();
    assert!(h.agregar(&mut m, 0x10_0000, bytes));
    h.comprobar(&m).unwrap();
    (m, h)
}

#[test]
fn el_monton_pide_suelta_y_fusiona_hasta_quedar_como_estaba() {
    let (mut m, mut h) = arena(1 << 20);
    let libre = h.bytes_libres();
    let a = h.pedir(&mut m, 100, 16, 1).unwrap();
    let b = h.pedir(&mut m, 0, 16, 1).unwrap();
    let c = h.pedir(&mut m, 5000, 16, 2).unwrap();
    assert!(a % 16 == 0 && b % 16 == 0 && c % 16 == 0);
    assert_eq!(h.bloque(&m, a), Some(Bloque { pedido: 100, propietario: 1 }));
    assert_eq!(h.bloque(&m, b), Some(Bloque { pedido: 0, propietario: 1 }), "0 bytes: un bloque de verdad, HeapSize 0");
    h.comprobar(&m).unwrap();
    // Soltar el de en medio y luego sus vecinos: las tres fusiones.
    assert_eq!(h.soltar(&mut m, b), Some(Bloque { pedido: 0, propietario: 1 }));
    assert_eq!(h.soltar(&mut m, b), None, "el doble HeapFree se ve");
    assert_eq!(h.soltar(&mut m, a + 16), None, "dentro de un bloque: no");
    assert_eq!(h.soltar(&mut m, 0x42), None, "fuera de las arenas: no");
    h.comprobar(&m).unwrap();
    h.soltar(&mut m, a).unwrap();
    h.soltar(&mut m, c).unwrap();
    h.comprobar(&m).unwrap();
    assert_eq!(h.bytes_libres(), libre, "todo suelto: un bloque libre otra vez");
    assert!(h.pedir(&mut m, libre - 16, 16, 1).is_some(), "y cabe entero");
    assert_eq!(h.pedir(&mut m, 16, 16, 1), None, "y ya no cabe nada");
}

#[test]
fn el_monton_aguanta_miles_de_pedidas_y_ni_un_byte_se_pisa() {
    let (mut m, mut h) = arena(8 << 20);
    let libre = h.bytes_libres();
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    let mut azar = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    // (direccion, medida, sello): cada bloque lleva su sello escrito entero.
    let mut vivos: Vec<(u64, u64, u64)> = Vec::new();
    for vuelta in 0..6000 {
        let r = azar();
        if r % 3 != 0 || vivos.is_empty() {
            let tam = 1 + r % 3000;
            let alin = if r % 17 == 0 { 4096 } else { 16 };
            let p = h.pedir(&mut m, tam, alin, 1 + (r % 4) as u16).expect("cabe");
            assert_eq!(p % alin, 0);
            let sello = azar() | 1;
            for k in 0..tam.div_ceil(8) {
                m.poner(p + 8 * k, sello ^ k);
            }
            vivos.push((p, tam, sello));
        } else {
            let (p, tam, sello) = vivos.swap_remove((r as usize / 3) % vivos.len());
            for k in 0..tam.div_ceil(8) {
                assert_eq!(m.leer(p + 8 * k), sello ^ k, "alguien piso un bloque vivo");
            }
            assert_eq!(h.bloque(&m, p).unwrap().pedido, tam);
            h.soltar(&mut m, p).unwrap();
        }
        if vuelta % 500 == 0 {
            h.comprobar(&m).unwrap();
        }
    }
    // Crecer y encoger en su sitio sin perder lo de dentro.
    let (p, tam, sello) = vivos[0];
    if h.cambiar_en_sitio(&mut m, p, tam / 2 + 1) {
        assert_eq!(m.leer(p), sello);
    }
    vivos[0].1 = h.bloque(&m, p).unwrap().pedido;
    h.comprobar(&m).unwrap();
    // HeapDestroy de los del propietario 3, y el resto uno a uno.
    // (Lo ya suelto no se vuelve a mirar: su cabecera vieja puede seguir ahi,
    // dentro de un libre -- un monton de C tampoco lo distingue.)
    let (de_3, resto): (Vec<_>, Vec<_>) = vivos.into_iter().partition(|v| h.bloque(&m, v.0).unwrap().propietario == 3);
    assert_eq!(h.soltar_de(&mut m, 3) as usize, de_3.len());
    h.comprobar(&m).unwrap();
    for (p, _, _) in resto {
        h.soltar(&mut m, p).unwrap();
    }
    h.comprobar(&m).unwrap();
    assert_eq!(h.bytes_libres(), libre, "seis mil vueltas despues, la arena entera otra vez");
}

#[test]
fn el_monton_crece_y_encoge_en_su_sitio() {
    let (mut m, mut h) = arena(1 << 16);
    let a = h.pedir(&mut m, 64, 16, 1).unwrap();
    let b = h.pedir(&mut m, 64, 16, 1).unwrap();
    assert!(!h.cambiar_en_sitio(&mut m, a, 200), "el de delante esta usado: no");
    assert_eq!(h.bloque(&m, a).unwrap().pedido, 64, "y nada tocado");
    assert!(h.cambiar_en_sitio(&mut m, b, 4000), "el de delante esta libre: si");
    assert_eq!(h.bloque(&m, b).unwrap().pedido, 4000);
    assert!(h.cambiar_en_sitio(&mut m, b, 10), "encoger siempre");
    h.comprobar(&m).unwrap();
    assert!(h.cambiar_en_sitio(&mut m, b, 1 << 15));
    assert!(!h.cambiar_en_sitio(&mut m, b, 1 << 17), "mas que la arena: no");
    h.comprobar(&m).unwrap();
    // Una segunda arena, pegada a la primera en las direcciones: son dos.
    assert_eq!(h.arenas().len(), 1);
}

#[test]
fn los_handles_de_monton_no_se_confunden() {
    assert_eq!(monton::propietario_de(monton::asa(1)), Some(1));
    assert_eq!(monton::propietario_de(monton::asa(7)), Some(7));
    assert_eq!(monton::propietario_de(monton::asa(monton::PROPIETARIO_VIRTUAL)), None, "VirtualAlloc no es un monton");
    assert_eq!(monton::propietario_de(0), None);
    assert_eq!(monton::propietario_de(0x5A1D_0001), None, "la consola no es un monton");
}

#[test]
fn las_regiones_de_virtualalloc_cuentan_paginas_como_windows() {
    let base = 0x40_0000;
    let mut r = Regiones::nuevas();
    r.nueva(base, 1 << 20, 4, false);
    assert_eq!(r.consultar(base), Some(Consulta { base, base_region: base, prot_inicial: 4, tam: 1 << 20, estado: MEM_RESERVE, prot: 0 }));
    // Commit de [base+64K+10, +8K): TRES paginas, las que tocan el rango.
    let mut ceros = Vec::new();
    assert_eq!(r.hacer(base + 0x1_0000 + 10, 0x2000, 4, |d, n| ceros.push((d, n))), Ok(base + 0x1_0000));
    assert_eq!(ceros, [(base + 0x1_0000, 0x3000)]);
    let c = r.consultar(base + 0x1_0000 + 77).unwrap();
    assert_eq!((c.base, c.tam, c.estado, c.prot), (base + 0x1_0000, 0x3000, MEM_COMMIT, 4));
    assert_eq!(r.consultar(base).unwrap().tam, 0x1_0000, "delante, 64 KiB reservados");
    // Otra vez, mas ancho: solo lo que no estaba hecho va a cero.
    ceros.clear();
    r.hacer(base + 0xF000, 0x6000, 4, |d, n| ceros.push((d, n))).unwrap();
    assert_eq!(ceros, [(base + 0xF000, 0x1000), (base + 0x1_3000, 0x2000)]);
    assert_eq!(r.proteger(base + 0x1_0000, 1, 2), Ok(4));
    assert_eq!(r.consultar(base + 0x1_0000).unwrap().tam, 0x1000, "la pagina protegida es su propia tirada");
    assert_eq!(r.proteger(base, 0x1000, 4), Err(NoVirtual::Direccion), "sobre una reservada: 487");
    r.deshacer(base + 0x1_0000, 1).unwrap();
    assert_eq!(r.consultar(base + 0x1_0000).unwrap().estado, MEM_RESERVE);
    assert_eq!(r.hacer(base + (1 << 20) - 10, 100, 4, |_, _| {}), Err(NoVirtual::Direccion), "saliendo de la region: 487");
    assert_eq!(r.hacer(0x1000, 100, 4, |_, _| {}), Err(NoVirtual::Direccion));
    assert_eq!(r.soltar(base, 0x1000), Err(NoVirtual::Parametro), "RELEASE con medida: 87");
    assert_eq!(r.soltar(base + 0x1000, 0), Err(NoVirtual::Parametro), "RELEASE fuera de la base: 87");
    assert_eq!(r.soltar(base, 0), Ok(1 << 20));
    assert_eq!(r.consultar(base), None);
    assert_eq!(NoVirtual::Direccion.error(), 487);
    assert_eq!(NoVirtual::SinMemoria.error(), 8);
}

#[test]
fn regiones_piden_y_devuelven_las_paginas() {
    // P0.4c: quien tiene las paginas (el kernel) las da y las recibe por
    // tiradas; si no puede darlas, esa tirada no se marca.
    use crate::regiones::*;
    let mut r = Regiones::nuevas();
    let base = 0x20_0000_0000u64;
    r.nueva(base, 0x10_0000, 0, false);
    let mut dadas = Vec::new();
    r.hacer_con(base + 0x1000, 0x2000, 4, |d, n| {
        dadas.push((d, n));
        true
    })
    .unwrap();
    r.hacer_con(base + 0x5000, 0x1000, 4, |d, n| {
        dadas.push((d, n));
        true
    })
    .unwrap();
    // Del 0 al 0x8000: las dos hechas no se piden otra vez; los huecos si.
    let mut pedidas = Vec::new();
    r.hacer_con(base, 0x8000, 4, |d, n| {
        pedidas.push((d, n));
        true
    })
    .unwrap();
    assert_eq!(pedidas, [(base, 0x1000), (base + 0x3000, 0x2000), (base + 0x6000, 0x2000)]);
    // Sin RAM: la tirada no se marca, y se dice 8.
    assert_eq!(r.hacer_con(base + 0x8000, 0x1000, 4, |_, _| false), Err(NoVirtual::SinMemoria));
    assert_eq!(r.consultar(base + 0x8000).unwrap().estado, MEM_RESERVE, "la que no se dio sigue solo reservada");
    // Deshacer devuelve solo lo que estaba hecho, por tiradas.
    r.deshacer(base + 0x2000, 0x1000).unwrap();
    let mut devueltas = Vec::new();
    r.deshacer_con(base, 0, |d, n| devueltas.push((d, n))).unwrap();
    assert_eq!(devueltas, [(base, 0x2000), (base + 0x3000, 0x5000)]);
    assert_eq!(r.consultar(base).unwrap().tam, 0x10_0000, "toda la region, solo reservada");
    assert_eq!(regiones::paginas(0x1FFF, 2), (0x1000, 0x3000));
}

#[test]
fn la_linea_de_ordenes_y_el_nombre_del_exe() {
    let exe = proceso::ruta_windows("apps/juego.exe");
    assert_eq!(exe, "C:\\apps\\juego.exe");
    let l = proceso::linea(&exe, "  -nivel 3 ");
    assert_eq!(l, "\"C:\\apps\\juego.exe\" -nivel 3");
    assert_eq!(proceso::argumentos(&l), ["C:\\apps\\juego.exe", "-nivel", "3"]);
    assert_eq!(proceso::linea(&exe, ""), "\"C:\\apps\\juego.exe\"");
    // Las reglas del CRT de Microsoft.
    assert_eq!(proceso::argumentos(r#"a.exe "dos palabras" b\\"c" d\"e f\\g "h""i""#), ["a.exe", "dos palabras", "b\\c", "d\"e", "f\\\\g", "h\"i"]);
    assert_eq!(proceso::argumentos(r#""C:\a b\x.exe"z y"#), ["C:\\a b\\x.exe", "z", "y"], "el programa: hasta la comilla, sin escapes");
}

#[test]
fn el_entorno_es_el_de_windows() {
    let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    let mut e = Entorno::de_bmo("C:\\apps");
    assert_eq!(e.leer(&w("path")), Some(&w("C:\\apps")[..]), "sin mayusculas que cuenten");
    assert_eq!(e.leer(&w("OS")), Some(&w("Windows_NT")[..]));
    assert_eq!(e.leer(&w("SystemRoot")), None, "no hay un Windows debajo");
    e.poner(&w("PxPrueba"), Some(&w("hola"))).unwrap();
    e.poner(&w("PXPRUEBA"), Some(&w("adios"))).unwrap();
    assert_eq!(e.leer(&w("pxprueba")), Some(&w("adios")[..]));
    let b = String::from_utf16(&e.bloque()).unwrap();
    assert!(b.contains("\0PxPrueba=adios\0"), "el nombre, como se escribio la primera vez: {b:?}");
    assert!(b.ends_with("\0\0"));
    let nombres: Vec<String> = b.trim_end_matches('\0').split('\0').map(|x| x.split('=').next().unwrap().to_uppercase()).collect();
    let mut ordenados = nombres.clone();
    ordenados.sort();
    assert_eq!(nombres, ordenados, "el bloque va ordenado");
    assert!(e.poner(&w("A=B"), Some(&w("x"))).is_err());
    assert!(e.poner(&w(""), Some(&w("x"))).is_err());
    e.poner(&w("pxprueba"), None).unwrap();
    assert_eq!(e.leer(&w("PXPRUEBA")), None);
    assert_eq!(Entorno::vacio().bloque(), [0, 0]);
}

// -- P4f: el texto ------------------------------------------------------------------

use crate::texto::{self, MalFormado};

#[test]
fn el_texto_pasa_entre_utf8_y_utf16_como_windows() {
    let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    assert_eq!(texto::a_ancho("a\u{F1}\u{20AC}\u{1F600}".as_bytes(), true), Ok(w("a\u{F1}\u{20AC}\u{1F600}")));
    assert_eq!(texto::a_ancho(b"a\xFFb", false), Ok(vec![0x61, 0xFFFD, 0x62]), "un byte malo: un U+FFFD");
    assert_eq!(texto::a_ancho(b"a\xFFb", true), Err(MalFormado), "MB_ERR_INVALID_CHARS: NO");
    assert_eq!(texto::a_ancho(b"\xED\xA0\x80", true), Err(MalFormado), "un sustituto en UTF-8 no vale");
    assert_eq!(texto::a_estrecho(&w("\u{F1}\u{1F600}"), true), Ok("\u{F1}\u{1F600}".as_bytes().to_vec()));
    assert_eq!(texto::a_estrecho(&[0x41, 0xD800, 0x42], false), Ok(b"A\xEF\xBF\xBDB".to_vec()), "sustituto suelto: EF BF BD");
    assert_eq!(texto::a_estrecho(&[0xD800], true), Err(MalFormado));
    assert_eq!(texto::comparar(&w("abc"), &w("ABC"), false), 1);
    assert_eq!(texto::comparar(&w("abc"), &w("ABC"), true), 0);
    assert_eq!(texto::comparar(&w("ab"), &w("abc"), true), -1);
}

// -- P4f2: mutex y temporizadores ---------------------------------------------------

use crate::hilos::WAIT_ABANDONED_0;

#[test]
fn un_mutex_es_de_un_hilo_con_recursion_y_se_abandona() {
    let mut p = Planificador::nuevo();
    let m = p.nuevo_objeto(Objeto::Mutex { propietario: None, cuenta: 0, abandonado: false });
    let (h1, _) = p.crear(false);
    // El principal lo coge dos veces (recursion).
    assert_eq!(p.esperar(&[m], false, None, 0), Some(WAIT_OBJECT_0));
    assert_eq!(p.esperar(&[m], false, None, 0), Some(WAIT_OBJECT_0));
    assert!(!p.soltar_mutex(m, h1), "el 1 no lo tiene: ERROR_NOT_OWNER");
    // El 1 lo espera y no puede.
    p.actual = h1;
    assert_eq!(p.esperar(&[m], false, None, 0), None);
    p.actual = 0;
    assert!(p.soltar_mutex(m, 0));
    assert_eq!(p.siguiente(0), Turno::Hilo(0), "suelto una vez: sigue siendo del principal");
    assert!(p.soltar_mutex(m, 0));
    assert_eq!(p.siguiente(0), Turno::Hilo(h1), "suelto del todo: el 1 lo coge");
    assert_eq!(p.resultado(h1), WAIT_OBJECT_0);
    // El 1 acaba sin soltarlo: el principal lo coge ABANDONADO, y solo una vez.
    p.terminar(h1, 0);
    assert_eq!(p.esperar(&[m], false, None, 0), Some(WAIT_ABANDONED_0));
    assert!(p.soltar_mutex(m, 0));
    assert_eq!(p.esperar(&[m], false, None, 0), Some(WAIT_OBJECT_0));
}

#[test]
fn un_temporizador_vence_a_su_hora_y_con_periodo_vuelve() {
    let mut p = Planificador::nuevo();
    let t = p.nuevo_objeto(Objeto::Temporizador { manual: false, encendido: false, vence: None, periodo: 0 });
    assert!(p.poner_temporizador(t, 1000, 300));
    assert_eq!(p.esperar(&[t], false, None, 0), None);
    assert_eq!(p.siguiente(10), Turno::Esperar(1000), "esperar un temporizador no es un bloqueo: se duerme hasta el");
    assert_eq!(p.siguiente(1000), Turno::Hilo(0));
    assert_eq!(p.resultado(0), WAIT_OBJECT_0);
    assert_eq!(p.esperar(&[t], false, None, 1100), None, "automatico: lo apago quien lo cogio");
    assert_eq!(p.siguiente(1299), Turno::Esperar(1300));
    assert_eq!(p.siguiente(1300), Turno::Hilo(0), "el periodo: 1000 + 300");
    assert_eq!(p.resultado(0), WAIT_OBJECT_0);
    assert!(p.cancelar_temporizador(t));
    assert_eq!(p.esperar(&[t], false, None, 5000), None);
    assert_eq!(p.siguiente(99_999), Turno::Bloqueo, "cancelado: ya no vence nunca");
}

#[test]
fn la_hora_de_windows_cuenta_desde_1601() {
    use crate::hora;
    assert_eq!(hora::segundos_unix(1970, 1, 1, 0, 0, 0), 0);
    assert_eq!(hora::segundos_unix(2000, 3, 1, 0, 0, 0), 951_868_800, "despues de un 29 de febrero");
    assert_eq!(hora::segundos_unix(2026, 9, 27, 12, 34, 56), 1_790_512_496);
    assert_eq!(hora::filetime(0, 0), 116_444_736_000_000_000);
    assert_eq!(hora::filetime(1, 250), 116_444_736_010_000_002);
}

// -- P4f3: las carpetas -------------------------------------------------------------

#[test]
fn los_comodines_son_los_de_windows() {
    use crate::ficheros::comodin;
    assert!(comodin("*.txt", "a.TXT"), "sin mayusculas que cuenten");
    assert!(comodin("px?.txt", "pxa.txt"));
    assert!(!comodin("px?.txt", "pxab.txt"), "? es UN caracter");
    assert!(comodin("*", "lo_que_sea"));
    assert!(comodin("*.*", "sin_punto"), "*.* es todo, como en Windows");
    assert!(comodin("a*b*c", "aXXbYYc"));
    assert!(!comodin("a*b*c", "aXXbYY"));
    assert!(comodin("**x", "x"));
    assert!(!comodin("*.pak", "datos.pak.bak"));
}

#[test]
fn partir_lo_que_se_busca() {
    use crate::ficheros::partir_patron;
    let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    assert_eq!(partir_patron(&w("*.txt"), "window"), Ok(("window".into(), "*.txt".into())));
    assert_eq!(partir_patron(&w("datos\\*.pak"), "window"), Ok(("window/datos".into(), "*.pak".into())));
    assert_eq!(partir_patron(&w("C:\\*"), "window"), Ok(("".into(), "*".into())), "la raiz del volumen");
    assert_eq!(partir_patron(&w("..\\*"), "window"), Ok(("".into(), "*".into())));
    assert_eq!(partir_patron(&w("..\\..\\*"), "window"), Err(NoRuta::FueraDelVolumen));
    assert_eq!(partir_patron(&w("datos\\"), "window"), Err(NoRuta::NoEsFichero), "sin patron no se busca nada");
    use crate::ficheros::ruta_o_raiz;
    assert_eq!(ruta_o_raiz(&w(".."), "window"), Ok("".into()), "subir desde window: la raiz");
    assert_eq!(ruta_o_raiz(&w("C:\\"), "window"), Ok("".into()));
    assert_eq!(ruta_o_raiz(&w("."), ""), Ok("".into()));
    assert_eq!(ruta_o_raiz(&w("."), "window"), Ok("window".into()));
    assert_eq!(ruta_o_raiz(&w(""), "window"), Err(NoRuta::NoEsFichero), "vacia: nada");
    assert_eq!(ruta_o_raiz(&w("\\\\.\\CON"), "window"), Err(NoRuta::NoEsFichero), "un dispositivo: nada");
}

// -- P4f4: los codigos y sus mensajes ---------------------------------------------

#[test]
fn los_ntstatus_y_los_mensajes_de_windows() {
    use crate::mensajes::{self, *};
    assert_eq!(dos_de_status(STATUS_SUCCESS), 0);
    assert_eq!(dos_de_status(STATUS_END_OF_FILE), 38, "ERROR_HANDLE_EOF");
    assert_eq!(dos_de_status(STATUS_OBJECT_NAME_NOT_FOUND), 2);
    assert_eq!(dos_de_status(STATUS_OBJECT_PATH_NOT_FOUND), 3);
    assert_eq!(dos_de_status(0xC0DE_0001), ERROR_MR_MID_NOT_FOUND, "uno que no se sabe");
    assert_eq!(mensajes::texto(2), Some("El sistema no puede encontrar el archivo especificado."));
    assert!(mensajes::texto(87).unwrap().contains("par\u{e1}metro"));
    assert_eq!(mensajes::texto(0xDEAD), None);
    // Cada error que la casa pone tiene su mensaje.
    for e in [0, 1, 2, 3, 5, 6, 8, 18, 38, 50, 80, 87, 122, 126, 127, 183, 203, 267, 1113, 1460] {
        assert!(mensajes::texto(e).is_some_and(|t| t.ends_with('.')), "{e}");
    }
}

// -- P4f5: el printf de C ----------------------------------------------------------

#[test]
fn el_printf_de_c_da_lo_mismo_que_c() {
    use crate::formato::{a_texto, formatear, Lista};
    fn f(fmt: &[u8], r: &[u64], c: &[(u64, &str)]) -> String {
        a_texto(formatear(fmt, &mut Lista { ranuras: r, cadenas: c, i: 0 }, false))
    }
    assert_eq!(f(b"[%d]", &[42], &[]), "[42]");
    assert_eq!(f(b"[%5d|%-5d|%05d]", &[42, 42, 42], &[]), "[   42|42   |00042]");
    assert_eq!(f(b"[%+d % d]", &[7, 7], &[]), "[+7  7]");
    assert_eq!(f(b"[%d]", &[18446744073709551599], &[]), "[-17]");
    assert_eq!(f(b"[%x %X %#x %o %#o]", &[255, 255, 255, 8, 8], &[]), "[ff FF 0xff 10 010]");
    assert_eq!(f(b"[%.3d]", &[5], &[]), "[005]");
    assert_eq!(f(b"[%10.4f]", &[4614256656543962353u64], &[]), "[    3.1416]");
    assert_eq!(f(b"[%e]", &[4683220299150161609u64], &[]), "[1.234568e+05]");
    assert_eq!(f(b"[%.2E]", &[4548669923058963014u64], &[]), "[1.23E-04]");
    assert_eq!(f(b"[%g %g %g %g]", &[4681608360884174848u64, 4696837146684686336u64, 4547007122018943789u64, 4532020583610935537u64], &[]), "[100000 1e+06 0.0001 1e-05]");
    assert_eq!(f(b"[%#g]", &[4607182418800017408u64], &[]), "[1.00000]");
    assert_eq!(f(b"[%.3g]", &[4614256650576692846u64], &[]), "[3.14]");
    assert_eq!(f(b"[%f]", &[13826050856027422720u64], &[]), "[-0.500000]");
    assert_eq!(f(b"[%.0f %.0f]", &[4602678819172646912u64, 4609434218613702656u64], &[]), "[0 2]");
    assert_eq!(f(b"[%c%c]", &[111, 107], &[]), "[ok]");
    assert_eq!(f(b"[%s|%10s|%-6s|%.2s]", &[0x1001, 0x1002, 0x1003, 0x1004], &[(0x1001, "hola"), (0x1002, "hola"), (0x1003, "hola"), (0x1004, "hola")]), "[hola|      hola|hola  |ho]");
    assert_eq!(f(b"[%%]", &[], &[]), "[%]");
    assert_eq!(f(b"[%u]", &[4294967295], &[]), "[4294967295]");
    assert_eq!(f(b"[%*d|%-*d]", &[6, 42, 6, 42], &[]), "[    42|42    ]");
    assert_eq!(f(b"[%.*f]", &[2, 4613303441197561744u64], &[]), "[2.72]");
    // Lo del UCRT: %p en 16 cifras mayusculas, (null), y %ls ancho.
    assert_eq!(f(b"%p", &[0xABCD], &[]), "000000000000ABCD");
    assert_eq!(f(b"[%s]", &[0], &[]), "[(null)]");
    assert_eq!(f(b"%hhd %hd %lld", &[0x1FF, 0x1FFFF, u64::MAX], &[]), "-1 -1 -1");
    assert_eq!(f(b"%I64u", &[u64::MAX], &[]), "18446744073709551615");
    assert_eq!(f(b"%a y %n", &[], &[]), "%a y %n", "tal cual, sin consumir");
}

#[test]
fn mirar_la_cola_no_saca_nada() {
    use crate::ventanas::{Cola, Msg, WM_QUIT};
    let mut c = Cola::nueva();
    assert_eq!(c.mirar(), None);
    c.salir(5);
    let q = Msg { hwnd: 0, mensaje: WM_QUIT, wparam: 5, lparam: 0 };
    assert_eq!(c.mirar(), Some(q));
    assert_eq!(c.mirar(), Some(q), "mirar dos veces: lo mismo");
    assert_eq!(c.sacar(), Some(q));
    assert_eq!(c.mirar(), None, "sacado, ya no esta");
}

#[test]
fn la_cola_filtrada_de_la_tanda_6() {
    use crate::ventanas::{Cola, Msg, WM_PAINT, WM_QUIT};
    let m = |hwnd, mensaje| Msg { hwnd, mensaje, wparam: 0, lparam: 0 };
    let mut c = Cola::nueva();
    c.publicar(m(0x10, 0x100));
    c.publicar(m(0, 0x8001));
    c.publicar(m(0x20, 0x113));
    c.invalidar(0x20);
    assert_eq!(c.tipos(), 0x1 | 0x8 | 0x10 | 0x20);
    assert_eq!(c.mirar_filtrado(0x20, 0, 0), Some(m(0x20, 0x113)), "solo esa ventana");
    assert_eq!(c.mirar_filtrado(u64::MAX, 0, 0), Some(m(0, 0x8001)), "-1: solo los del hilo");
    assert_eq!(c.sacar_filtrado(0, 0x8000, 0xBFFF), Some(m(0, 0x8001)), "por numero");
    assert_eq!(c.sacar_filtrado(0, 0x8000, 0xBFFF), None);
    assert!(c.espera(0x20, 0x113, 0) && !c.espera(0x20, 0x113, 1));
    assert_eq!(c.sacar_filtrado(0x20, WM_PAINT, WM_PAINT), Some(m(0x20, WM_PAINT)), "el WM_PAINT se sintetiza...");
    assert!(c.por_pintar(0x20), "...y no valida");
    c.salir(3);
    let q = Msg { hwnd: 0, mensaje: WM_QUIT, wparam: 3, lparam: 0 };
    assert_eq!(c.sacar_filtrado(0x20, 0x300, 0x301), Some(q), "WM_QUIT pasa cualquier filtro");
    assert_eq!(c.sacar(), Some(m(0x10, 0x100)), "lo demas sigue en su orden");
    assert!(c.hay_algo());
}

// -- P3c2: D3DCompile pagando una vez -----------------------------------------------

#[test]
fn la_huella_de_una_compilacion_y_su_ent() {
    use crate::sombras::{escribir_ent, huella, leer_ent, nombre, Pedido};
    let p = Pedido { entrada: b"VSMain".to_vec(), perfil: b"vs_5_0".to_vec(), banderas1: 0x800, banderas2: 0, macros: vec![(b"LUZ".to_vec(), b"1".to_vec())] };
    let h = huella(b"float4 f() { return 0; }", &p);
    assert_eq!(nombre(h).len(), 8, "8.3: ocho cifras");
    assert_eq!(h, huella(b"float4 f() { return 0; }", &p.clone()), "la misma, la misma huella");
    // Cada cosa que cambia la salida cambia la huella.
    let mut q = p.clone();
    q.perfil = b"vs_4_0".to_vec();
    assert_ne!(huella(b"float4 f() { return 0; }", &q), h);
    q = p.clone();
    q.banderas1 = 0;
    assert_ne!(huella(b"float4 f() { return 0; }", &q), h);
    q = p.clone();
    q.macros.clear();
    assert_ne!(huella(b"float4 f() { return 0; }", &q), h);
    assert_ne!(huella(b"float4 f() { return 1; }", &p), h);
    // El 0 entre trozos: "ab"+"c" no es "a"+"bc".
    let a = Pedido { entrada: b"ab".to_vec(), perfil: b"c".to_vec(), ..Pedido::default() };
    let b = Pedido { entrada: b"a".to_vec(), perfil: b"bc".to_vec(), ..Pedido::default() };
    assert_ne!(huella(b"", &a), huella(b"", &b));
    // El .ent va y vuelve (y acepta \r\n, por si pasa por Windows).
    let e = escribir_ent(&p);
    assert_eq!(e, b"VSMain\nvs_5_0\n800\n0\nLUZ=1\n");
    assert_eq!(leer_ent(&e), Some(p.clone()));
    assert_eq!(leer_ent(b"VSMain\r\nvs_5_0\r\n800\r\n0\r\nLUZ=1\r\n"), Some(p));
    assert_eq!(leer_ent(b"solo una linea"), None);
}

#[test]
fn los_pendientes_del_cubo_de_bmox12_llevan_la_huella_de_la_casa() {
    use crate::sombras::{huella, leer_ent, nombre};
    // Lo que D3DCompile del cubo de BMOX-12 (estudio_d3d12) deja en BMO-X:
    // su cubo.hlsl tal cual, VSMain/vs_5_0 y PSMain/ps_5_0, sin banderas.
    let fuente = include_bytes!("../prueba/sombras/f3ef42a0.hls");
    for (h, ent) in [("f3ef42a0", &include_bytes!("../prueba/sombras/f3ef42a0.ent")[..]), ("4d67f5e4", &include_bytes!("../prueba/sombras/4d67f5e4.ent")[..])] {
        let p = leer_ent(ent).unwrap();
        assert_eq!(nombre(huella(fuente, &p)), h, "{}", String::from_utf8_lossy(&p.entrada));
    }
}

// -- P5a: lo que da una DLL ------------------------------------------------------------

#[test]
fn las_exportaciones_de_saludo_dll_con_sus_ordinales() {
    use crate::cargar::Funcion;
    use crate::dll::{self, Destino};
    let d = include_bytes!("../prueba/saludo.dll");
    let pe = crate::leer(d).unwrap();
    assert!(pe.es_dll, "IMAGE_FILE_DLL");
    assert!(!crate::leer(include_bytes!("../prueba/usadll.exe")).unwrap().es_dll);
    let img = crate::colocar(&pe, d, pe.base).unwrap();
    let e = dll::exportaciones(&pe, &img).unwrap();
    let v: Vec<(u32, Option<&str>)> = e.iter().map(|x| (x.ordinal, x.nombre.as_deref())).collect();
    assert_eq!(v, [(7, Some("suma")), (8, Some("frase")), (9, Some("visto_attach"))], "los ordinales del .def");
    let por_nombre = dll::buscar(&e, &Funcion::Nombre("suma".into()));
    assert!(matches!(por_nombre, Some(Destino::Rva(r)) if *r > 0));
    assert_eq!(por_nombre, dll::buscar(&e, &Funcion::Ordinal(7)), "por nombre y por ordinal: lo mismo");
    assert_eq!(dll::buscar(&e, &Funcion::Nombre("Suma".into())), None, "el nombre es exacto");
    assert_eq!(dll::fichero("C:\\juego\\SALUDO"), "saludo.dll");
    // Y lo que PIDE una DLL se lee igual que lo de un .exe.
    let imps = crate::importaciones(&pe, &img).unwrap();
    assert_eq!(imps.len(), 1);
    assert_eq!(imps[0].dll.to_ascii_lowercase(), "kernel32.dll");
}
