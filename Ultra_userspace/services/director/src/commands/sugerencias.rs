//! **LAS SUGERENCIAS DE LA CAJA** -- mientras se teclea, la linea de estado
//! dice que PALABRA puede ir ahora, en cualquier punto de la orden, y TAB,
//! las flechas o un clic la escriben; Ctrl+clic la escribe y la CORRE.
//!
//! # Palabra a palabra (28-09)
//!
//! Peticion del propietario: *"las sugerencias tienen que aparecer en
//! global"* y *"CONTROL + Click ... para ahorrar esfuerzo de escritura"*.
//! Antes se sugerian ORDENES ENTERAS de la lista, y solo las que estaban en
//! ella: `gpu verrano bmox12` no salia nunca, y `bmx12` corrio V0 callado.
//! Ahora se sugiere la palabra SIGUIENTE:
//!
//! ```text
//!    gp            gpu
//!    gpu           init  estatica  objetos  salud ...      (sus subordenes)
//!    gpu verrano   banco  0  30  60  bmox12  ligero ...    (sus opciones)
//! ```
//!
//! Las subordenes salen de la lista de abajo; las OPCIONES que se combinan
//! libres (`gpu verrano bmox12 30`), de quien las entiende
//! (`gspcubo::palabras_verrano`): la misma lista con la que las lee, asi que
//! no hay dos catalogos que se separen.
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

/// Cuantas se muestran a la vez (las que caben en la linea, hasta estas).
pub(crate) const MAX: usize = 8;

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
    (b"gpu cubo", b"el cubo del estudio D3D, por la CPU, igual bit a bit que la 3060 bajo Windows"),
    (b"gpu cubo 3060", b"X5: el mismo cubo dibujado por la 3060 SIN Windows, y su huella contra D3D12"),
    (b"gpu verrano bmox12", b"E5: la 3060 transforma e ilumina con los programas de BMOX-12 traducidos por PROTON-X (0, 30 o 60)"),
    (b"gpu verrano", b"VERRANO V0: el cubo por la API de BMO-X, en la 3060 (programas del BSF) y en la CPU, comparados"),
    (b"gpu verrano banco", b"VERRANO V1: el cubo GIRANDO, 360 fotogramas seguidos por la 3060, con sus fps y el 30 juzgado contra D3D12"),
    (b"gpu verrano banco inti", b"VERRANO: los vertices los cuenta una app de INTI (run inti/cubo.ibx antes) y los publica en una lamina; la 3060 los dibuja sin esperarla"),
    (b"gpu verrano banco ligero", b"VERRANO V1: lo mismo SIN la escalera de diagnostico (2 esperas en vez de ~30 por fotograma): los fps de verdad"),
    (b"gpu verrano banco anillo", b"VERRANO V1b: la CPU ORQUESTA -- envia el fotograma siguiente mientras la tarjeta dibuja este, vertices en RAM, sin esperar"),
    (b"gpu verrano banco coopera", b"VERRANO V1c: CPU y tarjeta COOPERAN -- la CPU sabe donde esta el cubo y la tarjeta solo limpia eso, no la ventana entera"),
    (b"gpu verrano banco maximo", b"VERRANO V1c: TODO -- coopera, y la CPU le EXIGE a la tarjeta sus relojes al maximo antes de empezar (PERF_BOOST)"),
    (b"gpu pase", b"el pase de la GPU: el lienzo prestado una vez y el buzon, abierto y cerrado"),
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
    (b"gpu reintentar", b"tras un 0x15: deshacer el booter y volver a subir, SIN reiniciar"),
    (b"iommu", b"la frontera del DMA de todo aparato"),
    (b"metiche", b"lo que el hardware apunto solo: los errores del bus, preguntados a todos"),
    (b"info", b"RAM, CPU, tareas y disco"),
    (b"consumo", b"nucleos, MHz, vatios y RAM en tabla"),
    (b"cpu", b"el procesador y su reloj"),
    (b"mem", b"la memoria"),
    (b"ram prueba", b"BMO-X prueba su propia RAM libre: escribe, vacia la cache y relee"),
    (b"apps", b"que programa tiene RAM pedida"),
    (b"disco", b"el aparato, lo que queda y lo devuelto"),
    (b"ls", b"que hay en el disco"),
    (b"lee", b"que hay DENTRO de un fichero"),
    (b"personal ls", b"que hay en tu disco Personal (D:), solo para mirar"),
    (b"personal lee", b"la medida y los primeros bytes de un fichero de D:"),
    (b"personal censo", b"que DLL y funciones de Windows pide un .exe de D:, y cuantas tiene PROTON-X"),
    (b"personal diario", b"arranca un .exe de D: con PROTON-X y apunta cada funcion que llama (informe/diario.txt)"),
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
    (b"fraps", b"FRAPS-X: los FPS en una esquina (fraps banco: el banco)"),
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

/// Cuantas candidatas caben (las subordenes de `gpu` son ~50).
const TODAS: usize = 64;
/// Cuantas palabras se miran de una linea.
const PALABRAS: usize = 12;
/// Lo que cabe delante de la palabra (las palabras ya enteras).
const DELANTE: usize = 64;

type Palabra = (&'static [u8], &'static [u8]);

/// Las palabras de `t`, separadas por espacios (las vacias fuera).
fn partir(t: &[u8]) -> ([&[u8]; PALABRAS], usize) {
    let mut w: [&[u8]; PALABRAS] = [b""; PALABRAS];
    let mut n = 0;
    for p in t.split(|&c| c == b' ').filter(|p| !p.is_empty()) {
        if n == PALABRAS {
            break;
        }
        w[n] = p;
        n += 1;
    }
    (w, n)
}

/// **Las candidatas**: la palabra que puede ir tras `completas`, que empiece
/// por `parcial`, y que hace.
#[derive(Clone, Copy)]
pub(crate) struct Sugeridas {
    pal: [Palabra; TODAS],
    pub total: usize,
    /// Lo que se escribe delante de la elegida: las palabras enteras, cada
    /// una con su espacio detras.
    delante: [u8; DELANTE],
    delante_n: usize,
}

impl Sugeridas {
    fn vacia() -> Self {
        Sugeridas { pal: [(b"", b""); TODAS], total: 0, delante: [0; DELANTE], delante_n: 0 }
    }

    fn poner(&mut self, w: &'static [u8], que: &'static [u8], exacta: bool) {
        if let Some(k) = self.pal[..self.total].iter().position(|p| p.0 == w) {
            // La descripcion de la orden que ACABA en esa palabra gana a la de
            // una mas larga que solo pasa por ella.
            if exacta {
                self.pal[k].1 = que;
            }
            return;
        }
        if self.total < TODAS {
            self.pal[self.total] = (w, que);
            self.total += 1;
        }
    }

    pub(crate) fn palabra(&self, k: usize) -> &'static [u8] {
        self.pal[k].0
    }

    pub(crate) fn que(&self, k: usize) -> &'static [u8] {
        self.pal[k].1
    }

    /// La linea entera que escribe la candidata `k` en `out`; cuanto mide.
    fn linea(&self, k: usize, out: &mut [u8]) -> Option<usize> {
        let w = self.pal.get(k)?.0;
        let n = self.delante_n + w.len();
        if k >= self.total || n > out.len() {
            return None;
        }
        out[..self.delante_n].copy_from_slice(&self.delante[..self.delante_n]);
        out[self.delante_n..n].copy_from_slice(w);
        Some(n)
    }

    /// La posicion de `linea` entre las candidatas, si es una de ellas.
    pub(crate) fn donde(&self, linea: &[u8]) -> Option<usize> {
        let l = linea.trim_ascii_end();
        let mut b = [0u8; DELANTE + 32];
        (0..self.total).find(|&k| self.linea(k, &mut b).is_some_and(|n| n == l.len() && empieza(&b[..n], l)))
    }
}

/// Las que siguen a `completas` y empiezan por `parcial`.
fn siguientes(completas: &[&[u8]], parcial: &[u8]) -> Sugeridas {
    let mut s = Sugeridas::vacia();
    let k = completas.len();
    // Las subordenes: la palabra k de cada orden de la lista cuyas k
    // primeras son las tecleadas.
    for &(linea, que) in LISTA {
        let (w, n) = partir(linea);
        if n > k && (0..k).all(|i| w[i].eq_ignore_ascii_case(completas[i])) && empieza(w[k], parcial) && viva(linea) {
            // `w[k]` es un trozo de `linea`, que es 'static.
            s.poner(w[k], que, n == k + 1);
        }
    }
    // Y las opciones libres de quien las tenga, que no esten ya puestas.
    if k >= 2 && completas[0].eq_ignore_ascii_case(b"gpu") && completas[1].eq_ignore_ascii_case(b"verrano") {
        for lista in super::gspcubo::palabras_verrano(&completas[2..]) {
            for &(w, que) in lista {
                if empieza(w, parcial) && !completas[2..].iter().any(|c| c.eq_ignore_ascii_case(w)) && super::gspcubo::vale_verrano(&completas[2..], w) {
                    s.poner(w, que, true);
                }
            }
        }
    }
    s
}

fn con_delante(mut s: Sugeridas, completas: &[&[u8]]) -> Sugeridas {
    let mut n = 0;
    for w in completas {
        if n + w.len() + 1 > DELANTE {
            s.total = 0;
            return s;
        }
        for &c in w.iter() {
            s.delante[n] = baja(c);
            n += 1;
        }
        s.delante[n] = b' ';
        n += 1;
    }
    s.delante_n = n;
    s
}

/// **Las sugerencias para lo tecleado.** Vacio no sugiere nada. Si la ultima
/// palabra ya es entera y es la unica que encaja (`gpu verrano`), se sugiere
/// lo que puede ir DETRAS: repetirle a alguien lo que acaba de escribir no
/// ayuda.
pub(crate) fn para(tecleado: &[u8]) -> Sugeridas {
    let t = tecleado.trim_ascii_start();
    let (w, n) = partir(t);
    if n == 0 {
        return Sugeridas::vacia();
    }
    let abierta = !t.ends_with(b" ");
    let (k, parcial): (usize, &[u8]) = if abierta { (n - 1, w[n - 1]) } else { (n, b"") };
    let s = siguientes(&w[..k], parcial);
    if abierta && s.total == 1 && s.pal[0].0.eq_ignore_ascii_case(parcial) {
        return con_delante(siguientes(&w[..n], b""), &w[..n]);
    }
    con_delante(s, &w[..k])
}

/// **TAB sobre una orden** (24-09, *"la tab no aplica las sugerencias"*):
/// escribe la sugerencia resaltada, y cada TAB siguiente pasa a la otra
/// (vuelve a la primera al acabar), como fish o zsh. `base` es lo que habia
/// tecleado antes del primer TAB; `path[..n]`, lo que hay ahora. `Some(nueva
/// n)` si escribio; `None` si no hay palabra y el TAB es de rutas.
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
    s.linea(k, path)
}

/// **Escribir la candidata `k`** de las sugerencias para `base`, con lo que
/// va delante. Lo usa el CLIC sobre la linea de sugerencias.
pub(crate) fn escribir(path: &mut [u8], base: &[u8], k: usize) -> Option<usize> {
    para(base).linea(k, path)
}
