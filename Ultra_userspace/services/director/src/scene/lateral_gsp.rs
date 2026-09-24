//! **LA LUZ DEL GSP**: en el panel, por donde va el arranque del GSP de la
//! 3060 -- un camino de siete nodos, uno por escalon de L0 que se ve desde
//! fuera, y una palabra con su luz. Sin abrir `gpu` ni leer el INFORME.
//!
//! [consumo] LATE      ocho preguntas al kernel cada 250 ms, con el panel a la
//!                     vista; se repinta si algo cambio, o a 4 Hz mientras la
//!                     luz respira
//!
//! ```text
//!    gsp           * despierto     la palabra: el ultimo escalon, o donde fallo
//!    o---o---o---o---o---o---o     el camino, de izquierda a derecha:
//!    W   R   L   S   D   Q   I
//!
//!      W  WPR2 montada por FWSEC (L0b)       D  el RISC-V se vio activo (L0c3b)
//!      R  el GSP-RM prestado, radix3 (L0c2)  Q  secuenciador corrido (L0c4b2c)
//!      L  LIBOS prestado y comprobado (L0c3a) I  GSP_INIT_DONE (L0c4b2c)
//!      S  SetSystemInfo escrito (L0c4b2a)
//! ```
//!
//! Hecho, un punto del acento y la via hasta el, del acento (todo verde
//! cuando llega `GSP_INIT_DONE`); el que fallo, rojo; el siguiente, un anillo
//! del acento; los de despues, anillos apagados. La luz de delante de la
//! palabra RESPIRA mientras el RISC-V del GSP esta activo: vivo se ve vivo,
//! como la aguja del pulso. Lo pidio el propietario (24-09): *"que si
//! despierta algo mi GPU se lee"*, y luego *"mas elegante"*.
//!
//! # Y debajo, LA 3060 (24-09, *"poner en HUD por completo"*)
//!
//! ```text
//!    3060           49o?     la temperatura; `?` = probable (ver `salud`)
//!    ~~~~~~~~~~~~~~~~~~~     su HISTORIA, un minuto, de 25 a 95 grados, con
//!    - - - - - - - - - -     la raya de 83 (donde la 3060 baja relojes)
//!    pstate           P0     lo que dijo el GSP-RM; P0 a tope, P8 reposo
//!    pcie          1/3 x16   el enlace AHORA / el techo que declara
//!    vram       12G GDDR6    lo que dijo GET_GSP_STATIC_INFO
//! ```
//!
//! ** Eran dos renglones con tres cosas cada uno, y en 17 columnas se
//! pisaban (`pcie 1/312G6GDDR6`, captura del 24-09 11:25). Ahora es el idioma
//! de los instrumentos de abajo: una cosa por renglon, el nombre apagado a la
//! izquierda y la cifra en claro a la derecha.
//!
//! La temperatura y el enlace se leen cada vuelta (`INFO_GPU_SALUD`, dos
//! lecturas); la historia toma una muestra por segundo. El P-state y la VRAM
//! los apunta el escritorio cuando se los pregunta al GSP-RM (`commands`
//! llama a `scene`, no al reves). Los VATIOS de la 3060 no estan: NVIDIA no
//! publica la orden del RM que los da (ver `bmo_gpu_ga10x::salud`), y un
//! numero inventado es peor que ninguno.

use bmo_userland as bmo;

use super::estilo::estilo;
use super::{acento, INK, INK_DIM};

/// Las casillas y como se llaman cuando son la ultima hecha.
const PASOS: usize = 7;
const PALABRAS: [&str; PASOS] = ["WPR2", "prestado", "libos", "escrito", "despierto", "reanudado", "LISTO"];
/// Las de fallo, de 11 letras como mucho: con la luz y `gsp` delante, 12 ya
/// las pegan (visto en la maqueta del 24-09).
const NOMBRES_NO: [&str; PASOS] = ["NO fwsec", "NO radix", "NO libos", "NO sistema", "NO desperto", "NO secuen", "NO init"];

const VERDE: u32 = 0x0022_C55E;
const ROJO: u32 = 0x00EF_4444;

/// Lo que mide el bloque: la palabra, el camino de nodos y sus letras, y los
/// dos renglones de la 3060.
pub(crate) const ALTO: u32 = GSP_ALTO + 12 + GPU_ALTO;
/// El bloque de la 3060: su renglon, la historia y tres renglones mas.
const GPU_ALTO: u32 = RENGLON + HIST_ALTO + 4 + 3 * RENGLON;
const RENGLON: u32 = bmo::GLIFO_ALTO + 2;
const HIST_ALTO: u32 = 20;
/// Muestras de temperatura: una por segundo, a 2 px cada una.
const MUESTRAS: usize = 68;
/// La escala de la historia, en grados, y la raya de aviso.
const T_MIN: u32 = 25;
const T_MAX: u32 = 95;
const T_AVISO: u32 = 83;
const GSP_ALTO: u32 = bmo::GLIFO_ALTO + 8 + NODO + 4 + bmo::GLIFO_ALTO;
/// Un nodo: un punto de 8 px.
const NODO: u32 = 8;
/// La letra de cada nodo, bajo el.
const LETRAS: [u8; PASOS] = *b"WRLSDQI";

/// Como va, leido: las casillas hechas (bit k = escalon k), el que fallo
/// (`PASOS` = ninguno), si hay NVIDIA y si su RISC-V esta activo ahora.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Estado {
    hechos: u8,
    fallo: usize,
    hallada: bool,
    riscv: bool,
    /// Con el RISC-V vivo, la luz respira: cambia de tono cada muestra, y el
    /// bloque se repinta a 4 Hz solo mientras tanto.
    respira: bool,
    /// La 3060: el sensor crudo, el enlace (`LNKSTA | LNKCAP << 32`), el
    /// P-state (`0xFF` sin preguntar) y la VRAM que dijo el GSP-RM.
    termico: u32,
    enlace: u64,
    pstate: u8,
    vram: (u32, u8),
    tomadas: u32,
}

/// Lo pintado, para no repintar si no cambio.
static mut PINTADO: Option<Estado> = None;
/// Como acabo `gpu init` (solo lo sabe el escritorio, no el kernel): lo
/// apunta `commands::gspinit` -- `commands` puede llamar a `scene`, no al
/// reves (L8, `capas.py`). 0 sin intentar, 1 GSP_INIT_DONE, 2 no llego.
static mut INIT: u8 = 0;
/// La fase de la respiracion de la luz.
static mut FASE: bool = false;
/// Lo que apunta el escritorio al preguntarle al GSP-RM: el P-state (`0xFF`
/// sin preguntar) y la VRAM `(MiB, tipo de RAM)`.
static mut PSTATE: u8 = 0xFF;
static mut VRAM: (u32, u8) = (0, 0);
/// La historia de la temperatura (0 = sin muestra), la mas nueva al final, y
/// las vueltas desde la ultima muestra (se pinta a 4 Hz; se muestrea a 1).
static mut HIST: [u8; MUESTRAS] = [0; MUESTRAS];
static mut VUELTAS: u8 = 0;
/// Cuantas muestras se han tomado (para que el Estado cambie con cada una).
static mut TOMADAS: u32 = 0;

/// **El P-state**, tal como lo contesto el GSP-RM (`commands::gspsalud`).
pub(crate) fn pstate(p: u8) {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { PSTATE = p };
}

/// **La VRAM**, tal como la dijo `GET_GSP_STATIC_INFO` (`commands::gsprpc`).
pub(crate) fn vram(mib: u32, tipo: u8) {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { VRAM = (mib, tipo) };
}

/// **`gpu init` acabo**: con `GSP_INIT_DONE` o sin el.
pub(crate) fn init(llego: bool) {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { INIT = if llego { 1 } else { 2 } };
}

/// Donde esta, en `INFO_GPU_GSP_MEM`, el `writePtr` de la cola de la CPU:
/// tras los tres logs (3 x 16 paginas), GspMem + 0x1000 + 16. El mismo sitio
/// que lee `commands::gspsistema`.
const CPU_ESCRITO: u64 = 3 * 16 * 4096 + 0x1000 + 16;

/// Lo pintado se da por perdido (el panel se repinto entero).
pub(crate) fn olvidar() {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { PINTADO = None };
}

fn leer() -> Estado {
    let hallada = bmo::info(bmo::INFO_GPU_CHIP) & bmo::GPU_HALLADA != 0;
    let d = bmo::info(bmo::INFO_GPU_DESPIERTO);
    let sec = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 2 << 8);
    let como = sec >> 24 & 0xFF;
    let g = bmo::info(bmo::INFO_GPU_GSP);
    // SAFETY: el escritorio es un solo hilo.
    let init = unsafe { INIT };
    // Las mismas preguntas que los pasos de `save mode`, hechas aqui.
    let hechos = [
        (bmo::info(bmo::INFO_GPU_WPR2) >> 32) as u32 >> 4 != 0,
        g & bmo::GSP_PRESTADO != 0 && g & bmo::GSP_CUADRA != 0 && g & bmo::GSP_ES_570 != 0,
        bmo::info(bmo::INFO_GPU_LIBOS) & bmo::LIBOS_COMPROBADO != 0,
        bmo::info(bmo::INFO_GPU_GSP_MEM | CPU_ESCRITO << 8) & 0xFFFF_FFFF != 0,
        d & bmo::DESPIERTO_VISTO != 0,
        como & bmo::SEC_HECHO != 0,
        init == 1,
    ];
    let mut bits = 0u8;
    for (k, &h) in hechos.iter().enumerate() {
        bits |= (h as u8) << k;
    }
    // Donde fallo, si fallo: el despertar con su NO apuntado, el secuenciador
    // parado en una orden, o el GSP-RM sin su GSP_INIT_DONE.
    let fallo = if d & bmo::DESPIERTO_VALIDO != 0 && d & bmo::DESPIERTO_VISTO == 0 && (d >> bmo::DESPIERTO_MOTIVO_SHIFT) & 0xFF != 0 {
        4
    } else if como & 0x3F != 0 {
        5
    } else if init == 2 {
        6
    } else {
        PASOS
    };
    let riscv = d & bmo::DESPIERTO_RISCV_ACTIVO != 0;
    // SAFETY: el escritorio es un solo hilo.
    let respira = riscv && unsafe {
        FASE = !FASE;
        FASE
    };
    // SAFETY: el escritorio es un solo hilo.
    let (pstate, vram) = unsafe { (PSTATE, VRAM) };
    let termico = bmo::info(bmo::INFO_GPU_SALUD) as u32;
    // Una muestra por segundo: la cuarta vuelta de cada cuatro.
    // SAFETY: el escritorio es un solo hilo.
    let tomadas = unsafe {
        VUELTAS = (VUELTAS + 1) % 4;
        if VUELTAS == 0 && hallada {
            let h = &mut *core::ptr::addr_of_mut!(HIST);
            h.copy_within(1.., 0);
            h[MUESTRAS - 1] = bmo_gpu_ga10x::salud::grados(termico).map_or(0, |g| g.min(255) as u8);
            TOMADAS = TOMADAS.wrapping_add(1);
        }
        TOMADAS
    };
    Estado {
        hechos: bits,
        fallo,
        hallada,
        riscv,
        respira,
        termico,
        enlace: bmo::info(bmo::INFO_GPU_SALUD | 1 << 8),
        pstate,
        vram,
        tomadas,
    }
}

/// `a` hacia `b`, `t` de 256.
fn mezcla(a: u32, b: u32, t: u32) -> u32 {
    let c = |d: u32| {
        let (x, y) = ((a >> d) & 0xFF, (b >> d) & 0xFF);
        ((x * (256 - t) + y * t) / 256) << d
    };
    c(16) | c(8) | c(0)
}

/// Un punto de 8 px: cuatro filas de sangria por arriba y por abajo (el mismo
/// truco de la tabla de `rounded_rect`, a escala de un nodo).
fn punto(p: &bmo::Pantalla, x: u32, y: u32, color: u32) {
    for (f, s) in [2u32, 1, 0, 0, 0, 0, 1, 2].into_iter().enumerate() {
        p.rect(x + s, y + f as u32, NODO - 2 * s, 1, color);
    }
}

/// Un anillo: el punto, y dentro uno de 6 px del fondo.
fn anillo(p: &bmo::Pantalla, x: u32, y: u32, color: u32, fondo: u32) {
    punto(p, x, y, color);
    for (f, s) in [1u32, 0, 0, 0, 0, 1].into_iter().enumerate() {
        p.rect(x + 1 + s, y + 1 + f as u32, NODO - 2 - 2 * s, 1, fondo);
    }
}

/// **Pintar el bloque** en `(x0, y)`, `iw` de ancho, si cambio.
///
/// ```text
///    gsp             * LISTO     la palabra, con su luz delante
///    o---o---o---o---o---o---o   el camino: hecho, del acento; lo que falta,
///    W   R   L   S   D   Q   I   un anillo; el siguiente, un anillo del acento
/// ```
pub(crate) fn pintar(p: &bmo::Pantalla, x0: u32, y: u32, iw: u32) {
    let e = leer();
    // SAFETY: el escritorio es un solo hilo.
    if unsafe { PINTADO } == Some(e) {
        return;
    }
    unsafe { PINTADO = Some(e) };
    let est = estilo();
    let (fondo, borde) = (est.barra_fondo, est.barra_borde);
    p.rect(x0, y, iw, ALTO, fondo);

    // La palabra, y su luz.
    let listo = e.hechos & 1 << 6 != 0;
    let ultimo = (0..PASOS).rev().find(|&k| e.hechos & 1 << k != 0);
    let hecho = if listo { VERDE } else { acento() };
    let (palabra, tinta, luz) = if !e.hallada {
        ("sin NVIDIA", INK_DIM, borde)
    } else if e.fallo < PASOS {
        (NOMBRES_NO[e.fallo], ROJO, ROJO)
    } else if listo {
        ("LISTO", VERDE, VERDE)
    } else {
        match ultimo {
            None => ("dormida", INK_DIM, borde),
            // Despierto, sin secuenciador corrido y el RISC-V parado: el GSP-RM
            // se paro solo y espera al secuenciador.
            Some(4) if !e.riscv => ("espera sec", INK, acento()),
            Some(k) => (PALABRAS[k], INK, acento()),
        }
    };
    // Vivo, respira: la mitad del tono una muestra si y otra no.
    let luz = if e.respira { mezcla(luz, fondo, 128) } else { luz };
    p.texto(x0, y, "gsp", INK_DIM);
    let px = (x0 + iw).saturating_sub(palabra.len() as u32 * bmo::GLIFO_ANCHO);
    p.texto(px, y, palabra, tinta);
    punto(p, px.saturating_sub(NODO + 6), y + (bmo::GLIFO_ALTO - NODO) / 2, luz);

    // El camino: los nodos repartidos de punta a punta, y la via entre ellos.
    let ny = y + bmo::GLIFO_ALTO + 8;
    let paso = (iw - NODO) / (PASOS as u32 - 1);
    let nx = |k: usize| x0 + k as u32 * paso;
    for k in 0..PASOS - 1 {
        let via = if e.hechos & 1 << (k + 1) != 0 { hecho } else { borde };
        p.rect(nx(k) + NODO, ny + NODO / 2 - 1, paso - NODO, 2, via);
    }
    let siguiente = ultimo.map_or(0, |k| k + 1);
    for k in 0..PASOS {
        let x = nx(k);
        if k == e.fallo {
            punto(p, x, ny, ROJO);
        } else if e.hechos & 1 << k != 0 {
            punto(p, x, ny, hecho);
        } else if k == siguiente && e.hallada && e.fallo == PASOS {
            anillo(p, x, ny, acento(), fondo);
        } else {
            anillo(p, x, ny, borde, fondo);
        }
        // Su letra, centrada bajo el nodo: clara la ultima hecha, roja la que
        // fallo, apagadas las demas.
        let tinta = if k == e.fallo {
            ROJO
        } else if Some(k) == ultimo {
            INK
        } else {
            INK_DIM
        };
        let lx = (x + NODO / 2).saturating_sub(bmo::GLIFO_ANCHO / 2);
        p.texto_bytes(lx, ny + NODO + 4, &LETRAS[k..k + 1], tinta);
    }
    if e.hallada {
        la_3060(p, &e, x0, y + GSP_ALTO + 12, iw);
    }
}

/// Un numero en `t` desde `n`; devuelve donde acabo.
fn num(t: &mut [u8], n: usize, v: u32) -> usize {
    let mut d = [0u8; 10];
    let k = crate::text::decimal(v as u64, &mut d);
    let k = k.min(t.len() - n);
    t[n..n + k].copy_from_slice(&d[..k]);
    n + k
}

fn poner(t: &mut [u8], n: usize, s: &[u8]) -> usize {
    let k = s.len().min(t.len() - n);
    t[n..n + k].copy_from_slice(&s[..k]);
    n + k
}

/// Un renglon: `nombre` apagado a la izquierda, `cifra` a la derecha.
fn renglon(p: &bmo::Pantalla, x0: u32, y: u32, iw: u32, nombre: &str, cifra: &[u8], tinta: u32) {
    p.texto(x0, y, nombre, INK_DIM);
    let cx = (x0 + iw).saturating_sub(cifra.len() as u32 * bmo::GLIFO_ANCHO);
    p.texto_bytes(cx, y, cifra, tinta);
}

/// **El bloque de la 3060**: el idioma de los instrumentos de abajo, una cosa
/// por renglon.
fn la_3060(p: &bmo::Pantalla, e: &Estado, x0: u32, y: u32, iw: u32) {
    use bmo_gpu_ga10x::{estatica, salud};
    let est = estilo();
    let (fondo, borde) = (est.barra_fondo, est.barra_borde);
    p.rect(x0, y, iw, GPU_ALTO, fondo);

    // 3060  49o?  -- verde hasta 70, el acento hasta 83, rojo encima.
    let mut t = [0u8; 8];
    let lectura = salud::lectura(e.termico);
    let (n, tinta) = match lectura {
        Some((g, fe)) => {
            let mut n = num(&mut t, 0, g);
            n = poner(&mut t, n, &[0xB0]);
            if fe == salud::Fe::Probable {
                n = poner(&mut t, n, b"?");
            }
            (n, if g >= T_AVISO { ROJO } else if g >= 70 { acento() } else { VERDE })
        }
        None => (poner(&mut t, 0, b"--"), INK_DIM),
    };
    renglon(p, x0, y, iw, "3060", &t[..n], tinta);

    // La historia: una linea en escala FIJA (25..95), para que un grado sea
    // siempre la misma altura; y la raya de 83, punteada.
    let gy = y + RENGLON;
    let ancho = (MUESTRAS as u32 * 2).min(iw);
    let y_de = |g: u32| gy + HIST_ALTO - 2 - (g.clamp(T_MIN, T_MAX) - T_MIN) * (HIST_ALTO - 3) / (T_MAX - T_MIN);
    p.rect(x0, gy + HIST_ALTO - 1, ancho, 1, borde);
    let raya = y_de(T_AVISO);
    for k in (0..ancho).step_by(6) {
        p.rect(x0 + k, raya, 3, 1, mezcla(ROJO, fondo, 160));
    }
    // SAFETY: el escritorio es un solo hilo.
    let h = unsafe { &*core::ptr::addr_of!(HIST) };
    let mut antes: Option<u32> = None;
    for (k, &g) in h.iter().enumerate() {
        if g == 0 {
            antes = None;
            continue;
        }
        let yy = y_de(g as u32);
        let (a, z) = match antes {
            Some(p0) if p0 < yy => (p0, yy),
            Some(p0) => (yy, p0),
            None => (yy, yy),
        };
        let c = if g as u32 >= T_AVISO { ROJO } else if g >= 70 { acento() } else { mezcla(VERDE, fondo, 64) };
        p.rect(x0 + k as u32 * 2, a, 2, z - a + 2, c);
        antes = Some(yy);
    }

    // pstate  P0 -- a tope en el acento, el reposo (P8 y mas) en verde.
    let y = gy + HIST_ALTO + 4;
    let mut t = [0u8; 4];
    let (n, tinta) = if e.pstate == 0xFF {
        (poner(&mut t, 0, b"--"), INK_DIM)
    } else {
        let n = poner(&mut t, 0, b"P");
        let n = num(&mut t, n, e.pstate as u32);
        (n, if e.pstate == 0 { acento() } else if e.pstate >= 8 { VERDE } else { INK })
    };
    renglon(p, x0, y, iw, "pstate", &t[..n], tinta);

    // pcie  1/3 x16 -- AHORA / el techo, solo si va por debajo.
    let y = y + RENGLON;
    let mut t = [0u8; 12];
    let n = match salud::enlace(e.enlace as u16, (e.enlace >> 32) as u32) {
        Some(l) => {
            let mut n = num(&mut t, 0, l.gen as u32);
            if l.gen < l.gen_max {
                n = poner(&mut t, n, b"/");
                n = num(&mut t, n, l.gen_max as u32);
            }
            n = poner(&mut t, n, b" x");
            num(&mut t, n, l.ancho as u32)
        }
        None => poner(&mut t, 0, b"--"),
    };
    renglon(p, x0, y, iw, "pcie", &t[..n], INK);

    // vram  12G GDDR6
    let y = y + RENGLON;
    let mut t = [0u8; 12];
    let n = if e.vram.0 == 0 {
        poner(&mut t, 0, b"--")
    } else {
        let mut n = num(&mut t, 0, e.vram.0 >> 10);
        n = poner(&mut t, n, b"G");
        let r = estatica::ram(e.vram.1 as u32);
        if r.len() <= 6 {
            n = poner(&mut t, n, b" ");
            n = poner(&mut t, n, r);
        }
        n
    };
    renglon(p, x0, y, iw, "vram", &t[..n], INK);
}
