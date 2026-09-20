//! **`save` sin tema: el INFORME MAESTRO** (2026-09-20).
//!
//! Eddi: *"con el SAVE ese mismo tiene que redactar TODO, pero cambia todo
//! eso en buena presentacion MAESTRA"*.
//!
//! # Lo que habia, y por que no bastaba
//!
//! `save` volcaba el historial de la pantalla con la tabla de consumo pegada
//! al final. Era UN fichero con lo que se tecleo y veinte lineas de la
//! maquina; todo lo demas --el CPU, la memoria, el disco, la autopsia-- existia
//! como informe suelto y habia que pedirlo a mano, de uno en uno, en otro
//! reinicio. Y el `.txt` es lo unico que cruza del Ryzen a este lado.
//!
//! # Lo que hace
//!
//! Un fichero, siete capitulos, cada uno con su rotulo y en el mismo orden
//! siempre, para que dos volcados se comparen poniendolos uno al lado del otro:
//!
//! ```text
//!    1. la sesion      lo que se tecleo y lo que contesto
//!    2. la maquina     cpu, caches medidas, extensiones
//!    3. la memoria     marcos, entregas, cache de disco
//!    4. el consumo     escritorio, RAM, tareas, DMA, usb, prestamos, avisos
//!    5. los programas  memoria pedida + LA FICHA BEF2 de cada uno
//!    6. el disco       aparato, particiones, ESTRATOS
//!    7. la autopsia    el ultimo fallo de Ring 3, con su rastro
//! ```
//!
//! # ** El anillo de la pantalla mide 200 filas, y eso decide la forma
//!
//! Todo lo que se guarda pasa por la pantalla (un solo camino de salida: lo que
//! esta en el fichero se vio). Pero siete capitulos son mas de 200 filas, asi
//! que no se pueden pintar todos y volcar al final: los primeros ya se habrian
//! caido del anillo. Se pinta UN capitulo, se vuelca ESE tramo, y se sigue.
//! El historial de la sesion se vuelca el primero de todos, antes de pintar
//! nada mas, porque es lo primero que el anillo tira.
//!
//! [!] Los indices de fila del anillo se MUEVEN cuando desplaza (la fila nueva
//! esta siempre abajo del todo), asi que un tramo se vuelca justo despues de
//! pintarlo y antes de pintar el siguiente. Y ningun capitulo puede pasar de
//! 200 filas o se recorta por arriba sin aviso -- `rows_since` devuelve solo lo
//! que queda. Hoy el mas largo (consumo) anda por 90.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! [carril]  VERDE     pinta lo que el kernel contesta y lo escribe; no decide
//! [cuesta]  DATO      pregunta a la maquina (INFO) y escribe un fichero; una
//!                     fila mal leida engana al que mira, no a la maquina
//! [riesgo]  ESPEJO    desempaqueta bits que empaqueta el kernel; la forma esta
//!                     en `bmo-abi/syscalls/surface/informe.rs`
//! [consumo] NADA      solo corre cuando alguien escribe `save`

use bmo_userland as bmo;

use super::tabla::{fila, section, subregla};
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::OUT_COLS;

/// Cuantas fichas tiene el registro del kernel. Se recorre hasta el cero.
const FICHAS: u64 = 8;

/// **El informe entero, a `dest`.** Devuelve `(bytes, lineas)` escritas.
#[inline(never)]
pub(crate) fn maestro(dsk: &mut Desktop, dest: &[u8]) -> Result<(usize, usize), u32> {
    let a = bmo::Archivo::create(dest)?;
    let mut c = Cuenta { bytes: 0, lineas: 0 };
    let tick = &dsk.tick;
    let g = &mut dsk.out.grid;

    // La portada y el rotulo del capitulo 1 se pintan y se vuelcan ANTES que
    // la sesion, aunque en la pantalla queden debajo: el fichero empieza por
    // decir que es. Y la sesion --lo que ya estaba-- va justo despues, con los
    // indices tomados en el mismo instante, antes de que otro capitulo mueva
    // el anillo.
    let m = g.mark();
    cabecera(g);
    capitulo(g, b"1. LA SESION -- lo que se tecleo y lo que contesto");
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);
    let (desde, _) = g.all_rows();
    if desde < f {
        volcar(&a, g, desde, f - 1, &mut c);
    }

    // 2..7. Un capitulo, un tramo.
    let m = g.mark();
    capitulo(g, b"2. LA MAQUINA -- cpu, caches medidas, extensiones");
    super::reports::report_cpu(g, tick.consumo.ultimo);
    super::reports::report_cache(g);
    super::reports::report_ext(g);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    let m = g.mark();
    capitulo(g, b"3. LA MEMORIA -- marcos, entregas, cache de disco");
    super::reports::report_memory(g);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    let m = g.mark();
    capitulo(g, b"4. EL CONSUMO -- escritorio, RAM, tareas, DMA, usb, prestamos, avisos");
    super::reports::report_consumo(g, tick);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    let m = g.mark();
    capitulo(g, b"5. LOS PROGRAMAS -- memoria pedida y la ficha BEF2 de cada uno");
    super::reports::report_apps(g);
    report_programas(g);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    let m = g.mark();
    capitulo(g, b"6. EL DISCO -- aparato, particiones, ESTRATOS");
    super::reports::report_disco(g);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    let m = g.mark();
    capitulo(g, b"7. LA AUTOPSIA -- el ultimo fallo de Ring 3");
    super::reports::report_autopsy(g);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    // El cierre dice cuanto se escribio: un fichero cortado por un disco lleno
    // se nota porque le falta esta linea.
    let m = g.mark();
    regla(g);
    g.with_ink(INK_ECHO);
    g.text(b"  fin del informe: ");
    g.dec(c.lineas as u64);
    g.text(b" lineas hasta aqui\n");
    g.with_ink(INK_PLAIN);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    if a.close() {
        Ok((c.bytes, c.lineas))
    } else {
        // El kernel no dice el motivo -- se queda en la CABINA (F11). Lo que
        // si se sabe con certeza es que en el disco NO hay nada.
        Err(0)
    }
}

struct Cuenta {
    bytes: usize,
    lineas: usize,
}

/// Las filas `from..=to` del historial, al fichero. `\r\n` por lo mismo que
/// `dump_output`: esto se abre en el bloc de notas de Windows.
#[inline(never)]
fn volcar(a: &bmo::Archivo, g: &Output, from: usize, to: usize, c: &mut Cuenta) {
    for f in from..=to {
        c.bytes += a.write(g.line(f));
        c.bytes += a.write(b"\r\n");
        c.lineas += 1;
    }
}

/// Una regla doble hasta el margen.
#[inline(never)]
fn regla(s: &mut Output) {
    s.with_ink(INK_ECHO);
    s.text(b"  ");
    for _ in 2..OUT_COLS.saturating_sub(2) {
        s.byte(b'=');
    }
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);
}

/// El rotulo de un capitulo: entre dos reglas dobles, para que al hojear el
/// fichero se vea donde empieza cada cosa sin leer.
#[inline(never)]
fn capitulo(s: &mut Output, titulo: &[u8]) {
    regla(s);
    s.with_ink(INK_ECHO);
    s.text(b"  ");
    s.text(titulo);
    s.byte(b'\n');
    regla(s);
}

/// La portada: que maquina, que dia, que capitulos.
#[inline(never)]
fn cabecera(s: &mut Output) {
    regla(s);
    s.with_ink(INK_ECHO);
    const TITULO: &[u8] = b"  BMO-X  --  INFORME MAESTRO";
    s.text(TITULO);
    // La fecha de la placa, pegada al margen derecho, si la sabe.
    let mut fecha = [0u8; 24];
    let n = fecha_en(&mut fecha);
    if n > 0 {
        for _ in (TITULO.len() + n)..OUT_COLS.saturating_sub(2) {
            s.byte(b' ');
        }
        s.text(&fecha[..n]);
    }
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);
    regla(s);

    s.text(b"    formato    BEF2 x86-64 (regiones: codigo, constantes, datos, ceros)\n");
    let mut nombre = [0u8; 48];
    let k = bmo::info_texto(bmo::INFO_TXT_CPU_NOMBRE, &mut nombre);
    s.text(b"    cpu        ");
    if k > 0 {
        s.text(&nombre[..k]);
    } else {
        s.text(b"(sin nombre)");
    }
    s.byte(b'\n');
    fila(s, b"ticks", bmo::info(bmo::INFO_TICKS), b"", b"desde el arranque");
    let vistos = bmo::info(bmo::INFO_PROGRAMAS);
    let olvidados = bmo::info(bmo::INFO_PROGRAMAS_OLVIDADOS);
    let nota: &[u8] = if olvidados > 0 {
        b"y los mas viejos ya no caben en la bitacora"
    } else {
        b""
    };
    fila(s, b"lanzados", vistos + olvidados, b"programas", nota);
    s.text(b"    capitulos  1 la sesion   2 la maquina   3 la memoria   4 el consumo\n");
    s.text(b"               5 los programas   6 el disco   7 la autopsia\n");
}

/// `AAAA-MM-DD HH:MM` de la placa, o 0 si no sabe que dia es.
fn fecha_en(out: &mut [u8; 24]) -> usize {
    let Some(f) = bmo_rtc::desempaquetar(bmo::info(bmo::INFO_FECHA)) else {
        return 0;
    };
    let mut b = [0u8; 24];
    let n = bmo_rtc::escribir(&f, &mut b);
    if n < 16 {
        return 0;
    }
    out[..16].copy_from_slice(&b[..16]);
    16
}

/// **LA FICHA BEF2 DE CADA PROGRAMA**: lo que declaro y lo que el cargador
/// hizo con ello. Ver `INFO_PROG_*` en el ABI.
///
/// Dos tablas y no una porque en 88 columnas no caben quince numeros por
/// fila, y porque son dos preguntas: *quien es y cuanto pesa*, y *que trajo y
/// que se comprobo*.
#[inline(never)]
pub(crate) fn report_programas(s: &mut Output) {
    section(s, b"programas -- la ficha BEF2 de cada uno");
    if bmo::info(bmo::INFO_PROG_QUIEN) == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"    ningun programa lanzado desde el arranque\n");
        s.with_ink(INK_PLAIN);
        return;
    }

    // -- Tabla 1: quien es y cuanto pesa -----------------------------------
    subregla(s, b"quien es y cuanto pesa  (bytes)");
    s.with_ink(INK_ECHO);
    s.text(b"    n");
    der(s, b"pid", 6);
    der(s, b"tid", 6);
    s.text(b"  lenguaje  nombre              ");
    der(s, b"fichero", 10);
    der(s, b"mapeado", 9);
    s.text(b"  admitido\n");
    s.text(b"    -");
    der(s, b"---", 6);
    der(s, b"---", 6);
    s.text(b"  --------  --------------------");
    der(s, b"-------", 10);
    der(s, b"-------", 9);
    s.text(b"  --------\n");
    s.with_ink(INK_PLAIN);
    let mut n = 0u64;
    while n < FICHAS {
        let quien = bmo::info(bmo::INFO_PROG_QUIEN | (n << 8));
        if quien == 0 {
            break;
        }
        let imagen = bmo::info(bmo::INFO_PROG_IMAGEN | (n << 8));
        s.text(b"    ");
        s.dec(n);
        s.dec_right(quien & 0xFFFF, 6);
        s.dec_right((quien >> 16) & 0xFFFF, 6);
        s.text(b"  ");
        texto_a(s, bmo::INFO_TXT_PROG_TAG | (n << 8), 8);
        s.text(b"  ");
        texto_a(s, bmo::INFO_TXT_PROG_NOMBRE | (n << 8), 20);
        s.dec_right(imagen & 0xFFFF_FFFF, 10);
        s.dec_right(imagen >> 32, 9);
        s.text(b"  ");
        if quien >> 63 != 0 {
            s.with_ink(INK_GOOD);
            s.text(b"si");
        } else {
            s.with_ink(INK_ERR);
            s.text(b"NO (no paso)");
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
        n += 1;
    }

    // -- Tabla 2: que trajo y que se comprobo ------------------------------
    subregla(s, b"que trajo y que se comprobo  (bytes en memoria por region)");
    s.with_ink(INK_ECHO);
    s.text(b"    n");
    der(s, b"codigo", 9);
    der(s, b"constantes", 12);
    der(s, b"datos", 9);
    der(s, b"ceros", 9);
    der(s, b"relocs", 7);
    der(s, b"cuadran", 8);
    der(s, b"s/hash", 7);
    s.text(b"  firma         xcr0\n");
    s.text(b"    -");
    der(s, b"------", 9);
    der(s, b"----------", 12);
    der(s, b"-----", 9);
    der(s, b"-----", 9);
    der(s, b"------", 7);
    der(s, b"-------", 8);
    der(s, b"------", 7);
    s.text(b"  ------------  -----\n");
    s.with_ink(INK_PLAIN);
    let mut n = 0u64;
    while n < FICHAS {
        let quien = bmo::info(bmo::INFO_PROG_QUIEN | (n << 8));
        if quien == 0 {
            break;
        }
        let cierre = bmo::info(bmo::INFO_PROG_CIERRE | (n << 8));
        s.text(b"    ");
        s.dec(n);
        for region in 0..4u64 {
            let bytes = bmo::info(bmo::INFO_PROG_REGION | ((n * 4 + region) << 8));
            s.dec_right(bytes, if region == 1 { 12 } else { 9 });
        }
        s.dec_right(cierre & 0xFFFF, 7);
        s.dec_right((cierre >> 16) & 0xFF, 8);
        // ** Los cierres sin hash se ensenan en rojo cuando NO son cero: una
        // imagen del disco que paso por el escritor promete hashes de todo, y
        // un "sin hash" ahi es que el escritor dejo de firmar algo.
        let sin = (cierre >> 24) & 0xFF;
        if sin > 0 {
            s.with_ink(INK_ERR);
        }
        s.dec_right(sin, 7);
        s.with_ink(INK_PLAIN);
        s.text(b"  ");
        match (quien >> 40) & 0xFF {
            0 => s.text(b"sin tabla   "),
            1 => s.text(b"integridad  "),
            _ => {
                s.with_ink(INK_GOOD);
                s.text(b"firmado #");
                s.dec((quien >> 48) & 0xFF);
                s.with_ink(INK_PLAIN);
                s.text(b"  ");
            }
        }
        s.text(b"  0x");
        s.hex(cierre >> 32, 3);
        s.byte(b'\n');
        n += 1;
    }
    s.with_ink(INK_ECHO);
    s.text(b"    firma: sin tabla = imagen embebida, no promete hashes; integridad = hashes y\n");
    s.text(b"    ALGO_NINGUNO (llego lo que se escribio, no quien); firmado #k = Ed25519 y la\n");
    s.text(b"    clave k del ancla. xcr0 = componentes XSAVE declarados (0x7 = x87+SSE+AVX).\n");
    s.with_ink(INK_PLAIN);
}

/// Un rotulo de columna alineado a la derecha, como los numeros de debajo.
#[inline(never)]
fn der(s: &mut Output, txt: &[u8], ancho: usize) {
    for _ in txt.len()..ancho {
        s.byte(b' ');
    }
    s.text(txt);
}

/// Un campo de texto del kernel, a ancho fijo (recortado o rellenado).
#[inline(never)]
fn texto_a(s: &mut Output, campo: u64, ancho: usize) {
    let mut buf = [0u8; 32];
    let k = bmo::info_texto(campo, &mut buf).min(ancho);
    s.text(&buf[..k]);
    for _ in k..ancho {
        s.byte(b' ');
    }
}
