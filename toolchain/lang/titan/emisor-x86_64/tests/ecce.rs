//! ** PL1 y PL3 de `docs/plan/PLAN_ECCE.md` (15-10): ECCE, EL BLOC DE NOTAS
//! (`Ultra_userspace/apps/ecce/`), ejecutado en el emulador con un escritorio
//! de mentira que le manda las teclas.
//!
//! ```text
//!    escribir      las letras entran donde esta el cursor, Enter parte,
//!                  Retroceso y Supr borran; Esc guarda datos/notas.txt
//!    la nota       la de antes se abre (sin los \r de Windows) y el cursor
//!                  queda al final
//!    moverse       flechas, Inicio y Fin; arriba y abajo por la linea que
//!                  SE VE, partida en el ultimo espacio que cabe
//!    nada nuevo    Esc sin escribir no toca el disco
//!    deshacer      Alt+Z por palabras y Alt+Y rehace; MIL ediciones al azar
//!                  deshechas dan el texto de partida, byte a byte (PL1)
//!    la ventana    la cabecera con su raya de oro, y la tinta de lo escrito
//!    la mesa       Alt + rueda aleja: las hojas en chico y los nodos; un
//!                  clic en un nodo abre su hoja (PL9b)
//!    copiar        Mayus + flechas seleccionan; Alt+C, Alt+X, Alt+V; lo
//!                  que se escribe reemplaza lo seleccionado; Alt+Z lo deshace
//!    el guion      Fountain: cada elemento en su columna; en la mesa, los
//!                  nodos son ESCENAS en filas por ACTO (PL2, PL4, PL9b)
//!    Final Draft   Tab cambia lo que es la linea (y en el dialogo, el
//!                  parentesis); Enter baja al dialogo o deja el blanco (PL3)
//!    el documento  el que le pasan (un .fou del explorador); Alt+F guarda
//!                  como guion en datos/guion.fou (PL3)
//!    el raton      el clic pone el cursor, arrastrar selecciona (PL3)
//! ```

use std::path::Path;

use bmo_abi::syscalls::surface::{SUP_CABECERA, SUP_EV_CARACTER, SUP_EV_RATON, SUP_EV_RUEDA, SUP_RUEDA_ALT};
use bmo_lower::emu::{cargar_bex, run, run_acotado, Machine};

const NOTAS: &str = "datos/notas.txt";

fn ecce() -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/ecce");
    let src = std::fs::read_to_string(dir.join("src/main.titan")).expect("el main");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(dir.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn letra(c: u8) -> u64 {
    SUP_EV_CARACTER | 0x100 | 0x200 | c as u64
}

/// Una tecla pulsada, por su scancode (los de navegacion: `bmo_uhid::SC_*`).
fn tecla(sc: u64) -> u64 {
    0x100 | 0x200 | sc
}

const ARRIBA: u64 = 0x66;
const ABAJO: u64 = 0x67;
const IZQUIERDA: u64 = 0x68;
const DERECHA: u64 = 0x69;
const INICIO: u64 = 0x6B;
const SUPR: u64 = 0x6D;
const FIN: u64 = 0x6E;
const RETROCESO: u8 = 8;
const ENTER: u8 = 13;

fn texto(s: &str) -> Vec<u64> {
    s.bytes().map(letra).collect()
}

/// ECCE lanzada por el escritorio, con `nota` en el disco (o ninguna), la
/// medida de la ventana (o ninguna), los eventos y Esc.
fn con(nota: Option<&[u8]>, medida: Option<&str>, eventos: &[u64]) -> Machine {
    con_tope(nota, medida, eventos, 3_000_000_000)
}

fn con_tope(nota: Option<&[u8]>, medida: Option<&str>, eventos: &[u64], tope: usize) -> Machine {
    con_bex_tope(&ecce(), nota, medida, eventos, tope)
}

fn con_bex_tope(bex: &[u8], nota: Option<&[u8]>, medida: Option<&str>, eventos: &[u64], tope: usize) -> Machine {
    let mut m = cargar_bex(bex).expect("el .bex carga");
    m.padre = 7;
    // Las teclas llegan en rafagas (hasta 32 por vuelta, como un teclado
    // rapido): ECCE las lee todas y pinta UNA vez.
    m.buzon_por_vuelta = 32;
    if let Some(n) = nota {
        m.archivos.insert(NOTAS.into(), n.to_vec());
    }
    if let Some(t) = medida {
        m.archivos.insert("datos/resol.txt".into(), t.as_bytes().to_vec());
    }
    for &e in eventos {
        m.buzon_pendiente.push_back(e);
    }
    m.buzon_pendiente.push_back(letra(27));
    let m = run(m, tope);
    assert!(m.exited, "{}", m.console);
    m
}

fn nota(m: &Machine) -> Option<String> {
    m.archivos.get(NOTAS).map(|b| String::from_utf8_lossy(b).into_owned())
}

/// La ventana: `(ancho, alto, pixel(x, y))`, sin alfa.
fn pantalla(m: &Machine) -> (u64, u64, impl Fn(u64, u64) -> u32 + '_) {
    assert_eq!(m.ofertas.len(), 1, "UNA superficie: {:?}", m.ofertas);
    let (base, desde, _, destino) = m.ofertas[0];
    assert_eq!(destino, 7);
    let s = base + desde;
    let (w, h) = (m.read_u64(s + 4) as u32 as u64, m.read_u64(s + 8) as u32 as u64);
    (w, h, move |x: u64, y: u64| m.read_u64(s + SUP_CABECERA + 4 * (y * w + x)) as u32 & 0x00FF_FFFF)
}

const FONDO: u32 = 0x18_18_20;
const TINTA: u32 = 0xE8_E2_D0;
const ORO: u32 = 0xD8_A6_57;

#[test]
fn without_a_desktop_it_says_so() {
    let mut m = cargar_bex(&ecce()).unwrap();
    m.padre = 0;
    let m = run(m, 50_000_000);
    assert_eq!(m.console, "ECCE es una ventana: abrela desde el escritorio\n");
    assert!(nota(&m).is_none());
}

/// ** Escribir, partir la linea, borrar con Retroceso, y Esc guarda.
#[test]
fn typing_enter_and_backspace_are_saved_on_esc() {
    let mut ev = texto("hola");
    ev.push(letra(ENTER));
    ev.extend(texto("mundoo"));
    ev.push(letra(RETROCESO));
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("hola\nmundo"));
    assert_eq!(m.console, "");
}

/// ** La nota de antes se abre sin los `\r` de Windows, con el cursor al
/// final; las tildes y la ene son un byte Latin-1 y se guardan igual.
#[test]
fn the_old_note_opens_with_the_cursor_at_the_end() {
    let m = con(Some(b"una\r\nnota "), None, &[letra(0xF1), letra(b'u')]);
    assert_eq!(m.archivos.get(NOTAS).map(|v| v.as_slice()), Some(&b"una\nnota \xF1u"[..]));
}

/// ** Esc sin escribir nada: el disco no se toca (ni se crea una nota vacia).
#[test]
fn esc_without_changes_writes_nothing() {
    let m = con(None, None, &[tecla(IZQUIERDA), tecla(ARRIBA)]);
    assert!(nota(&m).is_none(), "{:?}", nota(&m));
    let m = con(Some(b"igual"), None, &[tecla(INICIO)]);
    assert_eq!(nota(&m).as_deref(), Some("igual"));
}

/// ** Moverse: izquierda, Inicio, Fin, Supr, y arriba y abajo por las lineas
/// partidas por un `\n`, a la misma columna (o al final, si es mas corta).
#[test]
fn arrows_home_end_and_delete_move_through_the_text() {
    // "abc|" -> izquierda dos veces -> "a|bc" -> X -> "aX|bc"
    let mut ev = texto("abc");
    ev.extend([tecla(IZQUIERDA), tecla(IZQUIERDA)]);
    ev.push(letra(b'X'));
    // Inicio va a 0 y Supr quita la `a`: "|Xbc".
    ev.extend([tecla(INICIO), tecla(SUPR)]);
    // Fin y una letra al final; derecha en el final no hace nada.
    ev.extend([tecla(FIN), tecla(DERECHA), letra(b'!')]);
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("Xbc!"));

    // Arriba y abajo: "largo\nab\nfinal", el cursor al final de "final" (col 5).
    // Arriba: "ab" es mas corta, queda en su final (col 2) -> "ab|" + "1".
    // Arriba otra vez: "largo" en col 3 -> "lar|go" + "2".
    // Abajo: "ab1" en col 4 -> su final (3) -> + "3".
    let mut ev = vec![tecla(ARRIBA), letra(b'1'), tecla(ARRIBA), letra(b'2'), tecla(ABAJO), letra(b'3')];
    ev.push(tecla(ABAJO));
    ev.push(tecla(ABAJO));
    ev.push(letra(b'4'));
    let m = con(Some(b"largo\nab\nfinal"), None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("lar2go\nab13\nfina4l"));
}

/// ** Las lineas que SE VEN: una ventana estrecha parte en el ultimo espacio
/// que cabe, y arriba sube por esas lineas, no por los `\n`.
#[test]
fn up_moves_through_the_wrapped_lines() {
    // 240 de ancho: (240 - 2 * 28) / 8 = 23 columnas.
    // "aaaa bbbb cccc dddd eeee ffff" son 29: la letra 24 cae en "eeee", asi
    // que parte en el espacio de antes: "aaaa bbbb cccc dddd" + "eeee ffff".
    // El cursor al final (col 9 de "eeee ffff"); arriba va a la col 9 de la
    // primera: "aaaa bbbb|" -> "X".
    let m = con(Some(b"aaaa bbbb cccc dddd eeee ffff"), Some("240 160\n"), &[tecla(ARRIBA), letra(b'X')]);
    assert_eq!(nota(&m).as_deref(), Some("aaaa bbbbX cccc dddd eeee ffff"));
}

/// ** Retroceso al principio y Supr al final no rompen nada; Tab son cuatro
/// espacios.
#[test]
fn edges_do_nothing_and_tab_is_four_spaces() {
    let mut ev = vec![letra(RETROCESO), tecla(SUPR), letra(b'a'), letra(9), letra(b'b'), tecla(SUPR)];
    ev.extend([tecla(INICIO), letra(RETROCESO)]);
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("a    b"));
}

/// ** La ventana: la cabecera con su raya de oro, el fondo de noche, y la
/// tinta de lo escrito donde va la primera letra.
#[test]
fn the_window_shows_the_ink_on_the_night() {
    let m = con(Some(b"M"), None, &[]);
    let (w, h, pixel) = pantalla(&m);
    assert_eq!((w, h), (720, 480));
    // La raya de oro bajo la cabecera, de lado a lado.
    for x in [0, 300, 719] {
        assert_eq!(pixel(x, 39), ORO, "x {x}");
    }
    // El fondo del texto, lejos de la letra.
    assert_eq!(pixel(400, 200), FONDO);
    // La `M` en (28, 52): alguno de sus pixeles es TINTA.
    let tinta = (52..68).flat_map(|y| (28..36).map(move |x| (x, y))).filter(|&(x, y)| pixel(x, y) == TINTA).count();
    assert!(tinta > 10, "la M tiene {tinta} pixeles de tinta");
    // El cursor detras de la `M`: una barra de oro en x = 36.
    assert_eq!(pixel(36, 60), ORO);
}

/// La tecla Alt (scancode 56) al bajar y al subir.
fn alt_baja() -> u64 {
    tecla(56)
}
fn alt_sube() -> u64 {
    0x100 | 56
}

/// Alt+Z y Alt+Y, con la letra cocida que tambien llega (y no se escribe).
fn deshacer() -> Vec<u64> {
    vec![alt_baja(), tecla(44), letra(b'z'), alt_sube()]
}
fn rehacer() -> Vec<u64> {
    vec![alt_baja(), tecla(21), letra(b'y'), alt_sube()]
}

/// ** Deshacer va por PALABRAS (lo que se escribe seguido), rehacer las
/// vuelve, y escribir otra cosa tira lo deshecho.
#[test]
fn undo_goes_word_by_word_and_redo_brings_it_back() {
    let mut ev = texto("hola mundo");
    ev.extend(deshacer());
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("hola"));

    let mut ev = texto("hola mundo");
    ev.extend(deshacer());
    ev.extend(deshacer());
    ev.extend(rehacer());
    ev.push(letra(b'!'));
    // Rehacer ya no tiene nada: "!" tiro lo deshecho.
    ev.extend(rehacer());
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("hola!"));

    // Un borrado seguido es UN paso; Supr tambien.
    let mut ev = texto("abc");
    ev.extend([letra(RETROCESO), letra(RETROCESO), letra(RETROCESO)]);
    ev.extend(deshacer());
    ev.push(letra(b'.'));
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("abc."));
    let ev = [vec![tecla(INICIO), tecla(SUPR), tecla(SUPR)], deshacer(), texto("-")].concat();
    let m = con(Some(b"xyz"), None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("-xyz"));
}

/// Mil ediciones al azar (letras, espacios, Enter, Retroceso, Supr y las
/// flechas), las mismas siempre: un LCG con su semilla.
fn al_azar(n: usize) -> Vec<u64> {
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut ev = Vec::new();
    for _ in 0..n {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r = (x >> 33) % 20;
        ev.push(match r {
            0..=7 => letra(b'a' + ((x >> 40) % 26) as u8),
            8 => letra(b' '),
            9 => letra(ENTER),
            10 | 11 => letra(RETROCESO),
            12 => tecla(SUPR),
            13 => tecla(IZQUIERDA),
            14 => tecla(DERECHA),
            15 => tecla(ARRIBA),
            16 => tecla(ABAJO),
            17 => tecla(INICIO),
            18 => tecla(FIN),
            _ => letra(0xF1),
        });
    }
    ev
}

/// ** LA PRUEBA DE PL1: mil ediciones al azar, y deshacerlas TODAS da el
/// texto de partida byte a byte; rehacerlas todas, el de despues.
#[test]
fn a_thousand_random_edits_undo_to_the_start() {
    let antes: &[u8] = b"la nota de antes\ncon dos lineas y alguna palabra larga";
    let azar = al_azar(1000);
    // La ventana mas chica (240 x 160): cada tecla repinta, y aqui se prueba
    // el texto, no los pixeles. Las lineas se parten mas, que tambien vale.
    let chica = Some("240 160\n");
    let despues = con_tope(Some(antes), chica, &azar, 40_000_000_000);
    let despues = despues.archivos.get(NOTAS).cloned().expect("guardo");
    assert_ne!(despues, antes, "las ediciones cambiaron algo");

    let mut ev = azar.clone();
    for _ in 0..1000 {
        ev.extend(deshacer());
    }
    let m = con_tope(Some(antes), chica, &ev, 80_000_000_000);
    assert_eq!(m.archivos.get(NOTAS).map(|v| v.as_slice()), Some(antes), "deshacerlo todo vuelve al principio");

    for _ in 0..1000 {
        ev.extend(rehacer());
    }
    let m = con_tope(Some(antes), chica, &ev, 120_000_000_000);
    assert_eq!(m.archivos.get(NOTAS), Some(&despues), "rehacerlo todo vuelve al final");
}

/// ** SU CARA (IC2, 15-10): el `icono.svg` de su Titan.toml, pintado en un
/// BICO de 32 con alfa entero, va DENTRO del `.bex` -- y el `.bex` sigue
/// pasando por la puerta del kernel y corriendo igual.
#[test]
fn ecce_lleva_su_cara_de_32_y_sigue_pasando_la_puerta() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/ecce");
    let toml = std::fs::read(dir.join("Titan.toml")).expect("el Titan.toml");
    let (bex, saltado) = bmo_titan_x86_64::con_cara(Some(&toml), &mut |p| std::fs::read(dir.join(p)).ok(), ecce()).expect("la cara");
    assert!(saltado.is_empty(), "el SVG trae lo que no se sabe pintar: {saltado:?}");
    bmo_bex_gate::revisar(&bex, bex.len()).expect("la puerta del kernel");
    let d = bmo_abi::bef2::paquete::directorio(&bex).expect("el directorio de recursos");
    let cara = d.datos(d.buscar("icono").expect("el recurso icono")).expect("sus bytes");
    assert_eq!(&cara[..8], b"BICO\x20\x00\x20\x00");
    assert_eq!(cara.len(), 8 + 32 * 32 * 4);
    // Hay bordes SUAVES: algun pixel ni transparente ni opaco.
    let medios = cara[8..].chunks(4).filter(|p| p[3] != 0 && p[3] != 255).count();
    assert!(medios > 20, "solo {medios} pixeles de borde suave");
    // Y corre igual: escribe y guarda.
    let m = con_bex_tope(&bex, None, None, &texto("hola"), 3_000_000_000);
    assert_eq!(nota(&m).as_deref(), Some("hola"));
}

// -- LA MESA (PL9b de PLAN_ECCE, 15-10) --

const ALT: u64 = 56;

fn rueda(muescas: i8, con_alt: bool) -> u64 {
    SUP_EV_RUEDA | 0x100 | muescas as u8 as u64 | if con_alt { SUP_RUEDA_ALT } else { 0 }
}

fn clic(x: u64, y: u64) -> u64 {
    SUP_EV_RATON | 0x100 | 0x200 | 1 | x << 16 | y << 32
}

/// 120 lineas ("linea 1" .. "linea 120"): tres hojas de 54.
fn tres_hojas() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 1..=120 {
        v.extend_from_slice(format!("linea {i}\n").as_bytes());
    }
    v.pop();
    v
}

/// ECCE con `eventos` y SIN el Esc final: se queda abierta, y lo ultimo que
/// presento es lo que se mira.
fn abierta(nota: &[u8], eventos: &[u64]) -> Machine {
    let mut m = cargar_bex(&ecce()).expect("el .bex carga");
    m.padre = 7;
    m.buzon_por_vuelta = 32;
    m.archivos.insert(NOTAS.into(), nota.to_vec());
    for &e in eventos {
        m.buzon_pendiente.push_back(e);
    }
    // Acotado: ECCE sigue abierta esperando teclas, y eso no es un fallo.
    let (m, _) = run_acotado(m, 150_000_000);
    assert!(!m.exited, "ECCE se cerro sola: {}", m.console);
    m
}

/// ** Alt + rueda dos veces: la mesa y luego los nodos; un clic en el nodo
/// de la hoja 2 la ABRE -- lo que se escribe cae al principio de su primera
/// linea (la 55) --, y la rueda sin Alt, en la hoja, no aleja nada.
#[test]
fn alt_wheel_zooms_out_to_nodes_and_a_click_opens_the_page() {
    // Nodos de 168 x 56, tres por fila desde x = 28, y = 64: el segundo, en
    // x 240..408.
    let ev = [tecla(ALT), rueda(-1, true), rueda(-1, true), 0x100 | ALT, clic(324, 92), letra(b'X')];
    let m = con(Some(&tres_hojas()), None, &ev);
    let n = nota(&m).expect("guardo");
    let lineas: Vec<&str> = n.split('\n').collect();
    assert_eq!(lineas.len(), 120);
    assert_eq!(lineas[54], "Xlinea 55", "el clic no abrio la hoja 2");
    assert_eq!(lineas[53], "linea 54");
}

/// ** La mesa SE VE: el papel de cada hoja, una raya por linea y el marco de
/// oro de la elegida (la del cursor: la tercera).
#[test]
fn the_table_shows_the_pages_from_afar() {
    let m = abierta(&tres_hojas(), &[tecla(ALT), rueda(-1, true)]);
    let (w, _, px) = pantalla(&m);
    assert_eq!(w, 720);
    // La hoja 1 en x 28, y 64: papel arriba, y la raya de "linea 1" (7
    // letras de 83 en 80 px: 6 px) en su primera fila.
    assert_eq!(px(30, 66), 0xFB_FC_F8, "sin papel");
    assert_eq!(px(37, 72), 0x8A_85_86, "sin la raya de la linea 1");
    assert_eq!(px(60, 72), 0xFB_FC_F8, "la raya es mas larga que la linea");
    // La hoja 3, la elegida, en x 276: su marco de oro 3 px por fuera.
    assert_eq!(px(274, 100), 0xD8_A6_57, "sin el marco de la elegida");
    assert_ne!(px(26 + 124, 100), 0xD8_A6_57, "la hoja 2 no es la elegida");
}

/// ** Mas lejos, los NODOS: una tarjeta por hoja y el hilo de oro entre una y
/// la siguiente.
#[test]
fn further_out_the_pages_are_nodes() {
    let m = abierta(&tres_hojas(), &[tecla(ALT), rueda(-1, true), rueda(-2, true)]);
    let (_, _, px) = pantalla(&m);
    assert_eq!(px(28 + 160, 64 + 50), 0x1C_1C_26, "sin la tarjeta de la hoja 1");
    assert_eq!(px(28 + 168 + 20, 64 + 28), 0x8B_60_25, "sin el hilo de la hoja 1 a la 2");
    assert_eq!(px(28 + 168 + 20, 64 + 10), 0x18_18_20, "el hilo es una raya, no una banda");
}

// -- COPIAR Y PEGAR (15-10): Alt, porque Ctrl es del escritorio --

const MAYUS: u64 = 42;
const SC_C: u64 = 46;
const SC_X: u64 = 45;
const SC_V: u64 = 47;
const SC_Z: u64 = 44;

fn con_alt(sc: u64) -> [u64; 3] {
    [tecla(ALT), tecla(sc), 0x100 | ALT]
}

/// ** Mayus + Inicio selecciona la linea, Alt+C la copia, Fin y Alt+V la pegan.
#[test]
fn shift_selects_alt_c_copies_alt_v_pastes() {
    let mut ev = texto("hola mundo");
    ev.extend([tecla(MAYUS), tecla(INICIO), 0x100 | MAYUS]);
    ev.extend(con_alt(SC_C));
    ev.push(tecla(FIN));
    ev.extend(con_alt(SC_V));
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("hola mundohola mundo"));
}

/// ** Alt+X corta lo seleccionado, y Alt+Z lo devuelve entero de una vez.
#[test]
fn alt_x_cuts_and_alt_z_brings_it_back() {
    let mut ev = texto("uno dos");
    ev.extend([tecla(MAYUS), tecla(IZQUIERDA), tecla(IZQUIERDA), tecla(IZQUIERDA), 0x100 | MAYUS]);
    ev.extend(con_alt(SC_X));
    let solo_corte = con(None, None, &ev);
    assert_eq!(nota(&solo_corte).as_deref(), Some("uno "));
    ev.extend(con_alt(SC_Z));
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("uno dos"));
}

/// ** Lo que se escribe con algo seleccionado lo REEMPLAZA (y pegar tambien).
#[test]
fn typing_replaces_the_selection() {
    let mut ev = texto("el gato negro");
    ev.extend([tecla(MAYUS), tecla(IZQUIERDA), tecla(IZQUIERDA), tecla(IZQUIERDA), tecla(IZQUIERDA), tecla(IZQUIERDA), 0x100 | MAYUS]);
    ev.extend(texto("blanco"));
    let m = con(None, None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("el gato blanco"));
}

// -- EL MODO GUION (Fountain) --

const GUION: &str = "# ACTO I\n\nINT. CASA - NOCHE\n\nAna entra.\n\nANA\nHola.\n\nEXT. CALLE - DIA\n\nLlueve.\n\n# ACTO II\n\nINT. BAR - NOCHE\n\nMARCOS\n(bajo)\nOtra.\n\nCORTE A:";

/// La primera x con tinta (lo que no es el fondo) de la linea que se ve `k`.
fn primera_tinta(px: &dyn Fn(u64, u64) -> u32, k: u64) -> Option<u64> {
    let y0 = 52 + k * 20;
    (0..720).find(|&x| (y0..y0 + 16).any(|y| px(x, y) != FONDO))
}

/// ** Con un INT. dentro, ECCE se abre en GUION y cada elemento va en su
/// columna, como en Final Draft: la escena a la izquierda (en oro), el
/// personaje a 22 columnas, el dialogo a 10, el parentesis a 16 y la
/// transicion pegada a la derecha del ancho del guion (60).
#[test]
fn the_screenplay_elements_sit_in_their_columns() {
    // Dos RePag: el cursor arriba, y la primera linea que se ve es la 0.
    let m = abierta(GUION.as_bytes(), &[tecla(0x6C), tecla(0x6C)]);
    let (_, _, px) = pantalla(&m);
    let col = |k: u64| primera_tinta(&px, k).map(|x| (x - 28) / 8);
    let oro_en = |k: u64| (28..200).any(|x| (0..16).any(|y| px(x, 52 + k * 20 + y) == ORO));
    assert_eq!(col(0), Some(0), "el acto");
    assert_eq!(col(2), Some(0), "la escena");
    assert!(oro_en(2), "la escena va en oro");
    assert!(!oro_en(4), "la accion no");
    assert_eq!(col(4), Some(0), "la accion");
    assert_eq!(col(6), Some(22), "el personaje");
    assert_eq!(col(7), Some(10), "el dialogo");
    assert_eq!(col(15), Some(0), "la escena del acto II");
    assert_eq!(col(17), Some(22), "el segundo personaje");
    assert_eq!(col(18), Some(16), "el parentesis");
    assert_eq!(col(19), Some(10), "su dialogo");
}

/// ** La transicion va pegada a la derecha del ancho del guion (60 columnas):
/// "CORTE A:" (8 letras) empieza en la 52.
#[test]
fn the_transition_sits_on_the_right() {
    let m = abierta(GUION.as_bytes(), &[]);
    let (_, _, px) = pantalla(&m);
    // Con el cursor al final se ven las lineas 2..21: la 21 es la fila 19.
    assert_eq!(primera_tinta(&px, 19).map(|x| (x - 28) / 8), Some(52));
}

/// ** En la mesa del guion los nodos son ESCENAS, y cada ACTO empieza fila:
/// el tercer nodo (INT. BAR, del acto II) esta en la segunda fila, y un clic
/// en el abre esa escena.
#[test]
fn in_a_screenplay_the_nodes_are_scenes_by_act() {
    let ev = [tecla(ALT), rueda(-1, true), rueda(-1, true), 0x100 | ALT, clic(28 + 84, 64 + 96 + 28), letra(b'X')];
    let m = con(Some(GUION.as_bytes()), None, &ev);
    let n = nota(&m).expect("guardo");
    assert!(n.contains("\nXINT. BAR - NOCHE\n"), "el clic no abrio la escena 3:\n{n}");
}


// -- PL3: ESCRIBIR UN GUION COMO EN FINAL DRAFT --

const TAB: u8 = 9;

fn mayus_enter() -> [u64; 3] {
    [tecla(MAYUS), letra(ENTER), 0x100 | MAYUS]
}

/// ** Tab y Enter de Final Draft: "ana" + Tab es el personaje (@ANA); Enter
/// baja al dialogo sin linea en blanco; Enter tras el dialogo deja la linea
/// en blanco de Fountain, y lo que sigue es accion.
#[test]
fn tab_and_enter_write_a_screenplay_like_final_draft() {
    let mut ev = texto("ana");
    ev.push(letra(TAB));
    ev.push(letra(ENTER));
    ev.extend(texto("hola"));
    ev.push(letra(ENTER));
    ev.extend(texto("sale"));
    let m = con(Some(b"INT. CASA - DIA\n\n"), None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("INT. CASA - DIA\n\n@ANA\nhola\n\nsale"));
}

/// ** En el dialogo, Tab en una linea en blanco es el PARENTESIS (el cursor
/// dentro); Enter tras el baja al dialogo. Y Tab da la vuelta: accion,
/// personaje, transicion, escena, accion -- y Alt+Z la deja como estaba.
#[test]
fn tab_in_the_dialogue_is_a_parenthetical_and_cycles_the_rest() {
    let mut ev = texto("hola");
    ev.extend(mayus_enter());
    ev.push(letra(TAB));
    ev.extend(texto("bajo"));
    ev.push(tecla(FIN));
    ev.push(letra(ENTER));
    ev.extend(texto("si"));
    let m = con(Some(b"INT. CASA - DIA\n\nANA\n"), None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("INT. CASA - DIA\n\nANA\nhola\n(bajo)\nsi"));

    let base: &[u8] = b"INT. CASA - DIA\n\nsale ana";
    let vueltas = |n: usize| {
        let ev: Vec<u64> = (0..n).map(|_| letra(TAB)).collect();
        nota(&con(Some(base), None, &ev)).unwrap()
    };
    assert_eq!(vueltas(1), "INT. CASA - DIA\n\n@SALE ANA");
    assert_eq!(vueltas(2), "INT. CASA - DIA\n\n>SALE ANA");
    assert_eq!(vueltas(3), "INT. CASA - DIA\n\n.SALE ANA");
    assert_eq!(vueltas(4), "INT. CASA - DIA\n\n!SALE ANA");
    let mut ev = vec![letra(TAB), letra(TAB)];
    ev.extend(con_alt(SC_Z));
    ev.extend(texto("!"));
    assert_eq!(nota(&con(Some(base), None, &ev)).as_deref(), Some("INT. CASA - DIA\n\n@SALE ANA!"));
}

/// ** El documento es el que le pasan (el explorador abre asi un .fou): se
/// lee y se guarda ALLI, y las notas no se tocan. Alt+F guarda como guion,
/// en datos/guion.fou.
#[test]
fn it_opens_the_file_it_is_given_and_alt_f_saves_a_fountain() {
    let mut m = cargar_bex(&ecce()).expect("el .bex carga");
    m.padre = 7;
    m.buzon_por_vuelta = 32;
    m.poner_argumentos("datos/peli.fou");
    m.archivos.insert("datos/peli.fou".into(), b"EXT. MAR - DIA\n\nOlas".to_vec());
    for e in texto(".") {
        m.buzon_pendiente.push_back(e);
    }
    m.buzon_pendiente.push_back(letra(27));
    let m = run(m, 3_000_000_000);
    assert!(m.exited, "{}", m.console);
    assert_eq!(m.archivos.get("datos/peli.fou").map(|v| v.as_slice()), Some(&b"EXT. MAR - DIA\n\nOlas."[..]));
    assert!(nota(&m).is_none());

    let mut ev = texto("una idea");
    ev.extend(con_alt(33));
    let m = con(None, None, &ev);
    assert_eq!(m.archivos.get("datos/guion.fou").map(|v| v.as_slice()), Some(&b"una idea"[..]));
    assert!(nota(&m).is_none(), "Alt+F cambio de documento: las notas no se escriben");
}

fn suelta(x: u64, y: u64) -> u64 {
    SUP_EV_RATON | 0x100 | 0x200 | x << 16 | y << 32
}

fn arrastra(x: u64, y: u64) -> u64 {
    clic(x, y)
}

/// El centro de la letra `col` de la linea que se ve `fila` (sin sangria).
fn letra_en(fila: u64, col: u64) -> (u64, u64) {
    (28 + col * 8 + 2, 52 + fila * 20 + 8)
}

/// ** El clic pone el cursor en la letra; arrastrar selecciona (y Alt+X lo
/// corta); un clic en la cabeza no mueve nada.
#[test]
fn a_click_places_the_cursor_and_a_drag_selects() {
    let (x, y) = letra_en(1, 1);
    let m = con(Some(b"abc\ndef"), None, &[clic(x, y), suelta(x, y), letra(b'X')]);
    assert_eq!(nota(&m).as_deref(), Some("abc\ndXef"));

    let (x0, y0) = letra_en(0, 0);
    let (x1, y1) = letra_en(1, 2);
    let mut ev = vec![clic(x0, y0), arrastra(x1 - 4, y1), arrastra(x1, y1), suelta(x1, y1)];
    ev.extend(con_alt(SC_X));
    let m = con(Some(b"abc\ndef"), None, &ev);
    assert_eq!(nota(&m).as_deref(), Some("f"));

    let m = con(Some(b"abc\ndef"), None, &[clic(40, 10), suelta(40, 10), letra(b'X')]);
    assert_eq!(nota(&m).as_deref(), Some("abc\ndefX"));
}

// -- PL4: LAS PAGINAS DEL OFICIO, en la ventana --

/// ** Donde acaba una pagina, ECCE pinta lo que imprimiria: "(MAS)" en la
/// columna del nombre, la raya con "pag 2", y "ANA (CONT.)". Con el cursor
/// al final (la linea 61) se ven las filas 45..64: el MAS es la fila 9.
#[test]
fn the_page_break_shows_more_the_rule_and_cont() {
    let mut s = String::from("INT. A - DIA\n\n");
    for i in 0..48 {
        s.push_str(&format!("pasa algo {i}\n"));
    }
    s.push_str("\nANA\n");
    for i in 0..10 {
        s.push_str(&format!("frase {i}.\n"));
    }
    s.pop();
    let m = abierta(s.as_bytes(), &[]);
    let (_, _, px) = pantalla(&m);
    let col = |k: u64| primera_tinta(&px, k).map(|x| (x - 28) / 8);
    assert_eq!(col(6), Some(22), "ANA");
    assert_eq!(col(8), Some(10), "la segunda frase, en su dialogo");
    assert_eq!(col(9), Some(22), "el (MAS), donde iria el nombre");
    assert_eq!(col(10), Some(0), "la raya del final de la pagina");
    assert_eq!(col(11), Some(22), "ANA (CONT.)");
    assert_eq!(col(12), Some(10), "la tercera frase, arriba de la pagina 2");
}

// -- PL6: EL PDF --

/// El PDF que ECCE guarda con Alt+P, revisado por dentro: la cabecera, el
/// indice (cada objeto empieza en el byte que dice, y `startxref` apunta al
/// indice), y las paginas que dice `/Count`.
fn pdf_de(guion: &[u8]) -> Vec<u8> {
    let m = con(Some(guion), None, &con_alt(25));
    let pdf = m.archivos.get("datos/guion.pdf").cloned().expect("Alt+P guarda datos/guion.pdf");
    if let Ok(dir) = std::env::var("ECCE_PDF") {
        std::fs::write(dir, &pdf).unwrap();
    }
    let txt = String::from_utf8_lossy(&pdf).into_owned();
    assert!(txt.starts_with("%PDF-1.4\n") && txt.ends_with("%%EOF\n"));
    let sx: usize = txt.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
    assert!(txt[sx..].starts_with("xref\n0 "), "startxref no apunta al indice");
    let filas: Vec<&str> = txt[sx..].lines().skip(3).take_while(|l| !l.starts_with("trailer")).collect();
    for (i, f) in filas.iter().enumerate() {
        assert_eq!(f.len(), 19, "cada fila del indice son 20 bytes con su salto: {f:?}");
        let off: usize = f[..10].parse().unwrap();
        assert!(txt[off..].starts_with(&format!("{} 0 obj\n", i + 1)), "el objeto {} no esta en {off}", i + 1);
    }
    pdf
}

fn paginas_pdf(pdf: &str) -> usize {
    pdf.split("/Count ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap()
}

/// ** Alt+P: el guion en PDF con la Courier del oficio. La escena a 1,5
/// pulgadas (108 puntos) en mayusculas, el personaje a 3,7 (266,4) sin su
/// @, el dialogo a 2,5 (180), el parentesis a 3,1 (223,2) con sus \( \), la
/// transicion pegada a 7,5 pulgadas; y los actos no salen.
#[test]
fn alt_p_writes_the_screenplay_as_a_pdf() {
    let guion = format!("{GUION}\n\n@lola\n(fuera)\nVoy.");
    let pdf = String::from_utf8_lossy(&pdf_de(guion.as_bytes())).into_owned();
    assert_eq!(paginas_pdf(&pdf), 1);
    assert!(pdf.contains("/BaseFont /Courier /Encoding /WinAnsiEncoding"));
    assert!(pdf.contains("1 0 0 1 108.0 710 Tm (INT. CASA - NOCHE) Tj"), "{pdf}");
    assert!(pdf.contains("1 0 0 1 266.4 ") && pdf.contains(" Tm (ANA) Tj"));
    assert!(pdf.contains(" Tm (LOLA) Tj"), "sin la @ y en mayusculas");
    assert!(pdf.contains("1 0 0 1 223.2 ") && pdf.contains(" Tm (\\(bajo\\)) Tj"));
    assert!(pdf.contains("1 0 0 1 180.0 ") && pdf.contains(" Tm (Hola.) Tj"));
    // "CORTE A:" (8 letras): en la columna 52, 108 + 52 * 7,2 = 482,4.
    assert!(pdf.contains("1 0 0 1 482.4 ") && pdf.contains(" Tm (CORTE A:) Tj"));
    assert!(!pdf.contains("ACTO"), "los actos no se imprimen");
}

/// ** Un guion largo: tantas paginas como dice el paginado, el numero desde
/// la segunda ("2." a la derecha), y el dialogo partido con su (MAS) y su
/// ANA (CONT.).
#[test]
fn a_long_screenplay_has_its_pages_numbers_more_and_cont() {
    let mut s = String::from("INT. A - DIA\n\n");
    for i in 0..48 {
        s.push_str(&format!("pasa algo {i}\n"));
    }
    s.push_str("\nANA\n");
    for i in 0..10 {
        s.push_str(&format!("frase {i}.\n"));
    }
    s.pop();
    let pdf = String::from_utf8_lossy(&pdf_de(s.as_bytes())).into_owned();
    assert_eq!(paginas_pdf(&pdf), 2);
    assert!(pdf.contains(" Tm (\\(MAS\\)) Tj"), "el (MAS)");
    assert!(pdf.contains(" Tm (ANA \\(CONT.\\)) Tj"), "ANA (CONT.)");
    assert!(pdf.contains(" 747 Tm (2.) Tj"), "el numero de la pagina 2");
    assert!(!pdf.contains("(1.) Tj"), "la primera no lleva numero");
}
