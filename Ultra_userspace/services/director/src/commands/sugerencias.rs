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
    (b"gpu relojes", b"la 3060 al maximo un minuto (PERF_BOOST al GSP), medida antes y despues con el mismo fotograma"),
    (b"gpu relojes off", b"quita la subida: los relojes vuelven a lo que decida el GSP-RM"),
    (b"gpu vram", b"la CPU escribe en la VRAM (PRAMIN) y la deja como estaba"),
    (b"gpu directorio", b"la raiz del espacio de direcciones de la GPU, en tu VRAM"),
    (b"gpu tramo", b"mapear 64 KiB de tu VRAM en el espacio de la GPU"),
    (b"gpu volcado", b"la 3060 lleva tu escritorio a la pantalla, contra la CPU"),
    (b"gpu motores", b"que motores tiene la 3060 y el de copia para el canal (COPY2)"),
    (b"gpu canal", b"el primer canal de la 3060: pedido, atado a COPY2 y con su ficha"),
    (b"gpu copia", b"el primer trabajo de la 3060: copiar 4 KiB de VRAM por su canal"),
    (b"gpu gr", b"que buferes pide el motor grafico para su contexto (camino al triangulo)"),
    (b"gpu canalgr", b"el canal del motor grafico: pedido, atado a GR0 y en su lista"),
    (b"gpu grmem", b"los buferes del motor grafico en tu VRAM, mapeados para la GPU"),
    (b"gpu oro", b"el contexto de oro de GR0: PROMOTE_CTX y AMPERE_B"),
    (b"gpu apagar", b"apagar el GSP en orden antes de reiniciar: que el siguiente arranque encuentre la 3060 limpia"),
    (b"gpu aguante", b"la 3060 bajo carga larga: todos sus trabajos, vuelta tras vuelta, con la temperatura; se para en el primer fallo"),
    (b"gpu video", b"un video en tu pantalla: la 3060 pasa cada fotograma NV12 a color y lo agranda, la CPU solo lee el disco"),
    (b"gpu pantalla", b"la 3060 toma tu pantalla ENTERA: un fractal que se acerca y vuelve, cada pixel escrito por la GPU donde mira el monitor"),
    (b"gpu giro", b"una esfera que gira y bota, con luz y sombra: 32 fotogramas dibujados por la 3060, y en movimiento"),
    (b"gpu color", b"tres colores mezclados por el rasterizador de la 3060: el triangulo de T0 por el pipeline 3D"),
    (b"gpu raster", b"el triangulo por el rasterizador de la 3060: programas de vertice y de pixel, el pipeline 3D de verdad"),
    (b"gpu escena", b"una escena 3D con luz dibujada por la 3060: esfera, brillo, suelo y sombra"),
    (b"gpu 3d", b"la clase 3D de la 3060 escribe pixeles con su ROP: el primer paso del pipeline 3D"),
    (b"gpu triangulo", b"el primer triangulo de la 3060, a pantalla completa: tres aristas y tres colores"),
    (b"gpu fractal", b"el panel de la 3060 a pantalla completa: un fractal de 512x512, y cuanto mas rapida es que la CPU"),
    (b"gpu blur", b"la 3060 desenfoca un trozo de tu pantalla: el antes y el despues, arriba a la derecha"),
    (b"gpu lienzo", b"la 3060 pinta 128x128 pixeles en la RAM del PC y se ven en tu pantalla"),
    (b"gpu sombreo", b"el primer sombreador de BMO-X en la 3060: 32 hilos que escriben cada uno lo suyo"),
    (b"gpu computo", b"el primer trabajo del motor grafico: computo, ficha y un semaforo que paga el GR"),
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
    (b"buscar", b"buscar en la salida (Ctrl+F); cada Enter, la anterior"),
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

/// Cuantas candidatas caben: la lista entera.
const TODAS: usize = 64;

/// Las que empiezan por lo tecleado, TODAS: sus indices en la lista.
#[derive(Clone, Copy)]
pub(crate) struct Sugeridas {
    pub i: [usize; TODAS],
    pub total: usize,
}

impl Sugeridas {
    /// La posicion de `linea` entre las candidatas, si es una de ellas.
    pub(crate) fn donde(&self, linea: &[u8]) -> Option<usize> {
        self.i[..self.total].iter().position(|&k| LISTA[k].0 == linea)
    }
}

fn sin_espacios(t: &[u8]) -> &[u8] {
    let k = t.iter().take_while(|&&c| c == b' ').count();
    &t[k..]
}

/// **Las sugerencias para lo tecleado.** Vacio no sugiere nada; lo que ya es
/// una orden entera se sugiere igual si hay otras mas largas (`save` -> `save
/// mode`), pero no sola: repetirle a alguien lo que acaba de escribir no ayuda.
pub(crate) fn para(tecleado: &[u8]) -> Sugeridas {
    let mut s = Sugeridas { i: [0; TODAS], total: 0 };
    let t = sin_espacios(tecleado);
    if t.is_empty() {
        return s;
    }
    for (k, (linea, _)) in LISTA.iter().enumerate() {
        if s.total < TODAS && empieza(linea, t) && viva(linea) {
            s.i[s.total] = k;
            s.total += 1;
        }
    }
    if s.total == 1 && LISTA[s.i[0]].0.len() == t.len() {
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

/// **TAB sobre una orden** (24-09, *"la tab no aplica las sugerencias"*):
/// escribe ENTERA la sugerencia resaltada, y cada TAB siguiente pasa a la
/// otra (vuelve a la primera al acabar), como fish o zsh. `base` es lo que
/// habia tecleado antes del primer TAB; `path[..n]`, lo que hay ahora.
/// `Some(nueva n)` si escribio; `None` si no hay orden y el TAB es de rutas.
///
/// Antes completaba solo hasta donde TODAS coincidian: con `gp`, `gpu`, y ahi
/// se quedaba -- la sugerencia pintada en el acento no llegaba nunca.
pub(crate) fn tab(path: &mut [u8], n: usize, base: &[u8]) -> Option<usize> {
    mover(path, n, base, true)
}

/// **Moverse por las sugerencias** (24-09: *"con flecha izquierda y
/// derecha"*): `adelante` la siguiente, si no la anterior, dando la vuelta.
/// Sin ninguna elegida aun, la primera (o la ultima hacia atras).
pub(crate) fn mover(path: &mut [u8], n: usize, base: &[u8], adelante: bool) -> Option<usize> {
    let s = para(base);
    if s.total == 0 {
        return None;
    }
    let k = match (s.donde(&path[..n]), adelante) {
        (Some(k), true) => (k + 1) % s.total,
        (Some(k), false) => (k + s.total - 1) % s.total,
        (None, true) => 0,
        (None, false) => s.total - 1,
    };
    escribir(path, s.i[k])
}

/// **Escribir la sugerencia `i`** (su indice en la lista) en el campo, entera.
/// Lo usa tambien el CLIC sobre la linea de sugerencias.
pub(crate) fn escribir(path: &mut [u8], i: usize) -> Option<usize> {
    let l = LISTA.get(i)?.0;
    if l.len() > path.len() {
        return None;
    }
    path[..l.len()].copy_from_slice(l);
    Some(l.len())
}
