//! **EL LANZADOR: dar clic y ya.**
//!
//! [consumo] LATE      solo los ~280 ms en que brota la marca de un icono
//!                     (`scene::vida`); quieto, NADA: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! Una rejilla de iconos en el escritorio, uno por cada `.bex` que haya en
//! `apps\`. Se pulsa y el programa arranca.
//!
//! ## Por que esto NO es un acceso directo
//!
//! En Windows el icono del escritorio es un `.lnk`: **un fichero aparte que
//! APUNTA** al programa. En Linux un `.desktop` con el nombre, el icono y el
//! comando escritos dentro. Los dos son punteros, y los dos se rompen igual --
//! mueves el programa y te queda un icono que dice "no se encuentra el
//! destino".
//!
//! Aqui no hay nada que apuntar. **El icono vive DENTRO del `.bex`**, como un
//! recurso mas de su paquete (`SectionKind::Resources`, ver
//! `bmo_abi::bef::resources`). La app trae su propia cara: no hay `.lnk` que se
//! despegue, ni cache de iconos que reconstruir, ni un fichero de escritorio
//! que quede huerfano. Copias el `.bex` y va con su icono; lo borras y no queda
//! rastro que limpiar.
//!
//! Lo mas cerca que hay de esto fuera es el *bundle* de macOS --un `.app` es una
//! carpeta que el Finder muestra como un solo objeto-- y aqui esta un paso mas
//! alla: no es una carpeta, es **un fichero**.
//!
//! ## Y si la app no trae icono
//!
//! Se le dibuja uno: un cuadro de color con su inicial. El color sale del
//! propio nombre, asi que es **estable** --la misma app tiene siempre el mismo
//! color-- y distinto entre apps sin que nadie los asigne.
//!
//! No es un parche mientras llegan los iconos de verdad: es lo correcto. Un
//! escritorio donde una app sin icono aparece como un cuadro vacio obliga a
//! todo el mundo a dibujar antes de poder lanzar nada.
//!
//! ## El formato `BICO`, y por que es tan chico
//!
//! ```text
//!   0..4   "BICO"
//!   4..6   ancho  (u16)
//!   6..8   alto   (u16)
//!   8..    ancho*alto pixeles BGRA, u32 little-endian
//! ```
//!
//! **16x16, y se pinta al doble.** Un icono de 32x32 en crudo son 4 KiB por
//! app; doce apps son 48 KiB **residentes en el compositor** para algo que se
//! mira de reojo. A 16x16 son 1 KiB por app y 12 KiB en total.
//!
//! Y el numero importa mas de lo que parece: la gracia de meter el icono en el
//! paquete es que **no cueste nada llevarlo**. Un icono que engorda la app es
//! un icono que alguien acabara quitando.
//!
//! ## Y desde el 15-10, tambien 32x32 con su ALFA ENTERO (IC2)
//!
//! El propietario: *"que el icono en escritorio se vea elegante [...] con
//! imagen png o jpg o svg"*. Agrandar 16 al doble son cuadros de 2x2; lo
//! elegante es guardarlo a la medida a la que se pinta (32) y con el borde
//! SUAVE: el canal alto ya no es un bit sino 256 tonos, y cada pixel se
//! MEZCLA con lo que hay debajo (el fondo del escritorio, que `background_at`
//! sabe calcular sin leer el framebuffer, o la pastilla de la marca). Lo
//! fabrica `bmo-icono` al construir, desde un SVG, un PNG o un JPG. Son 4 KiB
//! por app y viven aparte (`CARAS`, en `.bss`): la `App` sigue chica y no
//! pasa por la pila. El de 16 se sigue entendiendo, como siempre.
//!
//! ## Una app tambien es una CARPETA (KA1, 15-10)
//!
//! `apps/ecce/` con su `nucleo.bex` (el que arranca) y sus piezas: es UNA app
//! y UN icono, el de su nucleo, con el nombre de la carpeta. Ver
//! `docs/plan/PLAN_EL_KERNEL_DE_LA_APP.md`.

use bmo_userland as bmo;

use super::double_click::DoubleClick;
use super::vida::{self, Paso, REBOTE};
use super::{acento, fino};

/// Cuantas apps caben en el escritorio. Doce es lo que entra en una fila y
/// media a 1080p; pasado eso hace falta una rejilla con scroll, y eso es otra
/// conversacion.
pub const MAX_APPS: usize = 12;
/// Lado del icono TAL COMO SE GUARDA.
pub const ICON_SIDE: u32 = 16;
/// A cuanto se pinta. Ver la cabecera: se guarda chico y se agranda.
pub const SCALE: u32 = 2;
const ICON_PX: u32 = ICON_SIDE * SCALE;

/// La celda de cada app: el icono arriba, el nombre debajo.
const CELL_W: u32 = 104;
const CELL_H: u32 = 72;
/// Donde empieza la rejilla. Debajo de la barra de titulo, con aire.
const GRID_X: u32 = 24;

/// Donde empieza la rejilla: a la derecha de la barra lateral, si esta a la
/// vista (HUD 3). Un solo sitio, porque la rejilla se mide en cuatro.
fn grid_x() -> u32 {
    GRID_X + super::lateral::margen()
}
/// Desde arriba: la barra que habia encima se fundio en el panel (HUD 5).
const GRID_Y: u32 = 24;

const PIXELS: usize = (ICON_SIDE * ICON_SIDE) as usize;
/// El lado de un icono GRANDE: el mismo al que se pinta, uno a uno.
pub const ICON_GRANDE: u32 = ICON_PX;
const PIXELS_GRANDE: usize = (ICON_GRANDE * ICON_GRANDE) as usize;

/// **Las caras grandes**, una por app y en el mismo orden que `apps`. En
/// `.bss` y no dentro de `App`: doce de 4 KiB son 48 KiB, y `Launcher::new`
/// los devolveria por la pila de 64 KiB del director si un dia no le llegara
/// el `sret` (ver su cabecera).
static mut CARAS: [[u32; PIXELS_GRANDE]; MAX_APPS] = [[0; PIXELS_GRANDE]; MAX_APPS];

/// Lo que se sabe de la cara de una app.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cara {
    Ninguna,
    /// 16x16 en `App::pixeles`, alfa de un bit, pintada al doble.
    Chica,
    /// 32x32 en `CARAS`, alfa entero, mezclada.
    Grande,
}

/// El nucleo de una app CARPETA: `apps/<carpeta>/nucleo.bex`.
pub const NUCLEO: &[u8] = b"nucleo.bex";

/// Una app encontrada en `apps\`.
pub struct App {
    /// `apps/doom.bex`, que es lo que se le pasa a `ejecutar`.
    path: [u8; 24],
    path_len: usize,
    /// `doom.bex`, para pintar debajo.
    name: [u8; 12],
    name_len: usize,
    /// Los pixeles del icono, si el paquete traia uno de 16.
    pixeles: [u32; PIXELS],
    cara: Cara,
}

impl App {
    const EMPTY: Self = Self {
        path: [0; 24],
        path_len: 0,
        name: [0; 12],
        name_len: 0,
        pixeles: [0; PIXELS],
        cara: Cara::Ninguna,
    };

    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }

    pub fn path(&self) -> &[u8] {
        &self.path[..self.path_len]
    }
}

pub struct Launcher {
    apps: [App; MAX_APPS],
    count: usize,
    /// El icono MARCADO, si hay alguno.
    ///
    /// ** Antes no existia porque un clic LANZABA, y lo que se lanza no hace
    /// falta senalarlo. Pero un escritorio en el que pulsar un icono arranca un
    /// programa al primer toque no deja mirar sin ejecutar -- y es la unica
    /// rejilla de la casa donde pulsar no se podia deshacer.
    sel: Option<usize>,
    /// La mitad abierta de un doble clic. Vive en `scene::double_click` porque
    /// la rejilla de ESTRATOS hace exactamente lo mismo, y porque hasta hoy las
    /// dos lo contaban en VUELTAS DEL BUCLE -- que es por lo que un icono se
    /// podia marcar y no se abria nunca.
    doble: DoubleClick,
}

// El realce de la celda marcada: un relleno tenue, no un marco -- un borde de
// un pixel alrededor de un icono transparente se lee como suciedad. Desde el
// 04-10 es la `.marca` de `tema.maqueta`, redonda (ver `paint_una`).

impl Launcher {
    /// Recorre `apps\`, se queda con los `.bex` y le saca el icono a cada uno.
    ///
    /// Se hace UNA VEZ, al arrancar el escritorio: son varias lecturas de disco
    /// por app y ninguna cambia mientras la maquina esta encendida. Un
    /// escritorio que releyera el directorio en cada fotograma seria un
    /// escritorio que hace E/S sesenta veces por segundo para mostrar lo mismo.
    // [!] `#[inline(never)]` NO es por medida de codigo: es por PILA. Inlineado,
    // el struct se construye en una ranura del marco del llamante y se copia
    // despues; como llamada aparte, LLVM le pasa la direccion de destino como
    // puntero de retorno (`sret`) y escribe directamente en `.bss`. Medido en
    // el Ryzen el 2026-08-14 -- ver la cabecera de `desktop::install`.
    #[inline(never)]
    pub fn new() -> Self {
        let mut me = Self {
            apps: [const { App::EMPTY }; MAX_APPS],
            count: 0,
            sel: None,
            doble: DoubleClick::new(),
        };
        let Ok(dir) = bmo::Directorio::open(b"apps") else {
            // No hay `apps\`: no es un fallo, es un disco sin aplicaciones.
            return me;
        };
        while me.count < MAX_APPS {
            let Some(e) = dir.next() else { break };
            let mut name = [0u8; 12];
            let n = e.legible(&mut name);
            let i = me.count;
            let app = &mut me.apps[i];
            if e.es_dir {
                // ** UNA APP CARPETA (KA1): la carpeta es la app si trae su
                // `nucleo.bex`; si no, es una carpeta cualquiera y no sale.
                // `apps/` + 8 + `/` + `nucleo.bex` = 24: cabe justo (8.3).
                if n == 0 || n > 8 || n == 1 && name[0] == b'.' || n == 2 && &name[..2] == b".." {
                    continue;
                }
                let mut ruta = [0u8; 24];
                ruta[..5].copy_from_slice(b"apps/");
                ruta[5..5 + n].copy_from_slice(&name[..n]);
                ruta[5 + n] = b'/';
                ruta[6 + n..6 + n + NUCLEO.len()].copy_from_slice(NUCLEO);
                let largo = 6 + n + NUCLEO.len();
                if bmo::Archivo::leer_de(&ruta[..largo]).is_err() {
                    continue;
                }
                app.path = ruta;
                app.path_len = largo;
            } else {
                if n < 5 || !ends_in_bex(&name[..n]) {
                    continue;
                }
                // `apps/` + el nombre. Cabe siempre: 5 + 12 = 17 de 24.
                app.path[..5].copy_from_slice(b"apps/");
                app.path[5..5 + n].copy_from_slice(&name[..n]);
                app.path_len = 5 + n;
            }
            app.name[..n].copy_from_slice(&name[..n]);
            app.name_len = n;
            // SAFETY: un solo hilo arma la rejilla, una vez, al arrancar; la
            // cara `i` es solo de la app `i`.
            let grande = unsafe { &mut *core::ptr::addr_of_mut!(CARAS[i]) };
            app.cara = read_icon(&app.path[..app.path_len], &mut app.pixeles, grande);
            me.count += 1;
        }
        me
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn app(&self, i: usize) -> Option<&App> {
        if i < self.count {
            Some(&self.apps[i])
        } else {
            None
        }
    }

    /// **Un clic en el icono `i`.** `true` si es el SEGUNDO de un doble clic.
    ///
    /// Es la misma regla que la rejilla de ESTRATOS, y desde el 2026-08-23 es
    /// literalmente el mismo codigo --`scene::double_click`-- y no dos copias
    /// que se parecen: el primero MARCA, el segundo ABRE. Dos rejillas con dos
    /// costumbres distintas en el mismo escritorio serian dos cosas que
    /// aprender donde deberia haber una.
    pub fn clic(&mut self, i: usize) -> bool {
        self.sel = Some(i);
        self.doble.hit(i)
    }

    /// Quita el realce. Lo llama quien pulsa FUERA de la rejilla.
    pub fn soltar(&mut self) -> bool {
        let habia = self.sel.is_some();
        self.sel = None;
        self.doble.clear();
        habia
    }

    /// El rectangulo de la celda `i`, para borrarla o realzarla.
    pub fn celda(&self, p: &bmo::Pantalla, i: usize) -> (u32, u32, u32, u32) {
        let (cx, cy) = self.cell(p, i);
        (cx, cy, CELL_W, CELL_H)
    }

    /// Que app cae bajo `(x, y)`, si es que hay alguna.
    ///
    /// El area sensible es **la celda entera**, no solo los pixeles del icono:
    /// apuntar a un cuadro de 32x32 con un raton es mas dificil de lo que
    /// parece, y el nombre de debajo forma parte de lo que uno cree estar
    /// pulsando.
    pub fn app_at(&self, p: &bmo::Pantalla, x: u32, y: u32) -> Option<usize> {
        let per_row = self.per_row(p);
        if per_row == 0 || y < GRID_Y || x < grid_x() {
            return None;
        }
        let col = (x - grid_x()) / CELL_W;
        let row = (y - GRID_Y) / CELL_H;
        if col >= per_row {
            return None;
        }
        let i = (row * per_row + col) as usize;
        if i < self.count {
            Some(i)
        } else {
            None
        }
    }

    fn per_row(&self, p: &bmo::Pantalla) -> u32 {
        if p.ancho <= grid_x() + GRID_X {
            return 0;
        }
        ((p.ancho - grid_x() - GRID_X) / CELL_W).max(1)
    }

    fn cell(&self, p: &bmo::Pantalla, i: usize) -> (u32, u32) {
        let per_row = self.per_row(p).max(1);
        let col = (i as u32) % per_row;
        let row = (i as u32) / per_row;
        (grid_x() + col * CELL_W, GRID_Y + row * CELL_H)
    }
}

/// Pinta la rejilla entera. Va DESPUES del fondo y antes de las ventanas.
pub fn paint(p: &bmo::Pantalla, l: &Launcher) {
    for i in 0..l.count {
        paint_una(p, l, i);
    }
}

/// **Pinta UNA celda**: su realce si esta marcada, el icono y el nombre.
///
/// Existe aparte para que cambiar de icono marcado no cueste repintar los
/// doce. Con tres apps la diferencia no se nota; con doce y un fondo en
/// degradado, si -- y la regla no deberia depender de cuantas haya.
///
/// ** El que la llama para BORRAR un realce tiene que devolver el fondo antes
/// (`scene::erase_window` sobre [`Launcher::celda`]): aqui no se pinta fondo,
/// porque el fondo del escritorio es un degradado y esto no lo sabe calcular.
pub fn paint_una(p: &bmo::Pantalla, l: &Launcher, i: usize) {
    if i >= l.count {
        return;
    }
    let (cx, cy) = l.cell(p, i);
    let app = &l.apps[i];
    // ** LA CELDA SE DEVUELVE A SI MISMA (04-10). La marca redonda y la letra
    // de la casa MEZCLAN su borde con lo que hay debajo, y esta funcion se
    // llama encima de si misma (`uncover` repinta la rejilla entera a cada
    // borrado): sin esto, cada vuelta oscureceria los bordes hasta dejarlos
    // en escalera. El fondo es el del escritorio (`background_at`, foto o
    // degradado): lo que haya encima --una ventana-- lo devuelve el cierre
    // del fotograma, como ya hacia con los iconos.
    let (y0, alto) = (cy.saturating_sub(4), CELL_H);
    p.marcar(cx, y0, CELL_W, alto);
    for fy in y0..y0 + alto {
        for fx in cx..cx + CELL_W {
            p.punto_ya_marcado(fx, fy, super::background_at(fx, fy, p.alto));
        }
    }
    let marcada = l.sel == Some(i);
    if marcada {
        let (x, y, w, h) = ((cx + 4) as i32, y0 as i32, (CELL_W - 8) as i32, (CELL_H - 6) as i32);
        // La pastilla BROTA desde el centro de la celda con rebote: del 60 %
        // a su medida, pasandose un pelo. El acento, cuando ya ha llegado.
        let k = brota(i);
        let (bw, bh) = (vida::entre(w * 6 / 10, w, k).min(CELL_W as i32 - 2), vida::entre(h * 6 / 10, h, k).min(h));
        // (Se pasa solo a lo ancho: a lo alto ya llena la celda, y lo que
        // saliera de ella no lo devolveria nadie.)
        p.caja_redonda(x + (w - bw) / 2, y + (h - bh) / 2, bw, bh, 12, super::tema_gen::MARCA_FONDO);
        // Y el acento, como una raya corta bajo el nombre: dice "esta" sin
        // enmarcar nada.
        p.caja_redonda(x + w / 2 - 9, y + h - 5, 18, 2, 1, acento());
    }
    // El icono, centrado en la celda.
    let ix = cx + (CELL_W - ICON_PX) / 2;
    match app.cara {
        Cara::Chica => paint_pixels(p, ix, cy, &app.pixeles),
        Cara::Grande => {
            // Lo que hay DEBAJO de cada pixel: la pastilla de la marca si cae
            // dentro (el icono va centrado, lejos de sus esquinas redondas), y
            // si no el fondo del escritorio.
            let pastilla = if marcada {
                let (x, y, w, h) = ((cx + 4) as i32, y0 as i32, (CELL_W - 8) as i32, (CELL_H - 6) as i32);
                let k = brota(i);
                let (bw, bh) = (vida::entre(w * 6 / 10, w, k).min(CELL_W as i32 - 2), vida::entre(h * 6 / 10, h, k).min(h));
                Some((x + (w - bw) / 2, y + (h - bh) / 2, bw, bh))
            } else {
                None
            };
            let debajo = |x: u32, y: u32| match pastilla {
                Some((px, py, pw, ph)) if (x as i32) >= px && (x as i32) < px + pw && (y as i32) >= py && (y as i32) < py + ph => {
                    super::tema_gen::MARCA_FONDO
                }
                _ => super::background_at(x, y, p.alto),
            };
            // SAFETY: las caras solo se escriben en `Launcher::new`.
            let cara = unsafe { &*core::ptr::addr_of!(CARAS[i]) };
            paint_grande(p, ix, cy, cara, debajo);
        }
        Cara::Ninguna => paint_default(p, ix, cy, app.name()),
    }
    // El nombre debajo, centrado y SIN el `.bex`: la extension es la misma
    // en todos, asi que ocupa sitio y no distingue nada. Medido con la letra
    // que lo pinta (era `len * 8`); si no cabe, se corta con `..`.
    let estilo = if marcada { NOMBRE_FIRME } else { NOMBRE };
    let mut visible = without_extension(app.name());
    let cabe = CELL_W as i32 - 10;
    let mut corto = [0u8; 40];
    if p.medir(visible, estilo) > cabe {
        let mut n = visible.len().min(corto.len() - 2);
        while n > 1 {
            corto[..n].copy_from_slice(&visible[..n]);
            corto[n] = b'.';
            corto[n + 1] = b'.';
            if p.medir(&corto[..n + 2], estilo) <= cabe {
                break;
            }
            n -= 1;
        }
        visible = &corto[..n + 2];
    }
    let width = p.medir(visible, estilo).max(0) as u32;
    let tx = cx + CELL_W.saturating_sub(width) / 2;
    fino::texto(p, tx, cy + ICON_PX + 4, 22, visible, if marcada { fino::TINTA } else { NOMBRE_TINTA }, estilo);
}

// ** LA VIDA (04-10): marcar un icono no es encender un rectangulo, es que
// la pastilla BROTE desde el centro. Una sola a la vez: la ultima marcada.
const BROTA_MS: u32 = 280;

struct Pop {
    celda: Option<usize>,
    paso: Paso,
    movia: bool,
}

static mut POP: Pop = Pop { celda: None, paso: Paso::QUIETO, movia: false };

fn pop() -> &'static mut Pop {
    // SAFETY: el escritorio es un solo hilo; solo lo toca la rejilla.
    unsafe { &mut *core::ptr::addr_of_mut!(POP) }
}

/// Cuanto ha brotado la pastilla de la celda `i` (milesimas; el rebote pasa
/// de 1000). La primera vez que se pinta una marcada nueva, empieza.
fn brota(i: usize) -> i32 {
    let b = pop();
    if b.celda != Some(i) {
        b.celda = Some(i);
        b.paso = Paso::empezar(BROTA_MS);
        b.movia = true;
    }
    b.paso.k(REBOTE)
}

/// **Un fotograma de vida**: si la pastilla esta brotando, repinta su celda
/// y la devuelve (quien llama apunta que hay que repintar lo de encima).
/// Tambien el fotograma de despues de acabar, que es el quieto.
pub(crate) fn vivir(p: &bmo::Pantalla, l: &Launcher) -> Option<(u32, u32, u32, u32)> {
    let b = pop();
    if l.sel.is_none() {
        // Sin marcada, la proxima que se marque brota otra vez.
        b.celda = None;
        b.movia = false;
        return None;
    }
    let vivo = b.paso.vivo();
    let pide = vivo || b.movia;
    b.movia = vivo;
    let i = b.celda.filter(|&i| pide && l.sel == Some(i))?;
    paint_una(p, l, i);
    let (cx, cy, cw, ch) = l.celda(p, i);
    Some((cx, cy.saturating_sub(4), cw, ch))
}

/// La letra de los nombres de la rejilla.
const NOMBRE: bmo::Estilo = bmo::Estilo::normal(13);
const NOMBRE_FIRME: bmo::Estilo = bmo::Estilo::media(13);
/// Sobre el escritorio el nombre va un punto mas apagado que el marfil: lo
/// que manda en la rejilla son los iconos.
const NOMBRE_TINTA: u32 = 0x00DD_D8D0;

/// **Repinta la rejilla entera devolviendo antes el fondo de cada celda.**
///
/// Es lo que hay que hacer cuando cambia QUIEN esta marcado: el realce viejo
/// solo se va devolviendo el degradado que habia debajo, y eso [`paint_una`] no
/// lo sabe hacer -- ni debe, porque el fondo es del escritorio y no de la
/// rejilla.
///
/// ** Vive aqui y no en el manejador del raton porque hacia falta en DOS
/// sitios --al marcar y al soltar-- y dos copias del mismo bucle es como se
/// acaba teniendo dos que no borran lo mismo.
pub(crate) fn repintar(p: &bmo::Pantalla, c: &super::RunBox, l: &Launcher, visible: bool) {
    for k in 0..l.count() {
        let (cx, cy, cw, ch) = l.celda(p, k);
        super::erase_window(p, c, cx, cy - 4, cw, ch, visible);
        paint_una(p, l, k);
    }
}

/// El area que ocupa la rejilla, para que quien repinte el fondo sepa que
/// tiene que volver a pintar esto encima.
pub fn area(p: &bmo::Pantalla, l: &Launcher) -> (u32, u32, u32, u32) {
    if l.count == 0 {
        return (0, 0, 0, 0);
    }
    let per_row = l.per_row(p).max(1);
    let rows = ((l.count as u32) + per_row - 1) / per_row;
    let cols = (l.count as u32).min(per_row);
    (
        grid_x(),
        GRID_Y,
        cols * CELL_W,
        rows * CELL_H,
    )
}

/// **Un icono grande, mezclado**: cada pixel con su alfa entero sobre lo que
/// `debajo` dice que hay.
fn paint_grande(p: &bmo::Pantalla, x: u32, y: u32, px: &[u32; PIXELS_GRANDE], debajo: impl Fn(u32, u32) -> u32) {
    p.marcar(x, y, ICON_GRANDE, ICON_GRANDE);
    for fy in 0..ICON_GRANDE {
        for fx in 0..ICON_GRANDE {
            let c = px[(fy * ICON_GRANDE + fx) as usize];
            let a = c >> 24;
            if a == 0 {
                continue;
            }
            let (sx, sy) = (x + fx, y + fy);
            let color = if a == 255 { c & 0x00FF_FFFF } else { mezcla(c, debajo(sx, sy), a) };
            p.punto_ya_marcado(sx, sy, color);
        }
    }
}

/// `c` sobre `fondo` con alfa `a` (0..255), canal a canal.
fn mezcla(c: u32, fondo: u32, a: u32) -> u32 {
    let canal = |s: u32| {
        let (v, f) = ((c >> s) & 0xFF, (fondo >> s) & 0xFF);
        ((v * a + f * (255 - a) + 127) / 255) << s
    };
    canal(16) | canal(8) | canal(0)
}

fn paint_pixels(p: &bmo::Pantalla, x: u32, y: u32, px: &[u32; PIXELS]) {
    for fy in 0..ICON_SIDE {
        for fx in 0..ICON_SIDE {
            let c = px[(fy * ICON_SIDE + fx) as usize];
            // ** El canal alto a cero = TRANSPARENTE, y se salta.
            //
            // Sin esto un icono redondo se pinta dentro de su cuadro negro y el
            // escritorio se llena de sellos. Es un test de bit, no un
            // compositor: no hay mezcla, o esta o no esta.
            if c >> 24 == 0 {
                continue;
            }
            p.rect(x + fx * SCALE, y + fy * SCALE, SCALE, SCALE, c & 0x00FF_FFFF);
        }
    }
}

/// El icono de quien no trae icono: un cuadro de color con su inicial.
fn paint_default(p: &bmo::Pantalla, x: u32, y: u32, name: &[u8]) {
    let color = color_de(name);
    // Un cuadro redondo con su inicial: un pelo mas claro arriba, para que
    // no parezca un agujero.
    let (xi, yi, lado) = (x as i32, y as i32, ICON_PX as i32);
    p.pieza(&bmo::Pieza::Degradado { x: xi, y: yi, w: lado, h: lado, r: 8, de: lighten(color), a: color, vertical: true }, 0, 0, None);
    let initial = [upper(name.first().copied().unwrap_or(b'?'))];
    let e = bmo::Estilo::negrita(18);
    let w = p.medir(&initial, e);
    p.letra_en_caja(xi + (lado - w) / 2, yi, lado, &initial, 0x00FF_FFFF, e);
}

/// Un color estable a partir del nombre.
///
/// La misma app tiene siempre el mismo, y dos apps distintas casi nunca el
/// mismo -- que es todo lo que se le pide. Se fija el brillo para que la
/// inicial blanca se lea encima: un hash suelto produce amarillos donde no se
/// ve nada.
pub(crate) fn color_de(name: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in name {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    // Tres canales en 0x40..0xC0: ni tan oscuro que parezca un hueco ni tan
    // claro que se coma el texto de encima.
    let r = 0x40 + (h & 0x7F);
    let g = 0x40 + ((h >> 8) & 0x7F);
    let b = 0x40 + ((h >> 16) & 0x7F);
    (r << 16) | (g << 8) | b
}

fn lighten(c: u32) -> u32 {
    let r = (((c >> 16) & 0xFF) + 0x30).min(0xFF);
    let g = (((c >> 8) & 0xFF) + 0x30).min(0xFF);
    let b = ((c & 0xFF) + 0x30).min(0xFF);
    (r << 16) | (g << 8) | b
}

fn upper(b: u8) -> u8 {
    if b.is_ascii_lowercase() {
        b - 32
    } else {
        b
    }
}

/// **Lanzable**: `.bex`, y tambien `.ibx` (lo que escribe INTI).
///
/// ## Por que son dos y no una
///
/// Son **el mismo formato**: un `.ibx` es un BEF, lo carga el mismo cargador y
/// lo lee el mismo gate. Lo que cambia es a que se ha comprometido quien lo
/// escribio -- declara su perfil, sus piezas y su mesa de katanas, y no llega al
/// disco si esa mesa no cuadra con sus bytes.
///
/// ** Asi que la extension no cambia como se lanza: cambia lo que se puede
/// afirmar de el sin abrirlo. Por eso aqui son dos nombres y una sola rama.
///
/// *** Y ES `.ibx`, NO `.ibex`, desde el 2026-09-13. Cuatro letras de extension
/// no caben en 8.3, y el FAT32 del kernel es 8.3 a proposito (se salta las
/// entradas de nombre largo). Windows guardaba `pulso.ibex` con el nombre corto
/// `PULSO~1.IBE`, y eso es lo UNICO que BMO-X veia: esta funcion no lo
/// reconocia --asi que INTI nunca tuvo icono-- y el TAB completaba `pulso~1.ibe`.
/// Un nombre que el sistema no puede leer no es un nombre: es un parche de
/// Windows. Las dos extensiones tienen tres letras y se comparan igual.
fn ends_in_bex(n: &[u8]) -> bool {
    let l = n.len();
    l >= 4 && (n[l - 4..].eq_ignore_ascii_case(b".ibx") || n[l - 4..].eq_ignore_ascii_case(b".bex"))
}

fn without_extension(n: &[u8]) -> &[u8] {
    match n.iter().rposition(|&b| b == b'.') {
        Some(i) => &n[..i],
        None => n,
    }
}

// -- Leer el icono DE DENTRO del paquete --------------------------------
//
// Se leen los bytes a mano y no con `bmo_abi::bef`, por el mismo motivo que lo
// hace el kernel: **`bmo-abi` es el CONTRATO y aqui se implementa contra el**.
// Dos lectores del mismo formato es lo que obliga a que el formato este escrito
// y no solo implementado.
//
// Los offsets salen de `bef/header.rs`, `bef/sections.rs` y `bef/resources.rs`.
// Si alguno cambia, esto deja de encontrar iconos -- y eso es visible al
// primer arranque, que es lo mejor que le puede pasar a una divergencia.

/// El anexo de RECURSOS de BEF2 (antes era la seccion `0x0B`).
const SECTION_RESOURCES: u8 = 0x04;
/// Una entrada de la tabla de ANEXOS de BEF2 (2026-09-19).
const SPLASH_SECTION: usize = 16;
const HEADER_BRES: usize = 16;
const SPLASH_BRES: usize = 64;

/// Saca el recurso `icono` de un `.bex` y lo descifra como BICO: uno de 16
/// queda en `px`, uno de 32 en `grande`.
fn read_icon(path: &[u8], px: &mut [u32; PIXELS], grande: &mut [u32; PIXELS_GRANDE]) -> Cara {
    let Ok(f) = bmo::Archivo::leer_de(path) else {
        return Cara::Ninguna;
    };
    // -- La cabecera BEF2: cuantos ANEXOS hay. Su tabla va justo detras, en
    // el byte 64, asi que no hay offset que leer ni que creerse.
    let mut cab = [0u8; 64];
    if f.read(&mut cab) < 64 {
        return Cara::Ninguna;
    }
    if &cab[0..4] != b"BEF2" {
        return Cara::Ninguna;
    }
    let lookup = 64u64;
    let count = read_u32(&cab, 20) as usize;
    if count == 0 || count > 16 {
        return Cara::Ninguna;
    }
    // -- Buscar la seccion de recursos --
    let mut sec_off = 0u64;
    let mut sec_len = 0u64;
    for i in 0..count {
        f.saltar(lookup + (i * SPLASH_SECTION) as u64);
        let mut e = [0u8; SPLASH_SECTION];
        if f.read(&mut e) < SPLASH_SECTION {
            return Cara::Ninguna;
        }
        if e[0] == SECTION_RESOURCES {
            sec_off = read_u32(&e, 4) as u64;
            sec_len = read_u32(&e, 8) as u64;
            break;
        }
    }
    if sec_len == 0 {
        return Cara::Ninguna;
    }
    // -- El indice BRES --
    f.saltar(sec_off);
    let mut bres = [0u8; HEADER_BRES];
    if f.read(&mut bres) < HEADER_BRES || &bres[0..4] != b"BRES" {
        return Cara::Ninguna;
    }
    let resources = read_u32(&bres, 4) as usize;
    for i in 0..resources.min(64) {
        f.saltar(sec_off + (HEADER_BRES + i * SPLASH_BRES) as u64);
        let mut e = [0u8; SPLASH_BRES];
        if f.read(&mut e) < SPLASH_BRES {
            return Cara::Ninguna;
        }
        let length = e[16] as usize;
        if length > 47 || &e[17..17 + length] != b"icono" {
            continue;
        }
        // El offset del recurso es RELATIVO A LA SECCION, no al fichero: por eso
        // se suma `sec_off`. Un offset absoluto habria que reescribirlo cada vez
        // que el `.bex` se vuelve a emitir con otra disposicion.
        let datum = sec_off + read_u64(&e, 0);
        let tam = read_u64(&e, 8) as usize;
        return read_bico(&f, datum, tam, px, grande);
    }
    Cara::Ninguna
}

fn read_bico(f: &bmo::Archivo, off: u64, tam: usize, px: &mut [u32; PIXELS], grande: &mut [u32; PIXELS_GRANDE]) -> Cara {
    const CAB: usize = 8;
    f.saltar(off);
    let mut cab = [0u8; CAB];
    if tam < CAB || f.read(&mut cab) < CAB || &cab[0..4] != b"BICO" {
        return Cara::Ninguna;
    }
    // Solo se aceptan las DOS medidas que este escritorio sabe pintar: la de
    // 16 (al doble) y la de 32 (tal cual). Escalar otra es una decision de
    // aspecto que no toca aqui -- la toma `bmo-icono` al construir --, y
    // aceptarla a medias daria iconos deformes sin decir por que.
    let (w, h) = (read_u16(&cab, 4) as u32, read_u16(&cab, 6) as u32);
    let (destino, cara): (&mut [u32], Cara) = match (w, h) {
        (ICON_SIDE, ICON_SIDE) => (&mut px[..], Cara::Chica),
        (ICON_GRANDE, ICON_GRANDE) => (&mut grande[..], Cara::Grande),
        _ => return Cara::Ninguna,
    };
    if tam < CAB + destino.len() * 4 {
        return Cara::Ninguna;
    }
    // Fila a fila (128 B la grande): un bufer de 4 KiB en la pila del
    // director no se gasta para esto.
    let lado = w as usize;
    let mut fila = [0u8; ICON_GRANDE as usize * 4];
    for y in 0..lado {
        let bytes = &mut fila[..lado * 4];
        if f.read(bytes) < bytes.len() {
            return Cara::Ninguna;
        }
        for x in 0..lado {
            destino[y * lado + x] = read_u32(bytes, x * 4);
        }
    }
    cara
}

fn read_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn read_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn read_u64(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}
