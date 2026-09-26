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
//! # ** Y CADA CAPITULO EN SU HOJA (2026-09-21)
//!
//! Eddi, con el primer informe de 412 lineas delante: *"dividir en hojas, el
//! uso, el consumo y todo eso, dividiendo en .txt en otros archivos para
//! facilitar las lecturas nada mas"*. El mismo `save` escribe ahora DOS cosas:
//!
//! ```text
//!    datos/salida.txt      el informe ENTERO, como hasta hoy (lo lee
//!                          `c/leer.bex`, y es lo que se compara de un
//!                          arranque a otro)
//!    informe/INDICE.TXT    la cabecera y que hoja es que
//!    informe/SESION.TXT    1     informe/PROGRAMA.TXT   5
//!    informe/MAQUINA.TXT   2     informe/DISCO.TXT      6
//!    informe/MEMORIA.TXT   3     informe/AUTOPSIA.TXT   7
//!    informe/CONSUMO.TXT   4
//! ```
//!
//! Cada fila se pinta UNA vez y se escribe en las dos: el camino de salida
//! sigue siendo uno. Los nombres son 8.3 porque el volumen es FAT32 y el
//! kernel no inventa nombres largos. La carpeta `informe/` la pone el build en
//! el disco de datos (`ejemplos.ps1`): FAT32 sabe crear ficheros, no carpetas,
//! y ensenarle seria codigo de Ring 0 para ahorrarse una linea de PowerShell.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! [carril]  VERDE     pinta lo que el kernel contesta y lo escribe; no decide
//! [cuesta]  DATO      pregunta a la maquina (INFO) y escribe un fichero; una
//!                     fila mal leida burla al que mira, no a la maquina
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

/// Donde van las hojas. La barra va dentro para que el nombre se pegue.
const CARPETA: &[u8] = b"informe/";

/// Las siete hojas, en el orden de los capitulos, y el fichero de cada una.
const HOJAS: [(&[u8], &[u8]); 7] = [
    (b"SESION.TXT",   b"1. LA SESION -- lo que se tecleo y lo que contesto"),
    (b"MAQUINA.TXT",  b"2. LA MAQUINA -- cpu, caches medidas, extensiones"),
    (b"MEMORIA.TXT",  b"3. LA MEMORIA -- marcos, entregas, cache de disco"),
    (b"CONSUMO.TXT",  b"4. EL CONSUMO -- escritorio, RAM, tareas, DMA, usb, prestamos, avisos"),
    (b"PROGRAMA.TXT", b"5. LOS PROGRAMAS -- memoria pedida y la ficha BEF2 de cada uno"),
    (b"DISCO.TXT",    b"6. EL DISCO -- aparato, particiones, ESTRATOS"),
    (b"AUTOPSIA.TXT", b"7. LA AUTOPSIA -- el ultimo fallo de Ring 3"),
];

/// **El informe entero, a `dest`, y cada capitulo a su hoja en `informe/`.**
/// Devuelve `(bytes, lineas, hojas)`: los bytes y las lineas del informe
/// entero, y cuantas hojas se pudieron abrir (9 = el indice, las siete y
/// DATOS.TXT).
///
/// [!] Una hoja que no se pueda crear NO para el informe: `dest` se escribe
/// igual y el numero de hojas dice cuantas faltaron. La carpeta puede no
/// existir en un disco de datos viejo, y eso no es motivo para perder el
/// informe entero.
#[inline(never)]
pub(crate) fn maestro(dsk: &mut Desktop, dest: &[u8], rayo: bmo::CuentasRayo) -> Result<(usize, usize, usize), u32> {
    let a = bmo::Archivo::create(dest)?;
    let mut c = Cuenta { bytes: 0, lineas: 0 };
    let mut hojas = 0usize;
    // La grabadora: cada `fila` de aqui hasta `parar` va tambien a
    // DATOS.TXT, la hoja para maquinas. Ver `datos.rs`.
    super::datos::empezar();
    let tick = &dsk.tick;
    let g = &mut dsk.out.grid;

    // La cabecera va al informe entero y al INDICE, que ademas lista las hojas.
    let m = g.mark();
    cabecera(g);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);
    if let Some(h) = hoja(b"INDICE.TXT") {
        let mut ch = Cuenta { bytes: 0, lineas: 0 };
        volcar(&h, g, f, t, &mut ch);
        h.write(b"\r\n    hojas de este informe, una por capitulo:\r\n");
        for (fichero, titulo) in HOJAS.iter() {
            h.write(b"      ");
            h.write(fichero);
            for _ in fichero.len()..14 {
                h.write(b" ");
            }
            h.write(titulo);
            h.write(b"\r\n");
        }
        h.write(b"      DATOS.TXT     los numeros de todas, una linea por dato, para una maquina\r\n");
        if h.close() {
            hojas += 1;
        }
    }
    for (n, (fichero, titulo)) in HOJAS.iter().enumerate() {
        let m = g.mark();
        capitulo(g, titulo);
        super::datos::capitulo(n as u8 + 1);
        match n {
            0 => {}
            1 => {
                super::reports::report_cpu(g, tick.consumo.ultimo);
                super::reports::report_cache(g);
                super::reports::report_ext(g);
                super::gpu::report_gpu(g, Some(rayo));
                super::iommu::report_iommu(g);
                super::metiche::report_metiche(g);
            }
            2 => super::reports::report_memory(g),
            3 => super::reports::report_consumo(g, tick),
            4 => {
                super::reports::report_apps(g);
                report_programas(g);
            }
            5 => super::reports::report_disco(g),
            _ => super::reports::report_autopsy(g),
        }
        let (f, t) = g.rows_since(m);
        let h = hoja(fichero);
        volcar(&a, g, f, t, &mut c);
        let mut ch = Cuenta { bytes: 0, lineas: 0 };
        if let Some(h) = h.as_ref() {
            volcar(h, g, f, t, &mut ch);
        }
        // La sesion es lo que ya estaba en el anillo ANTES del rotulo: se
        // vuelca despues de el, y es lo primero que el anillo tira.
        if n == 0 {
            let (desde, _) = g.all_rows();
            if desde < f {
                volcar(&a, g, desde, f - 1, &mut c);
                if let Some(h) = h.as_ref() {
                    volcar(h, g, desde, f - 1, &mut ch);
                }
            }
        }
        if let Some(h) = h {
            if h.close() {
                hojas += 1;
            }
        }
    }
    // ** LA HOJA PARA MAQUINAS, la ultima: lo mismo que las siete, sin
    // tipografia. Se escribe directa al fichero, no pasa por la pantalla:
    // es la unica, y a proposito -- son los mismos numeros que ya se vieron.
    super::datos::parar();
    if let Some(h) = hoja(b"DATOS.TXT") {
        super::datos::volcar(&h);
        if h.close() {
            hojas += 1;
        }
    }
    let m = g.mark();
    regla(g);
    // ** LO QUE HAY QUE PEGAR, AL FINAL (26-09). El propietario copia el final
    // del informe, y la fila `autopsia` vivia arriba, en la seccion de la GPU:
    // dos informes seguidos llegaron sin ella. Aqui va otra vez, junta con el
    // metiche, donde se copia seguro -- sin tener que acordarse.
    g.with_ink(INK_ECHO);
    g.text(b"  PARA PEGAR -- la autopsia del booter y el chisme del bus:\n");
    g.with_ink(INK_PLAIN);
    // LA RECETA (26-09, metal 21:16): que se pidio de verdad en ESTE
    // arranque. Sin esta fila, un `-frontera` que no llego a armarse o un
    // build viejo se confunden con un resultado.
    super::tabla::campo(g, b"receta");
    let mut args = [0u8; 64];
    match super::verificar::receta(&mut args) {
        Some(0) => g.text(b"save mode (sin quitar nada)"),
        Some(n) => {
            g.text(b"save mode ");
            g.text(&args[..n]);
            // El POR DEFECTO (`init`), y por que no habia uno armado.
            let q = super::verificar::por_que_no();
            if q & 0xFF != 0 {
                g.text(match q & 0xFF {
                    1 => b" (POR DEFECTO: datos/modo.txt NO SE PUDO ABRIR, codigo " as &[u8],
                    2 => b" (POR DEFECTO: datos/modo.txt vacio)",
                    _ => b" (POR DEFECTO: no hay uno armado)",
                });
                if q & 0xFF == 1 {
                    g.dec(q >> 8);
                    g.text(b")");
                }
            }
        }
        None => g.text(b"save mode NUNCA: ni panel ni 3060 al arrancar, a proposito"),
    }
    let corrio = |v: u64| if v & bmo::FUEGO_INTENTADO != 0 { b"CORRIO" as &[u8] } else { b"no corrio" };
    g.text(b"; fuego ");
    g.text(corrio(bmo::info(bmo::INFO_GPU_FUEGO)));
    g.text(b", frontera ");
    g.text(corrio(bmo::info(bmo::INFO_GPU_FRONTERA)));
    g.text(b"; build con la autopsia del bus (26-09)\n");
    super::gsp::fila_despierto(g);
    let r = bmo::info(bmo::INFO_METICHE);
    super::tabla::campo(g, b"metiche");
    g.dec(r >> 32 & 0xFFFF);
    g.text(b" funciones confiesan errores ahora, ");
    g.dec(bmo::info(bmo::INFO_METICHE | 1 << 8));
    g.text(b" al arrancar; con algo NUEVO en esta sesion: ");
    g.dec(bmo::info(bmo::INFO_METICHE | 34 << 8).count_ones() as u64);
    g.text(b" (el detalle: capitulo 2, `metiche`)\n");
    g.with_ink(INK_ECHO);
    g.text(b"  fin del informe: ");
    g.dec(c.lineas as u64);
    g.text(b" lineas hasta aqui; ");
    g.dec(super::datos::cuantos() as u64);
    g.text(b" datos en DATOS.TXT\n");
    g.with_ink(INK_PLAIN);
    let (f, t) = g.rows_since(m);
    volcar(&a, g, f, t, &mut c);

    if a.close() {
        Ok((c.bytes, c.lineas, hojas))
    } else {
        // El kernel no dice el motivo -- se queda en la CABINA (F11). Lo que
        // si se sabe con certeza es que en el disco NO hay nada.
        Err(0)
    }
}

/// Abre `informe/<fichero>` para escribir. `None` = no se pudo (la carpeta no
/// esta, o el kernel no tiene ranura): el informe entero sigue.
fn hoja(fichero: &[u8]) -> Option<bmo::Archivo> {
    let mut ruta = [0u8; 32];
    let n = CARPETA.len() + fichero.len();
    if n > ruta.len() {
        return None;
    }
    ruta[..CARPETA.len()].copy_from_slice(CARPETA);
    ruta[CARPETA.len()..n].copy_from_slice(fichero);
    bmo::Archivo::create(&ruta[..n]).ok()
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
    s.text(b"  nombre                        ");
    der(s, b"fichero", 10);
    der(s, b"mapeado", 9);
    s.text(b"  admitido\n");
    s.text(b"    -");
    der(s, b"---", 6);
    der(s, b"---", 6);
    s.text(b"  ------------------------------");
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
        // El nombre con el que se lanzo. (La etiqueta del kernel,
        // `INFO_TXT_PROG_TAG`, es el mismo nombre para lo que viene del disco
        // y solo distingue a los demos embebidos: no se muestra.)
        s.text(b"  ");
        texto_a(s, bmo::INFO_TXT_PROG_NOMBRE | (n << 8), 30);
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
        // ** Los cierres sin hash se muestran en rojo cuando NO son cero: una
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
    s.text(b"    cuadran = el INDICE (cabecera y tabla de anexos) + cada region con bytes + los\n");
    s.text(b"    relocs; s/hash tiene que ser 0. firma: integridad = hashes sin autor (llego lo\n");
    s.text(b"    que se escribio, no quien); firmado #k = Ed25519 con la clave k del ancla.\n");
    s.text(b"    xcr0 = componentes XSAVE declarados (0x3 = x87+SSE, lo que el kernel preserva).\n");
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
