//! **LAS SUGERENCIAS DE LA CAJA** -- mientras se teclea, la linea de estado
//! dice que ordenes empiezan asi, y TAB completa la orden (no solo rutas).
//!
//! [consumo] NADA      solo cuando cambia lo tecleado: una pasada por una
//!                     lista de unas cuarenta lineas
//!
//! # Por que existe (2026-09-24)
//!
//! Peticion del propietario: la caja de Ctrl+Alt *"parece MAS mezclado"* y
//! pide sugerencias. El consejero se AGREGABA a la salida en cada Ctrl+Alt (tres
//! lineas cada vez, entre las respuestas de las ordenes), y para saber como se
//! llama algo habia que teclear `ayuda` y leer sesenta renglones.
//!
//! # Y no es un segundo catalogo que se pudra
//!
//! `scene::paint_run_box` cuenta por que la pista de la caja dejo de ser una
//! lista: una lista copiada a mano se separa de la de verdad. Esta es una
//! lista, asi que cada linea se PASA POR [`parse`] antes de ofrecerse: si un
//! verbo deja de existir, contesta `Unknown` y deja de sugerirse solo. Lo que
//! no puede saber `parse` es si una suborden (`gpu objetos`) sigue viva:
//! esas las cuida quien las borre.

use super::{parse, Command};

/// Cuantas se muestran a la vez.
pub(crate) const MAX: usize = 4;

/// `(linea, que hace)`, de lo que mas se usa a lo que menos.
const LISTA: &[(&[u8], &[u8])] = &[
    (b"save mode", b"la verificacion total: cada paso de la GPU con su save antes"),
    (b"save", b"el INFORME MAESTRO en datos/salida.txt"),
    (b"save mode off", b"desarma el modo: al arrancar ya no se repite"),
    (b"save auto", b"guardar solo antes de lo arriesgado"),
    (b"save manual", b"guardar solo cuando se teclea"),
    (b"gpu", b"la 3060: el rayo, el GSP y cada fila de la verificacion"),
    (b"gpu init", b"corre el secuenciador y arranca el GSP-RM"),
    (b"gpu estatica", b"la primera RPC: lo que el GSP-RM dice de la 3060"),
    (b"gpu objetos", b"nuestro cliente, dispositivo y subdispositivo en el RM"),
    (b"gpu salud", b"temperatura, enlace PCIe y P-state de la 3060"),
    (b"gpu bar1", b"devolverle a BAR1 la del GOP"),
    (b"gpu vbios", b"la VBIOS y su FWSEC, solo lectura"),
    (b"gpu gsp", b"el firmware del GSP y su reparto de la VRAM"),
    (b"iommu", b"la frontera del DMA de todo aparato"),
    (b"info", b"RAM, CPU, tareas y disco"),
    (b"consumo", b"nucleos, MHz, vatios y RAM en tabla"),
    (b"cpu", b"el procesador y su reloj"),
    (b"mem", b"la memoria"),
    (b"apps", b"que programa tiene RAM pedida"),
    (b"disco", b"el aparato, lo que queda y lo devuelto"),
    (b"ls", b"que hay en el disco"),
    (b"lee", b"que hay DENTRO de un fichero"),
    (b"cabina", b"lo que el kernel apunto"),
    (b"cabina fallos", b"solo los fallos"),
    (b"fallo", b"la ultima autopsia de Ring 3"),
    (b"red", b"tarjeta, enlace y tramas"),
    (b"smp all", b"levantar todos los nucleos"),
    (b"banda", b"el ancho de banda de la RAM"),
    (b"audio", b"el aparato de sonido"),
    (b"ext", b"que ofrece el silicio y que coge BMO"),
    (b"cache", b"L1, L2 y L3 medidas"),
    (b"captura", b"la pantalla a capturas/"),
    (b"aspecto", b"el editor de colores del escritorio"),
    (b"calc", b"la calculadora"),
    (b"perf", b"lo que cuesta pintar"),
    (b"guia", b"por donde empezar"),
    (b"ayuda", b"la lista entera, por categorias"),
    (b"clear", b"limpia esta salida"),
    (b"reboot", b"reinicia la maquina"),
];

fn baja(c: u8) -> u8 {
    c.to_ascii_lowercase()
}

fn empieza(linea: &[u8], tecleado: &[u8]) -> bool {
    linea.len() >= tecleado.len() && linea.iter().zip(tecleado).all(|(&a, &b)| baja(a) == baja(b))
}

/// La linea todavia es una orden de esta casa.
fn viva(linea: &[u8]) -> bool {
    !matches!(parse(linea), Command::Unknown)
}

/// Las que empiezan por lo tecleado: sus indices en la lista, y cuantas hay
/// en total (pueden ser mas de [`MAX`]).
#[derive(Clone, Copy)]
pub(crate) struct Sugeridas {
    pub i: [usize; MAX],
    pub n: usize,
    pub total: usize,
}

/// **Las sugerencias para lo tecleado.** Vacio no sugiere nada; lo que ya es
/// una orden entera se sugiere igual si hay otras mas largas (`save` -> `save
/// mode`), pero no sola: repetirle a alguien lo que acaba de escribir no ayuda.
pub(crate) fn para(tecleado: &[u8]) -> Sugeridas {
    let mut s = Sugeridas { i: [0; MAX], n: 0, total: 0 };
    let t = {
        let mut k = 0;
        while k < tecleado.len() && tecleado[k] == b' ' {
            k += 1;
        }
        &tecleado[k..]
    };
    if t.is_empty() {
        return s;
    }
    for (k, (linea, _)) in LISTA.iter().enumerate() {
        if empieza(linea, t) && viva(linea) {
            if s.n < MAX {
                s.i[s.n] = k;
                s.n += 1;
            }
            s.total += 1;
        }
    }
    if s.total == 1 && LISTA[s.i[0]].0.len() == t.len() {
        s.n = 0;
        s.total = 0;
    }
    s
}

pub(crate) fn linea(i: usize) -> &'static [u8] {
    LISTA[i].0
}

pub(crate) fn que(i: usize) -> &'static [u8] {
    LISTA[i].1
}

/// **TAB sobre una orden**: completa hasta donde todas las candidatas
/// coinciden (entera, si es una). `Some(nueva n)` si escribio algo; `None`
/// si no hay orden que completar y el TAB es de rutas.
pub(crate) fn completar(path: &mut [u8], n: usize) -> Option<usize> {
    let t = &path[..n];
    let mut comun: Option<&[u8]> = None;
    for (linea, _) in LISTA {
        if !empieza(linea, t) || !viva(linea) {
            continue;
        }
        comun = Some(match comun {
            None => linea,
            Some(c) => {
                let k = c.iter().zip(linea.iter()).take_while(|(a, b)| baja(**a) == baja(**b)).count();
                &c[..k]
            }
        });
    }
    let c = comun?;
    if c.len() <= n || c.len() > path.len() {
        return None;
    }
    path[n..c.len()].copy_from_slice(&c[n..]);
    Some(c.len())
}
