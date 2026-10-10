//! **D2b: UNA IMAGEN DE 32 BITS POR LA 3060** -- la orden `gpu imagen` y su
//! fila. El paso del carril D de `docs/plan/PLAN_VERRANO.md` que prueba en el
//! metal lo que DOOM necesita: su 320 x 200 (`0x00RRGGBB`), agrandado por la
//! 3060 DIRECTAMENTE en la pantalla. La CPU no agranda ni copia un pixel:
//! comprueba 256 de cada fotograma.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: un trabajo de
//!                     la 3060 por fotograma, a su hora; en reposo, nada
//!
//! # Dos formas
//!
//! ```text
//!    gpu imagen                                   la carta de ajuste de
//!                                                 320x200, 90 fotogramas
//!                                                 hechos en memoria: sin
//!                                                 fichero, se prueba sola
//!    gpu imagen <fichero> <ancho>x<alto> [fps]    fotogramas crudos de 32
//!                                                 bits seguidos (una captura
//!                                                 de DOOM: 320x200, 256000 B)
//! ```
//!
//! Es `gpu video` con otro formato, a proposito: el mismo prestamo, la misma
//! espera, la misma forma de contar.

use bmo_gpu_ga10x::imagen as im;
use bmo_userland as bmo;

use super::super::tabla::campo;
use super::super::After;
use super::{con, estado, hasta_el_lienzo, no, Linea, NO_TRABAJO_SIN_FICHA, PANEL_ABIERTO};
use crate::desktop::Desktop;
use crate::scene::surface::{Table, MAX as CAJAS};
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo mas que dura una tanda: el escritorio no ve el teclado mientras tanto.
const TOPE_S: u64 = 120;

/// Lo que dejo una tanda.
#[derive(Clone, Copy)]
pub(super) struct Tanda {
    ancho: u32,
    alto: u32,
    escala: u32,
    fps: u32,
    /// `true`: la carta de ajuste; `false`: un fichero.
    carta: bool,
    en_fichero: u64,
    pedidos: u32,
    buenos: u32,
    /// Sumados, en us: hacer o leer el fotograma, la 3060, comprobar; y todo.
    origen_us: u64,
    gpu_us: u64,
    cpu_us: u64,
    total_us: u64,
    tarde: u32,
    /// El primero que no salio: su NO, o lo que dijo la 3060.
    malo: Option<(u32, Result<u64, u32>)>,
}

impl Tanda {
    fn bien(&self) -> bool {
        self.pedidos > 0 && self.buenos == self.pedidos && self.malo.is_none()
    }

    /// Fotogramas por segundo que se VIERON, en decimas.
    fn fps10(&self) -> u64 {
        self.buenos as u64 * 10_000_000 / self.total_us.max(1)
    }
}

fn ficha() -> Result<u64, u32> {
    hasta_el_lienzo()?;
    estado().timbre.map(|(v, _)| v as u64).ok_or(NO_TRABAJO_SIN_FICHA)
}

/// `<ruta> <ancho>x<alto> [fps]`.
fn parsear(args: &[u8]) -> Option<(&[u8], u32, u32, u32)> {
    let mut t = args.split(|&b| b == b' ').filter(|t| !t.is_empty());
    let ruta = t.next()?;
    let medida = t.next()?;
    let x = medida.iter().position(|&b| b == b'x' || b == b'X')?;
    let (w, h) = (numero(&medida[..x])?, numero(&medida[x + 1..])?);
    let fps = match t.next() {
        Some(f) => numero(f)?,
        None => 35,
    };
    (t.next().is_none() && (1..=60).contains(&fps)).then_some((ruta, w, h, fps))
}

fn numero(s: &[u8]) -> Option<u32> {
    if s.is_empty() || s.len() > 5 {
        return None;
    }
    s.iter().try_fold(0u32, |v, &c| c.is_ascii_digit().then(|| v * 10 + (c - b'0') as u32))
}

/// **La tanda**: el formato a la 3060, y fotograma a fotograma, a su hora.
/// `ruta` = `None`: la carta de ajuste, [`CARTA`] fotogramas.
fn tanda(ruta: Option<&[u8]>, f: im::Formato, fps: u32) -> Result<Tanda, u32> {
    let ficha = ficha()?;
    let encaje = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_IMAGEN_FORMATO, f.ancho as u64 | (f.alto as u64) << 16 | ficha << 32)?;
    let bytes = f.bytes();
    let a = match ruta {
        Some(r) => Some(bmo::Archivo::reflejar(r)?),
        None => None,
    };
    let bloque = bmo::Memoria::request(bytes).ok_or(super::NO_IMAGEN_SIN_MEMORIA)?;
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let us = |c: u64| c * 1_000_000 / hz;
    let periodo = hz / fps as u64;
    let mut t = Tanda {
        ancho: f.ancho,
        alto: f.alto,
        escala: (encaje & 0xFF) as u32,
        fps,
        carta: a.is_none(),
        en_fichero: a.as_ref().map_or(CARTA as u64, |a| a.size() / bytes),
        pedidos: 0,
        buenos: 0,
        origen_us: 0,
        gpu_us: 0,
        cpu_us: 0,
        total_us: 0,
        tarde: 0,
        malo: None,
    };
    let desde = bmo::ciclos();
    let mut k = 0u64;
    while k < t.en_fichero && us(bmo::ciclos() - desde) < TOPE_S * 1_000_000 {
        let c = bmo::ciclos();
        match &a {
            Some(a) => {
                a.saltar(k * bytes);
                if a.leer_en(&bloque, 0, bytes) != bytes {
                    t.malo = Some((k as u32, Err(super::NO_IMAGEN_MAL)));
                    break;
                }
            }
            None => {
                // SAFETY: el bloque mide `bytes` = ancho x alto x 4, es de
                // este proceso, alineado a pagina, y solo se usa aqui.
                let px = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut u32, (f.ancho * f.alto) as usize) };
                im::carta(&f, k as u32, px);
            }
        }
        t.origen_us += us(bmo::ciclos() - c);
        let cargar = if k == 0 { bmo::IMAGEN_CARGAR } else { 0 };
        let v = match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_IMAGEN, bloque.base() as u64 | cargar) {
            Ok(v) => v,
            Err(m) => {
                t.malo = Some((k as u32, Err(m)));
                break;
            }
        };
        t.pedidos += 1;
        let (_, _, _, _, gpu_us, cpu_us) = im::desempaquetar(v);
        t.gpu_us += gpu_us as u64;
        t.cpu_us += cpu_us as u64;
        if !im::sano(v) {
            t.malo = Some((k as u32, Ok(v)));
            break;
        }
        t.buenos += 1;
        k += 1;
        let hora = desde + k * periodo;
        if bmo::ciclos() > hora {
            t.tarde += 1;
        }
        while bmo::ciclos() < hora {
            bmo::yield_screen();
        }
    }
    t.total_us = us(bmo::ciclos() - desde);
    if let Some(a) = a {
        a.close();
    }
    Ok(t)
}

/// Los fotogramas de la carta: 90, a 35 fps (los de DOOM), unos 2,6 s.
const CARTA: u32 = 90;

const VERDE: u32 = 0x0076_B900;
const CLARO: u32 = 0x00E6_EDF6;
const TENUE: u32 = 0x008A_94A6;
const CAJA: u32 = 0x000B_0D12;

fn uso(dsk: &mut Desktop) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ERR);
    g.text(b"  gpu imagen   o   gpu imagen <fichero> <ancho>x<alto> [fps]   (p. ej. `gpu imagen capturas/doom.raw 320x200`)\n");
    g.with_ink(INK_ECHO);
    g.text(b"  sin fichero: la carta de ajuste de 320x200; con fichero: fotogramas crudos de 32 bits (0x00RRGGBB) seguidos\n");
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu imagen [<fichero> <ancho>x<alto> [fps]]`.
pub(crate) fn orden_imagen(dsk: &mut Desktop, p: &bmo::Pantalla, args: &[u8]) -> After {
    let (ruta, f, fps) = if args.iter().all(|&b| b == b' ') {
        (None, im::Formato::DOOM, 35)
    } else {
        let Some((ruta, ancho, alto, fps)) = parsear(args) else { return uso(dsk) };
        (Some(ruta), im::Formato { ancho, alto }, fps)
    };
    if !f.valido() {
        return uso(dsk);
    }
    paint_status(p, &dsk.run_box, "la 3060 agranda la imagen en la pantalla", INK_DIM);
    p.rect(0, 0, p.ancho, p.alto, 0);
    p.vaciar();
    let r = tanda(ruta, f, fps);
    con(|c| c.imagen = Some(r));
    let y = p.alto.saturating_sub(150);
    let ancho_caja = p.ancho.saturating_sub(48).min(1100);
    p.rect(24, y, ancho_caja, 126, CAJA);
    p.rect(24, y, ancho_caja, 3, VERDE);
    p.texto_escala(40, y + 14, "LA IMAGEN DE DOOM, POR LA 3060", VERDE, 2);
    let bien = matches!(r, Ok(t) if t.bien());
    match r {
        Ok(t) => {
            let mut l = Linea::nueva();
            l.t(b"").d(t.buenos as u64).t(b" fotogramas de ").d(t.ancho as u64).t(b"x").d(t.alto as u64).t(b" (x").d(t.escala as u64).t(b"), ");
            l.d(t.fps10() / 10).t(b".").d(t.fps10() % 10).t(b" fps de ").d(t.fps as u64).t(b"; la 3060: ");
            l.d(t.gpu_us / t.pedidos.max(1) as u64).t(b" us por fotograma");
            p.texto_bytes(40, y + 54, &l.b[..l.n], if bien { CLARO } else { 0x00FF_5555 });
        }
        Err(m) => {
            p.texto_bytes(40, y + 54, super::super::iommu::motivo(m), 0x00FF_5555);
        }
    }
    p.texto_bytes(40, y + 78, b"la 3060 agranda cada fotograma y lo escribe donde mira el monitor; la CPU no mueve ni un pixel", TENUE);
    p.texto_bytes(40, y + 100, b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = true };
    let g = &mut dsk.out.grid;
    g.with_ink(if bien { INK_GOOD } else { INK_ERR });
    g.text(if bien {
        b"  LA 3060 AGRANDO LA IMAGEN EN LA PANTALLA (D2b): mira la fila `imagen`\n" as &[u8]
    } else {
        b"  la imagen no salio entera: mira la fila `imagen`\n"
    });
    g.with_ink(INK_PLAIN);
    super::fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// La fila `imagen`.
pub(super) fn fila(s: &mut Output, c: &super::Computo) {
    let Some(r) = c.imagen else { return };
    campo(s, b"imagen");
    let t = match r {
        Err(m) => return no(s, m),
        Ok(t) => t,
    };
    s.with_ink(if t.bien() { INK_GOOD } else { INK_ERR });
    s.text(if t.bien() { b"LA 3060 AGRANDO LA IMAGEN: " as &[u8] } else { b"la imagen NO salio entera: " });
    s.dec(t.buenos as u64);
    s.text(b" de ");
    s.dec(t.en_fichero);
    s.text(if t.carta { b" fotogramas de la carta, " as &[u8] } else { b" fotogramas del fichero, " });
    s.dec(t.ancho as u64);
    s.byte(b'x');
    s.dec(t.alto as u64);
    s.text(b" (x");
    s.dec(t.escala as u64);
    s.byte(b')');
    s.with_ink(INK_ECHO);
    s.text(b"; ");
    s.dec(t.fps10() / 10);
    s.byte(b'.');
    s.dec(t.fps10() % 10);
    s.text(b" fps de ");
    s.dec(t.fps as u64);
    let n = t.pedidos.max(1) as u64;
    s.text(if t.carta { b"; por fotograma: hacerla " as &[u8] } else { b"; por fotograma: disco " });
    s.dec(t.origen_us / n);
    s.text(b" us, la 3060 ");
    s.dec(t.gpu_us / n);
    s.text(b" us, comprobar ");
    s.dec(t.cpu_us / n);
    s.text(b" us; ");
    s.dec(t.tarde as u64);
    s.text(b" tarde");
    if let Some((f, r)) = t.malo {
        s.with_ink(INK_ERR);
        s.text(b"; el ");
        s.dec(f as u64);
        match r {
            Err(m) => {
                s.text(b": NO: ");
                s.text(super::super::iommu::motivo(m));
            }
            Ok(r) => {
                let (buenos, qmd, fin, _, _, _) = im::desempaquetar(r);
                s.text(b": ");
                s.dec(buenos as u64);
                s.text(b" de 256 muestras, semaforos ");
                s.text(if qmd && fin { b"PAGADOS" as &[u8] } else { b"sin pagar" });
            }
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::super::datos::anotar(b"gpu imagen fps10", t.fps10(), b"");
}

// == D2c: DOOM EN VIVO POR LA 3060 (2026-09-26) ===============================
//
// Una app que pinta su 320 x 200 SIN agrandar y lo marca `SUP_A_LA_3060`
// (DOOM con `3060` detras: `gpu doom`, o su icono con la 3060 lista) no se
// compone con la CPU: se pone a pantalla completa y la 3060 la agranda
// DIRECTAMENTE en la pantalla con `imagen`, leyendo el fotograma del PRESTAMO
// que el escritorio tomo (IMAGEN_PRESTADO). El volcado del escritorio solo
// copia lo sucio del lienzo, asi que mientras nada se pinte encima (ni el
// cursor: ver `desktop::paint`), la 3060 manda en la pantalla.

/// La vuelta anterior la 3060 presento: el programa sigue cargado. Si no,
/// la proxima lo carga (otro trabajo de la 3060 pudo usar sus paginas).
static mut EN_VIVO: bool = false;
/// El formato que se le dio a la 3060 en esta racha.
static mut FORMATO_VIVO: (u32, u32) = (0, 0);
/// ** 10-10: LO DE ENCIMA MANDA (`bmo_foco::encima`): con Alt+Tab abierto la
/// 3060 se queda quieta, y al cerrarlo vuelve con un fotograma.
static mut PAUSA: bmo_foco::encima::Pausa = bmo_foco::encima::Pausa::nueva();

/// **La 3060 esta lista para presentar**, SIN arrancar nada: el lienzo de M5d
/// hecho (lo deja `save mode`) y la ficha del timbre de GR0. Esto se pregunta
/// en cada vuelta: `hasta_el_lienzo` arrancaria pasos, y eso no puede ir aqui.
pub(crate) fn la_3060_lista() -> Option<u64> {
    if !super::pintado() {
        return None;
    }
    estado().timbre.map(|(v, _)| v as u64)
}

/// Un fotograma prestado a la pantalla. `nuevo`: el formato y el programa.
fn presentar(va: u64, ancho: u32, alto: u32, ficha: u64, nuevo: bool) -> Result<u64, u32> {
    if nuevo {
        bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_IMAGEN_FORMATO, ancho as u64 | (alto as u64) << 16 | ficha << 32)?;
    }
    let cargar = if nuevo { bmo::IMAGEN_CARGAR } else { 0 };
    let v = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_IMAGEN, va | bmo::IMAGEN_PRESTADO | cargar)?;
    if im::sano(v) {
        Ok(v)
    } else {
        Err(super::NO_IMAGEN_MAL)
    }
}

/// **Cada vuelta del escritorio**: las apps que piden la 3060. `true` si la
/// 3060 manda en la pantalla (entonces no se pinta el cursor).
///
/// `encima`: el escritorio tiene algo delante de TODO (el conmutador de
/// Alt+Tab). Entonces la 3060 se queda QUIETA -- ni un fotograma, y la
/// pantalla no es de la receta directa -- para que no lo tape; al quitarse,
/// se vuelca lo devuelto y la 3060 VUELVE con un fotograma aunque la app no
/// haya hecho otro (`bmo_foco::encima`).
///
/// Sin la 3060 lista no se hace nada: la app se compone con la CPU como
/// cualquier ventana (chica, 320 x 200). Si la 3060 dice NO, se dice una vez
/// por la consola y la app vuelve a la CPU para siempre -- un NO reintentado
/// en cada vuelta seria un escritorio atascado.
pub(crate) fn presentar_apps(t: &mut Table, p: &bmo::Pantalla, encima: bool) -> bool {
    use bmo_foco::encima::La3060;
    // SAFETY: el escritorio es un solo hilo.
    let la = unsafe { (*core::ptr::addr_of_mut!(PAUSA)).vuelta(encima) };
    if la == La3060::Vuelve {
        // Lo devuelto del lienzo, a la pantalla ANTES que la 3060: al reves,
        // el volcado del final del fotograma pisaria su fotograma.
        p.volcar();
        // SAFETY: el escritorio es un solo hilo.
        unsafe { *core::ptr::addr_of_mut!(EN_VIVO) = false };
    }
    let quieta = la == La3060::Quieta;
    let mut manda = false;
    for i in 0..CAJAS {
        if directa(t, i, p, quieta) {
            manda = true;
            continue;
        }
        let Some((va, ancho, alto, seq)) = t.get(i).and_then(|s| s.para_la_3060()) else { continue };
        let Some(ficha) = la_3060_lista() else { continue };
        if quieta {
            // Ni la entrada: su negro taparia lo de encima.
            manda |= t.get(i).is_some_and(|s| s.a_pantalla_completa());
            continue;
        }
        manda = true;
        // ** LA ENTRADA: pantalla completa y el lienzo en negro (lo que la
        // imagen agrandada no cubre). Esta vuelta solo sale el negro.
        if !t.get(i).is_some_and(|s| s.a_pantalla_completa()) {
            t.pantalla_completa(i, p);
            p.rect(0, 0, p.ancho, p.alto, 0);
            if let Some(s) = t.get_mut(i) {
                s.reservada_para_la_3060();
            }
            // SAFETY: el escritorio es un solo hilo.
            unsafe { *core::ptr::addr_of_mut!(EN_VIVO) = false };
            continue;
        }
        // SAFETY: el escritorio es un solo hilo.
        let nuevo = unsafe { !EN_VIVO || FORMATO_VIVO != (ancho, alto) };
        if !nuevo && t.get(i).is_some_and(|s| s.presentada() == seq) {
            continue;
        }
        let r = presentar(va, ancho, alto, ficha, nuevo);
        let Some(s) = t.get_mut(i) else { continue };
        match r {
            Ok(_) => {
                s.presentada_por_la_3060(seq);
                // SAFETY: el escritorio es un solo hilo.
                unsafe {
                    *core::ptr::addr_of_mut!(EN_VIVO) = true;
                    *core::ptr::addr_of_mut!(FORMATO_VIVO) = (ancho, alto);
                }
            }
            Err(m) => {
                s.sin_la_3060();
                manda = false;
                bmo::consola("[3060] la app vuelve a la CPU: ");
                bmo::consola(core::str::from_utf8(super::super::iommu::motivo(m)).unwrap_or("?"));
                bmo::consola("\n");
            }
        }
    }
    if !manda {
        // SAFETY: el escritorio es un solo hilo.
        unsafe {
            *core::ptr::addr_of_mut!(EN_VIVO) = false;
            *core::ptr::addr_of_mut!(FORMATO_VIVO) = (0, 0);
        }
    }
    manda
}

/// **P3b4c.9 Z1: una app que dibuja DIRECTO en la pantalla** (PROTON-X,
/// `SUP_LA_3060_DIRECTA`). La primera vuelta: pantalla completa, en negro, y
/// que `compose` no la pegue; la siguiente, el kernel le da la pantalla a su
/// tid. Desde ahi sus recetas dibujan en la ventana de la pantalla y NADA
/// pasa por la CPU ni por la RAM: el escritorio solo se aparta (sin cursor,
/// sin volcar). Si la app deja de pedirlo, o el kernel dice que no, se le
/// quita y vuelve a la CPU. `true` si esta app manda en la pantalla.
///
/// `quieta` (10-10): algo del escritorio esta encima de todo. La pantalla se
/// le APARTA (sus recetas dibujan en su RAM, como sin ella) y se le devuelve
/// al quitarse, sin decirlo otra vez por la consola.
fn directa(t: &mut Table, i: usize, p: &bmo::Pantalla, quieta: bool) -> bool {
    let Some(s) = t.get(i) else { return false };
    let (pide, dada, tid) = (s.directa(), s.pantalla_dada, s.tid);
    if quieta && pide && la_3060_lista().is_some() {
        let completa = s.a_pantalla_completa();
        if dada {
            let _ = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PANTALLA_PARA, 0);
            if let Some(s) = t.get_mut(i) {
                s.pantalla_dada = false;
                s.pantalla_apartada = true;
            }
        }
        return completa;
    }
    if !pide || la_3060_lista().is_none() {
        if dada {
            let _ = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PANTALLA_PARA, 0);
            if let Some(s) = t.get_mut(i) {
                s.pantalla_dada = false;
                s.sin_la_3060();
            }
            bmo::consola("[3060] la app deja la pantalla: vuelve a la CPU\n");
        }
        return false;
    }
    if !s.a_pantalla_completa() {
        t.pantalla_completa(i, p);
        p.rect(0, 0, p.ancho, p.alto, 0);
        if let Some(s) = t.get_mut(i) {
            s.reservada_para_la_3060();
        }
        return true;
    }
    if !dada {
        let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PANTALLA_PARA, tid as u64);
        let Some(s) = t.get_mut(i) else { return false };
        match r {
            Ok(_) => {
                s.pantalla_dada = true;
                if !core::mem::replace(&mut s.pantalla_apartada, false) {
                    bmo::consola("[3060] la app dibuja DIRECTO en la pantalla (Z1): el fotograma no sale de la VRAM\n");
                }
            }
            Err(m) => {
                s.sin_la_3060();
                bmo::consola("[3060] la pantalla no se le pudo dar a la app: ");
                bmo::consola(core::str::from_utf8(super::super::iommu::motivo(m)).unwrap_or("?"));
                bmo::consola("\n");
                return false;
            }
        }
    }
    true
}

/// **`gpu doom`**: DOOM por la 3060. Lanza `apps/doom.bex 3060` por la misma
/// ranura que un icono; si la 3060 no esta lista, lo dice y no lanza.
pub(crate) fn orden_doom(dsk: &mut Desktop) -> After {
    let g = &mut dsk.out.grid;
    if la_3060_lista().is_none() {
        g.with_ink(INK_ERR);
        g.text(b"  la 3060 no esta lista para DOOM: primero `save mode` (o `gpu lienzo`)\n");
    } else if crate::scene::abrir::pedir(&[b"apps/doom.bex", b"3060"]) {
        g.with_ink(INK_GOOD);
        g.text(b"  DOOM POR LA 3060: 320x200, la tarjeta lo agranda a pantalla completa\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  no se pudo pedir el lanzamiento de apps/doom.bex\n");
    }
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}
