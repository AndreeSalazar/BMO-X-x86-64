//! **EL PANEL**: la columna de la izquierda, y ya la UNICA barra del escritorio
//! (HUD 3 y 5, 2026-09-22).
//!
//! [consumo] LATE      4 muestras por segundo con el panel a la vista; ninguna
//!                     escondido o con una app a pantalla completa (L6h)
//!
//! El motivo, uno: **todo lo que no es una ventana, en un solo sitio.**
//!
//! Nacio como la barra lateral en vivo (HUD 3: lo que hace la maquina, sin
//! abrir nada), al lado de la barra de arriba. Esa misma tarde el propietario la
//! retiro: *"la barra de arriba vamos a quitar... ya cumplio su parte"*. Eran dos
//! barras para lo mismo --cpu, memoria y vatios salian en las dos-- y la de
//! arriba se habia llenado de instrumentos de diagnostico (el volcado, la
//! entrada, el pulso con su reparto) que eran de los dias de cazar averias. Se
//! fundio aqui, de arriba abajo:
//!
//! ```text
//!    la marca           los ojos del gato (`= =`, en el acento) y BMO-X
//!    las ventanas       una ficha por ventana, en vertical (eran las de arriba)
//!    .                  (el aire: las fichas crecen hacia abajo)
//!    la luz del bus     el testigo del teclado, siempre encendido
//!    los instrumentos   cpu, memoria, vatios, pulso con su AGUJA y sonido
//!    el reloj y el vol  al pie, como en cualquier barra
//! ```
//!
//! Los instrumentos de diagnostico se fueron a CABINA, que es donde se mira
//! cuando algo va mal (ver `cabina::instrumentos`).
//!
//! # El sitio: una columna RESERVADA
//!
//! Como la `exclusive zone` de una barra de Hyprland: ninguna ventana entra en
//! la columna. `margen()` la dice, y la leen `chrome::area_util` (encajar,
//! maximizar, el mosaico), los topes del arrastre y la rejilla de iconos, que
//! se corre a la derecha. Por eso el panel nunca queda tapado, y su repintado
//! de 4 Hz no puede pintar encima de una ventana.
//!
//! # Escondido (Ctrl+B) queda una TIRA
//!
//! Seis pixeles del color del panel con la luz del bus, y un clic lo trae. No es
//! adorno: la ficha de CABINA estaba SIEMPRE en la barra por una razon escrita
//! --*"un panel de diagnostico al que solo se llega con el aparato que puede
//! estar roto no es un panel de diagnostico"*-- y esconder el panel no puede
//! dejar al raton sin camino a CABINA el dia que el teclado se muera.
//!
//! # Lo que dice `sys/director.cfg`
//!
//! Lo que eran las opciones de la barra de arriba son ahora las del panel:
//! `barra_flotante` (pastilla con huecos o tira pegada al borde), `barra_hueco`,
//! y `cpu`, `memoria`, `vatios` y `reloj` encienden o apagan cada instrumento.

use bmo_userland as bmo;
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, Ordering};

use super::estilo::estilo;
use super::pulso::Lectura;
use super::{acento, inside_rounded, rounded_rect, INK, INK_DIM};
use crate::ventana::Ventana;

/// El ancho del panel: diecisiete letras (`SIN TECLADO USB` y su luz) y el
/// aire. Dentro caben 136 px: la grafica de 68 muestras a 2 px.
pub(crate) const ANCHO: u32 = 160;
/// Lo que queda a la vista con el panel escondido.
pub(crate) const TIRA: u32 = 6;
/// El margen interior, a cada lado.
const DENTRO: u32 = 12;

static VISIBLE: AtomicBool = AtomicBool::new(true);

/// Esta a la vista?
pub(crate) fn visible() -> bool {
    VISIBLE.load(Ordering::Relaxed)
}

/// Ctrl+B, o un clic en la tira: esconderlo o traerlo. Quien llama repinta el
/// escritorio: la rejilla y el area util cambian de sitio.
pub(crate) fn alternar() {
    VISIBLE.store(!visible(), Ordering::Relaxed);
    olvidar();
}

/// El aire entre el panel y el borde de la pantalla: el `barra_hueco` del
/// estilo si flota, y nada si es una tira pegada.
fn hueco() -> u32 {
    let e = estilo();
    if e.barra_flotante {
        e.barra_hueco.min(12)
    } else {
        0
    }
}

/// **Lo que la columna le quita a las ventanas**, por la izquierda: el panel y
/// el hueco que lo separa del borde. Escondido, la tira.
pub(crate) fn margen() -> u32 {
    if visible() {
        hueco() + ANCHO
    } else {
        TIRA
    }
}

/// `(x, y, ancho, alto)` del panel en una pantalla de `alto` filas.
pub(crate) fn caja(alto: u32) -> (u32, u32, u32, u32) {
    let h = hueco();
    (h, h, ANCHO, alto.saturating_sub(2 * h))
}

// ===================================================================
//  El plano: donde va cada cosa. UN sitio, porque lo leen el pintor, el
//  raton, el testigo y el vol
// ===================================================================

const FICHA_H: u32 = 24;
const FILA: u32 = FICHA_H + 4;
const TESTIGO_H: u32 = 20;
const VOL_H: u32 = 24;
const RENGLON: u32 = bmo::GLIFO_ALTO + 4;
const GRAF_ALTO: u32 = 30;
const SECCION: u32 = RENGLON + GRAF_ALTO + 14;
/// Las fichas que caben siempre, antes de quitarle sitio a los instrumentos.
const FICHAS_MIN: u32 = 4;

struct Plano {
    x0: u32,
    iw: u32,
    logo_y: u32,
    fichas_y: u32,
    /// Cuantas filas de fichas caben.
    fichas_max: u32,
    testigo_y: u32,
    graf_y: u32,
    /// Cuantos instrumentos caben (de los encendidos).
    graf_n: u32,
    reloj_y: u32,
    vol_y: u32,
}

fn plano(alto: u32) -> Plano {
    let (bx, by, bw, bh) = caja(alto);
    let x0 = bx + DENTRO;
    let iw = bw - 2 * DENTRO;
    let logo_y = by + 14;
    let fichas_y = by + 52;
    // Del pie hacia arriba: el vol, el reloj y su raya.
    let vol_y = (by + bh).saturating_sub(DENTRO + VOL_H);
    let reloj_y = vol_y.saturating_sub(6 + bmo::GLIFO_ALTO);
    let pie = reloj_y.saturating_sub(12);
    // Los instrumentos, encima del pie; los que no quepan sin dejar a las
    // fichas su minimo se quedan fuera por arriba.
    let suelo = fichas_y + FICHAS_MIN * FILA + 16 + TESTIGO_H + 8;
    let cabe = pie.saturating_sub(8).saturating_sub(suelo) / SECCION;
    let graf_n = activos().1.min(cabe as usize) as u32;
    let graf_y = pie.saturating_sub(8 + graf_n * SECCION);
    let testigo_y = graf_y.saturating_sub(8 + TESTIGO_H);
    let fichas_fin = testigo_y.saturating_sub(16);
    Plano {
        x0,
        iw,
        logo_y,
        fichas_y,
        fichas_max: fichas_fin.saturating_sub(fichas_y) / FILA,
        testigo_y,
        graf_y,
        graf_n,
        reloj_y,
        vol_y,
    }
}

/// **Donde pinta el testigo del bus**: su fila dentro del panel, o su trozo de
/// la tira si el panel esta escondido. Ver `testigo::pintar`.
pub(crate) fn caja_testigo(alto: u32) -> (u32, u32, u32, u32) {
    if !visible() {
        return (0, hueco() + 8, TIRA, 48);
    }
    let pl = plano(alto);
    (pl.x0, pl.testigo_y, pl.iw, TESTIGO_H)
}

/// **Donde pinta el vol**, si el panel esta a la vista. Ver `sound::barra`.
pub(crate) fn caja_vol(alto: u32) -> Option<(u32, u32, u32, u32)> {
    if !visible() {
        return None;
    }
    let pl = plano(alto);
    Some((pl.x0 - 4, pl.vol_y, pl.iw + 8, VOL_H))
}

/// **El color en `(x, y)` si es del panel**: la pastilla, su borde, la marca y
/// la tira. Lo pregunta `scene_color`, que es quien contesta al borrar. Lo
/// escrito encima no se modela: el panel lo repinta entero al darlo por perdido,
/// y ninguna ventana puede taparlo.
pub(crate) fn color_en(x: u32, y: u32, alto: u32) -> Option<u32> {
    let e = estilo();
    if !visible() {
        if x >= TIRA {
            return None;
        }
        let (_, ty, _, th) = caja_testigo(alto);
        if y >= ty && y < ty + th {
            return Some(super::testigo::color());
        }
        return Some(e.barra_fondo);
    }
    let (bx, by, bw, bh) = caja(alto);
    if x < bx || x >= bx + bw || y < by || y >= by + bh {
        return None;
    }
    let pl = plano(alto);
    if ojos_en(x, y, pl.x0, pl.logo_y) {
        return Some(acento());
    }
    if !e.barra_flotante {
        return Some(if x + 1 == bx + bw { e.barra_borde } else { e.barra_fondo });
    }
    if !inside_rounded(x, y, bx, by, bw, bh) {
        return None;
    }
    if !inside_rounded(x, y, bx + 1, by + 1, bw - 2, bh - 2) {
        return Some(e.barra_borde);
    }
    Some(e.barra_fondo)
}

// ===================================================================
//  La marca: los ojos del gato
// ===================================================================

/// **Los ojos del gato del logo de BMO-X**: dos `=`, cada uno de dos rayas de
/// 8 x 2 px, con 5 px entre ojo y ojo. Era un cuadrado del acento; el
/// propietario: *"no olvides los colores... como el gato"*. El gato del logo no
/// tiene mas cara que eso, y por eso se reconoce en 21 pixeles.
///
/// Una funcion para pintarlos y para contestar por su color: si fueran dos
/// geometrias, borrar encima dejaria un ojo tuerto.
const OJO_W: u32 = 8;
const OJO_ENTRE: u32 = 5;
const RAYAS: [u32; 2] = [3, 8];

fn ojos_en(x: u32, y: u32, x0: u32, y0: u32) -> bool {
    if x < x0 || y < y0 {
        return false;
    }
    let (dx, dy) = (x - x0, y - y0);
    let en_ojo = dx < OJO_W || (dx >= OJO_W + OJO_ENTRE && dx < 2 * OJO_W + OJO_ENTRE);
    en_ojo && RAYAS.iter().any(|&r| dy >= r && dy < r + 2)
}

fn pintar_ojos(p: &bmo::Pantalla, x0: u32, y0: u32) {
    for ojo in [x0, x0 + OJO_W + OJO_ENTRE] {
        for r in RAYAS {
            p.rect(ojo, y0 + r, OJO_W, 2, acento());
        }
    }
}

// ===================================================================
//  Las fichas: una por ventana abierta
// ===================================================================

/// Una ficha: la ventana, su nombre y su color, y los dos estados que se leen
/// sin texto (la de delante realzada, la minimizada apagada).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ficha {
    pub(crate) v: Ventana,
    pub(crate) nombre: &'static str,
    pub(crate) color: u32,
    pub(crate) activa: bool,
    pub(crate) minimizada: bool,
}

impl Ficha {
    pub(crate) const VACIA: Ficha = Ficha { v: Ventana::Run, nombre: "", color: 0, activa: false, minimizada: false };
}

/// Ejecutar, ESTRATOS, CABINA y las apps: 3 + `surface::MAX`, y aire.
pub(crate) const MAX_FICHAS: usize = 8;

static mut FICHAS: [Ficha; MAX_FICHAS] = [Ficha::VACIA; MAX_FICHAS];
static mut FICHAS_N: usize = 0;
static mut FICHAS_SUCIAS: bool = true;

/// **La lista de ventanas de ahora.** Si cambio, sus filas se repintan en la
/// vuelta siguiente (y solo ellas: lo demas del panel no se mueve, porque las
/// fichas crecen hacia el aire).
pub(crate) fn fichas(lista: &[Ficha]) {
    let n = lista.len().min(MAX_FICHAS);
    unsafe {
        let viejas = &mut *addr_of_mut!(FICHAS);
        if FICHAS_N == n && viejas[..n] == lista[..n] {
            return;
        }
        viejas[..n].copy_from_slice(&lista[..n]);
        FICHAS_N = n;
        FICHAS_SUCIAS = true;
    }
}

/// **Sobre que ficha esta el puntero**, si sobre alguna. La misma geometria que
/// las pinta.
pub(crate) fn ficha_en(x: u32, y: u32, alto: u32) -> Option<Ventana> {
    if !visible() {
        return None;
    }
    let pl = plano(alto);
    if x + 6 < pl.x0 || x >= pl.x0 + pl.iw + 6 || y < pl.fichas_y {
        return None;
    }
    let fila = (y - pl.fichas_y) / FILA;
    if (y - pl.fichas_y) % FILA >= FICHA_H || fila >= pl.fichas_max {
        return None;
    }
    let (lista, n) = unsafe { (&*addr_of!(FICHAS), FICHAS_N) };
    (fila < n as u32).then(|| lista[fila as usize].v)
}

/// El puntero esta sobre la tira del panel escondido?
pub(crate) fn en_la_tira(x: u32) -> bool {
    !visible() && x < TIRA
}

fn pintar_fichas(p: &bmo::Pantalla, pl: &Plano) {
    let e = estilo();
    let (lista, n) = unsafe { (&*addr_of!(FICHAS), FICHAS_N) };
    let alto = pl.fichas_max * FILA;
    p.rect(pl.x0 - 6, pl.fichas_y, pl.iw + 12, alto, e.barra_fondo);
    for (k, f) in lista[..n.min(pl.fichas_max as usize)].iter().enumerate() {
        let y = pl.fichas_y + k as u32 * FILA;
        // Tres estados y tres aspectos, los mismos de las fichas de arriba: la
        // de delante con su fondo y una raya de su color, la minimizada con el
        // punto apagado.
        if f.activa {
            p.rect(pl.x0 - 6, y, pl.iw + 12, FICHA_H, 0x0018_1433);
            p.rect(pl.x0 - 6, y + 4, 2, FICHA_H - 8, f.color);
        }
        let punto = if f.minimizada { INK_DIM } else { f.color };
        p.rect(pl.x0 + 2, y + (FICHA_H - 8) / 2, 8, 8, punto);
        let tinta = if f.minimizada { INK_DIM } else { INK };
        let cabe = ((pl.iw - 18) / bmo::GLIFO_ANCHO) as usize;
        let t = &f.nombre[..f.nombre.len().min(cabe)];
        p.texto(pl.x0 + 18, y + (FICHA_H - bmo::GLIFO_ALTO) / 2, t, tinta);
    }
}

// ===================================================================
//  Los instrumentos y su historia
// ===================================================================

/// Muestras de historia: a 2 px cada una llenan el ancho de dentro, y a 4 por
/// segundo son diecisiete segundos.
const HISTORIA: usize = ((ANCHO - 2 * DENTRO) / 2) as usize;
const INSTRUMENTOS: usize = 5;
const CPU: usize = 0;
const MEM: usize = 1;
const VATIOS: usize = 2;
const PULSO: usize = 3;
const SONIDO: usize = 4;

const NOMBRES: [&str; INSTRUMENTOS] = ["cpu", "memoria", "vatios", "pulso", "sonido"];

/// Los instrumentos encendidos, en su orden: el `.cfg` apaga cpu, memoria y
/// vatios. El pulso y el sonido no se apagan: el pulso es la prueba de vida del
/// escritorio, y el sonido el unico medidor del maestro a la vista.
fn activos() -> ([usize; INSTRUMENTOS], usize) {
    let e = estilo();
    let mut l = [0usize; INSTRUMENTOS];
    let mut n = 0;
    for (i, on) in [e.cpu, e.memoria, e.vatios, true, true].into_iter().enumerate() {
        if on {
            l[n] = i;
            n += 1;
        }
    }
    (l, n)
}

static mut HIST: [[u32; HISTORIA]; INSTRUMENTOS] = [[0; HISTORIA]; INSTRUMENTOS];
static mut LLENAS: usize = 0;
static mut PROXIMO: u64 = 0;
static mut FORZAR: bool = true;
static mut PREV_TSC: u64 = 0;
static mut PREV_REPOSO: u64 = 0;
/// El minuto que dice el reloj pintado; `u16::MAX` obliga a pintarlo.
static mut MINUTO: u16 = u16::MAX;

/// Lo pintado se da por perdido: el fondo se repinto debajo. La vuelta
/// siguiente pinta el panel entero, y la luz y el vol vuelven a pintarse.
pub(crate) fn olvidar() {
    unsafe { FORZAR = true };
    super::testigo::olvidar();
    super::sound::olvidar_barra();
}

fn empuja(i: usize, v: u32) {
    unsafe {
        let h = &mut (*addr_of_mut!(HIST))[i];
        h.copy_within(1.., 0);
        h[HISTORIA - 1] = v;
    }
}

/// **La vuelta**: si el panel se dio por perdido, se pinta entero; si cambiaron
/// las fichas, sus filas; y cada 250 ms una muestra de cada instrumento. `mw`
/// son los milivatios del paquete y `l` la lectura del pulso.
pub(crate) fn latido(p: &bmo::Pantalla, mw: Option<u64>, l: &Lectura) {
    let forzar = unsafe { FORZAR };
    if !visible() {
        if forzar {
            unsafe { FORZAR = false };
            p.rect(0, 0, TIRA, p.alto, estilo().barra_fondo);
        }
        return;
    }
    let pl = plano(p.alto);
    let e = estilo();
    if forzar {
        let (bx, by, bw, bh) = caja(p.alto);
        if e.barra_flotante {
            rounded_rect(p, bx, by, bw, bh, e.barra_borde);
            rounded_rect(p, bx + 1, by + 1, bw - 2, bh - 2, e.barra_fondo);
        } else {
            p.rect(bx, by, bw, bh, e.barra_fondo);
            p.rect(bx + bw - 1, by, 1, bh, e.barra_borde);
        }
        // La marca, arriba, y su raya: los ojos del gato y el nombre en blanco,
        // como en el logo.
        pintar_ojos(p, pl.x0, pl.logo_y);
        p.texto(pl.x0 + 2 * OJO_W + OJO_ENTRE + 10, pl.logo_y - 2, "BMO-X", INK);
        p.rect(pl.x0, pl.fichas_y - 10, pl.iw, 1, e.barra_borde);
        // Las rayas que separan el bus, los instrumentos y el pie.
        p.rect(pl.x0, pl.testigo_y - 10, pl.iw, 1, e.barra_borde);
        p.rect(pl.x0, pl.reloj_y - 12, pl.iw, 1, e.barra_borde);
        unsafe {
            FICHAS_SUCIAS = true;
            MINUTO = u16::MAX;
        }
    }
    if unsafe { FICHAS_SUCIAS } {
        unsafe { FICHAS_SUCIAS = false };
        pintar_fichas(p, &pl);
    }
    if e.reloj {
        reloj(p, &pl);
    }

    let ahora = bmo::ciclos();
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    if !forzar && ahora < unsafe { PROXIMO } {
        return;
    }
    unsafe {
        PROXIMO = ahora + hz / 4;
        FORZAR = false;
    }

    // -- Las muestras, las mismas cuentas que hacia la barra de arriba --
    let reposo = bmo::info(bmo::INFO_BSP_TICKS_REPOSO);
    let cpu = unsafe {
        let v = if PREV_TSC != 0 && ahora > PREV_TSC && reposo >= PREV_REPOSO {
            let dt = ahora - PREV_TSC;
            let dr = (reposo - PREV_REPOSO).min(dt);
            (100 - dr * 100 / dt) as u32
        } else {
            0
        };
        PREV_TSC = ahora;
        PREV_REPOSO = reposo;
        v
    };
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let usada_mib = (total.saturating_sub(bmo::info(bmo::INFO_RAM_LIBRE)) / (1024 * 1024)) as u32;
    let watios_d = mw.map(|m| (m / 100) as u32).unwrap_or(0); // decimas de W
    // El medidor del maestro: lo mas alto de los dos lados, de -60 a 0 dBFS.
    let med = bmo::info(bmo::INFO_AUDIO_MEDIDOR);
    let lado = |bit: u32| ((med >> bit) & 0xFFFF) as u16 as i16 as i32;
    let pico_db = lado(0).max(lado(16)); // 1/256 dB
    let sonido = ((pico_db + 60 * 256).max(0) * 100 / (60 * 256)).min(100) as u32;
    // ** EL PULSO LLEVA SU AGUJA. Es la leccion de `scene::pulso`: un numero
    // sin aguja se ve igual vivo que muerto, y un cero sin reloj no lo midio
    // nadie. La aguja gira con cada muestra, o sea cuatro veces por segundo.
    let d = super::pulso::dictamen(l);

    if !forzar {
        empuja(CPU, cpu);
        empuja(MEM, usada_mib);
        empuja(VATIOS, watios_d);
        empuja(PULSO, d.ritmo.unwrap_or(0));
        empuja(SONIDO, sonido);
        unsafe { LLENAS = (LLENAS + 1).min(HISTORIA) };
    }

    // -- Pintar --
    let x0 = pl.x0;
    let gw = (HISTORIA as u32) * 2;
    let (lista, _) = activos();
    for (s, &i) in lista[..pl.graf_n as usize].iter().enumerate() {
        let y = pl.graf_y + s as u32 * SECCION;
        let hist = unsafe { &(*addr_of!(HIST))[i] };
        let ultimo = hist[HISTORIA - 1];
        // El renglon: el nombre apagado y la cifra en claro.
        p.rect(x0, y, gw, RENGLON, e.barra_fondo);
        let mut t = [0u8; 12];
        let mut n = 0usize;
        let mut pon = |s: &[u8]| {
            for &b in s {
                if n < t.len() {
                    t[n] = b;
                    n += 1;
                }
            }
        };
        let mut dig = [0u8; 10];
        let mut nombre = NOMBRES[i];
        let mut tinta = INK;
        match i {
            CPU => {
                let k = crate::text::decimal(ultimo as u64, &mut dig);
                pon(&dig[..k]);
                pon(b"%");
            }
            MEM => {
                let k = crate::text::decimal(ultimo as u64, &mut dig);
                pon(&dig[..k]);
                pon(b"M");
            }
            VATIOS => {
                let k = crate::text::decimal((ultimo / 10) as u64, &mut dig);
                pon(&dig[..k]);
                pon(b".");
                pon(&[b'0' + (ultimo % 10) as u8]);
            }
            PULSO => {
                nombre = if d.en_reposo { "reposo" } else if d.en_latido { "latido" } else { "pulso" };
                match d.ritmo {
                    None => pon(b"SIN RELOJ"),
                    Some(v) => {
                        let k = crate::text::decimal(v as u64, &mut dig);
                        pon(&dig[..k]);
                        pon(b"/s");
                    }
                }
                if d.alarma || d.ritmo.is_none() {
                    tinta = 0x00EF_4444;
                }
                pon(b" ");
                pon(&[d.aguja]);
            }
            _ => {
                if pico_db <= -60 * 256 {
                    pon(b"--");
                } else {
                    let db = (-pico_db + 128) / 256;
                    pon(b"-");
                    let k = crate::text::decimal(db as u64, &mut dig);
                    pon(&dig[..k]);
                }
            }
        }
        p.texto(x0, y + 2, nombre, INK_DIM);
        let cx = (x0 + gw).saturating_sub(n as u32 * bmo::GLIFO_ANCHO);
        p.texto_bytes(cx, y + 2, &t[..n], tinta);

        // La grafica: de la mas vieja (izquierda) a la de ahora (derecha).
        let gy = y + RENGLON;
        p.rect(x0, gy, gw, GRAF_ALTO, e.barra_fondo);
        p.rect(x0, gy + GRAF_ALTO - 1, gw, 1, e.barra_borde);
        // ** LA ESCALA, POR LO QUE EL NUMERO ES (23-09).
        //
        // Antes, todo lo que no era un % se dibujaba en barras desde cero con
        // techo "la mayor de la ventana, con aire". Con un numero que no se
        // mueve eso es una barra al 80 %: el Ryzen pintaba `memoria 31M` casi
        // llena con 14,8 GiB libres, y `vatios 57.0` igual. Una barra DICE
        // cuanto es de su techo, y aquel techo no era de nada.
        //
        //   con techo de verdad   barras desde cero hasta ESE techo: la cpu y
        //                         el sonido (100), la memoria (la RAM que hay)
        //   sin techo             una LINEA en su propia ventana: dice como
        //                         se MUEVE, que es lo que se puede decir. Los
        //                         vatios (el techo del paquete no se pregunta,
        //                         y uno de catalogo lo prohibe `cpu/power.rs`)
        //                         y el pulso.
        let llenas = unsafe { LLENAS };
        let vistas = || hist.iter().enumerate().filter(move |&(k, _)| k + llenas >= HISTORIA);
        let techo = match i {
            CPU | SONIDO => Some(100),
            MEM => Some((total / (1024 * 1024)).max(1) as u32),
            _ => None,
        };
        let Some(techo) = techo else {
            // La ventana: de la menor a la mayor, con un margen que no baja de
            // la RESOLUCION del instrumento. Sin ese suelo, un numero quieto
            // haria del ruido de la ultima cifra una sierra.
            let resolucion = if i == VATIOS { 50 } else { 25 }; // 5,0 W; 25 vueltas/s
            let (mut lo, mut hi) = (u32::MAX, 0u32);
            for (_, &v) in vistas() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
            if lo > hi {
                continue;
            }
            let margen = ((hi - lo) / 4).max(resolucion);
            let base = lo.saturating_sub(margen);
            let rango = (hi + margen - base).max(1);
            let alto = GRAF_ALTO - 3;
            let y_de = |v: u32| gy + GRAF_ALTO - 3 - (v - base) * alto / rango;
            let mut antes: Option<u32> = None;
            for (k, &v) in vistas() {
                let y = y_de(v);
                // Del punto anterior a este, en vertical: un salto se ve entero
                // y no como dos puntos sueltos.
                let (a, z) = match antes {
                    Some(p0) if p0 < y => (p0, y),
                    Some(p0) => (y, p0),
                    None => (y, y),
                };
                p.rect(x0 + k as u32 * 2, a, 2, z - a + 2, acento());
                antes = Some(y);
            }
            continue;
        };
        for (k, &v) in vistas() {
            let h = (v.min(techo) as u64 * (GRAF_ALTO - 2) as u64 / techo as u64) as u32;
            // Lo que existe y no llega a un pixel se ve como UNO: una memoria
            // de 31 MiB no es una memoria vacia.
            let h = h.max(if v > 0 { 1 } else { 0 });
            if h == 0 {
                continue;
            }
            let color = match i {
                SONIDO if v >= 95 => 0x00EF_4444,
                SONIDO if v >= 80 => 0x00EA_B308,
                SONIDO => 0x0022_C55E,
                CPU if v >= 90 => 0x00EF_4444,
                _ => acento(),
            };
            p.rect(x0 + k as u32 * 2, gy + GRAF_ALTO - 1 - h, 2, h, color);
        }
    }
}

/// **El reloj del pie**: la hora en claro y el dia apagado. Se repinta cuando
/// cambia el minuto, no cada vuelta.
fn reloj(p: &bmo::Pantalla, pl: &Plano) {
    let Some(f) = bmo_rtc::desempaquetar(bmo::info(bmo::INFO_FECHA)) else {
        return;
    };
    let minuto = f.hora as u16 * 60 + f.minuto as u16;
    if unsafe { MINUTO } == minuto {
        return;
    }
    unsafe { MINUTO = minuto };
    let dos = |v: u8| [b'0' + v / 10, b'0' + v % 10];
    p.rect(pl.x0, pl.reloj_y, pl.iw, bmo::GLIFO_ALTO, estilo().barra_fondo);
    let x = p.texto_bytes(pl.x0, pl.reloj_y, &dos(f.hora), INK);
    let x = p.texto(x, pl.reloj_y, ":", INK);
    p.texto_bytes(x, pl.reloj_y, &dos(f.minuto), INK);
    // El dia, pegado a la derecha: `22/09`.
    let dia = dos(f.dia);
    let mes = dos(f.mes);
    let fecha = [dia[0], dia[1], b'/', mes[0], mes[1]];
    let fx = (pl.x0 + pl.iw).saturating_sub(5 * bmo::GLIFO_ANCHO);
    p.texto_bytes(fx, pl.reloj_y, &fecha, INK_DIM);
}
