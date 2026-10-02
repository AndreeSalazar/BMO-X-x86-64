//! **VITALES: lo que la maquina esta haciendo AHORA** -- F7 y F8, UNA ventana
//! con tres solapas: CPU, MEMORIA y PROCESOS.
//!
//! [consumo] LATE      un cuarto de segundo SOLO con la ventana abierta y sin
//!                     tapar; cerrada, NADA (L6h)
//!
//! Pedido por el propietario el 2026-08-12: *"el F7 y F8, pero al presionar no
//! veo mi terminal la caja para ver -- no es por terminal sino SU PROPIO
//! terminal para facilitar las vistas... y falta el mem, que estan comiendo,
//! inspirado en administrador de tareas"*. Y el 02-10, con F8 diciendo 129 MiB
//! de 2518 usados: *"le falta su parte e influencias... mejora con interfaz y
//! vista y control interfaz simple"*. Lo que eligio:
//!
//! ```text
//!    solapas        CPU, MEMORIA y PROCESOS en una ventana (1 2 3, Tab, o un
//!                    clic en la solapa); F7 abre en CPU, F8 en MEMORIA
//!    graficas        la CPU, los vatios y la memoria de los ultimos 15 s
//!    el resto        la memoria POR QUIEN LA TIENE: el kernel, lo que cada programa
//!                    pidio, y lo que nadie tiene apuntado (que cuadre: los
//!                    2518 MiB del metal eran 129 de programas y el resto
//!                    sin decir de quien)
//!    nombres         cada programa con su nombre, no solo su pid
//!    control         flechas para elegir, O para ordenar, F para FINALIZAR
//!                    (como el administrador de tareas) -- solo lo que lanzo
//!                    el escritorio: cerrar es tener el handle de haberlo
//!                    lanzado, no una autoridad (`bmo::Hijo`)
//! ```
//!
//! # Por que no son un comando de la caja de Ejecutar
//!
//! `info` en la caja es una FOTO: se escribe y se queda en el historial. Esto
//! es una VISTA: se repinta sola, y lo que se mira es **como cambia**. Es la
//! misma frontera que separa CABINA (F11) de `cabina` como orden.
//!
//! El mando (teclas, raton, abrir y cerrar) vive en `desktop::vitales`; aqui
//! solo se pinta y se mide.

use bmo_registro::Serie;
use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;
use crate::text::decimal;

const VIT_PCT_W: u32 = 56;
const VIT_PCT_H: u32 = 60;
const VIT_MIN_W: u32 = 560;
const VIT_MIN_H: u32 = 360;

const VIT_BG: u32 = 0x0008_0C10;
pub(crate) const VIT_TITLE_BG: u32 = 0x000E_161B;
const VIT_EDGE: u32 = 0x001B_3340;
/// El mismo cian del gato que usa CABINA.
const VIT_CYAN: u32 = 0x0034_E2E4;
const VIT_CYAN_DIM: u32 = 0x0017_6E70;
const C_CPU: u32 = 0x0039_FF88;
const C_MEM: u32 = 0x005E_F2E6;
const C_VATIOS: u32 = 0x00F0_B060;
const C_ELEGIDO: u32 = 0x0012_2A33;

/// Muestras de cada grafica: 15 s a cuatro por segundo.
pub(crate) const MUESTRAS: usize = 60;

/// Cuantas fichas de programa tiene el kernel (`INFO_PROG_*`): ocho.
pub(crate) const FICHAS: usize = 8;

/// **La solapa.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Solapa {
    /// F7 -- el CPU: a que va, que gasta, quien esta despierto.
    Cpu,
    /// F8 -- la memoria: cuanta hay, y **quien se la esta comiendo**.
    Memoria,
    /// Los programas, con su nombre, y el boton de finalizar.
    Procesos,
}

impl Solapa {
    pub(crate) const TODAS: [Solapa; 3] = [Solapa::Cpu, Solapa::Memoria, Solapa::Procesos];

    fn rotulo(self) -> &'static str {
        match self {
            Solapa::Cpu => " 1 CPU ",
            Solapa::Memoria => " 2 MEMORIA ",
            Solapa::Procesos => " 3 PROCESOS ",
        }
    }

    pub(crate) fn siguiente(self) -> Solapa {
        match self {
            Solapa::Cpu => Solapa::Memoria,
            Solapa::Memoria => Solapa::Procesos,
            Solapa::Procesos => Solapa::Cpu,
        }
    }
}

/// Como se ordena la tabla de PROCESOS.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Orden {
    /// La que mas memoria pidio, arriba.
    Memoria,
    Pid,
    Nombre,
}

impl Orden {
    pub(crate) fn siguiente(self) -> Orden {
        match self {
            Orden::Memoria => Orden::Pid,
            Orden::Pid => Orden::Nombre,
            Orden::Nombre => Orden::Memoria,
        }
    }

    fn nombre(self) -> &'static str {
        match self {
            Orden::Memoria => "memoria",
            Orden::Pid => "pid",
            Orden::Nombre => "nombre",
        }
    }
}

pub(crate) struct VitalsWindow {
    pub(crate) chrome: Chrome,
    pub(crate) solapa: Solapa,
    /// La fila elegida en PROCESOS (en el orden de la tabla).
    pub(crate) elegido: usize,
    pub(crate) orden: Orden,
    /// Lo ultimo que hizo el control (finalizar), para el pie.
    pub(crate) aviso: [u8; 80],
    pub(crate) aviso_n: usize,
    /// Las graficas: la CPU en %, los vatios en mW y la memoria en MiB.
    pub(crate) cpu: Serie<MUESTRAS>,
    pub(crate) vatios: Serie<MUESTRAS>,
    pub(crate) memoria: Serie<MUESTRAS>,
    /// La lectura anterior (el TSC y el reposo del nucleo de arranque): la CPU
    /// es una RESTA entre dos.
    tsc: u64,
    reposo: u64,
}

impl VitalsWindow {
    pub(crate) fn new(p: &bmo::Pantalla) -> Self {
        Self {
            chrome: Chrome::new(p, VIT_PCT_W, VIT_PCT_H, VIT_MIN_W, VIT_MIN_H),
            solapa: Solapa::Cpu,
            elegido: 0,
            orden: Orden::Memoria,
            aviso: [0; 80],
            aviso_n: 0,
            cpu: Serie::nueva(),
            vatios: Serie::nueva(),
            memoria: Serie::nueva(),
            tsc: 0,
            reposo: 0,
        }
    }

    /// **Una muestra** para las graficas (cada cuarto de segundo, con la
    /// ventana abierta). `mw`: los vatios del paquete que midio el lector
    /// del escritorio (`Tick::consumo`), si los hay.
    pub(crate) fn muestrear(&mut self, mw: Option<u64>) {
        let ahora = bmo::ciclos();
        let reposo = bmo::info(bmo::INFO_BSP_TICKS_REPOSO);
        if self.tsc != 0 && ahora > self.tsc {
            let dt = ahora - self.tsc;
            let dr = reposo.saturating_sub(self.reposo).min(dt);
            self.cpu.poner(100 - dr * 100 / dt);
        }
        self.tsc = ahora;
        self.reposo = reposo;
        if let Some(mw) = mw {
            self.vatios.poner(mw);
        }
        let total = bmo::info(bmo::INFO_RAM_TOTAL);
        self.memoria.poner(total.saturating_sub(bmo::info(bmo::INFO_RAM_LIBRE)) / (1024 * 1024));
    }

    pub(crate) fn decir(&mut self, s: &[u8]) {
        let n = s.len().min(self.aviso.len());
        self.aviso[..n].copy_from_slice(&s[..n]);
        self.aviso_n = n;
    }

    /// La y de la linea de solapas.
    fn y_solapas(&self) -> u32 {
        self.chrome.y + TITLE_H + 8
    }

    /// `(x0, x1)` de cada solapa, en el orden de [`Solapa::TODAS`].
    fn x_solapas(&self) -> [(u32, u32); 3] {
        let mut x = self.chrome.x + 16;
        let mut v = [(0, 0); 3];
        for (k, t) in Solapa::TODAS.iter().enumerate() {
            let w = t.rotulo().len() as u32 * bmo::GLIFO_ANCHO;
            v[k] = (x, x + w);
            x += w + bmo::GLIFO_ANCHO;
        }
        v
    }

    /// La solapa bajo el punto `(x, y)`, si hay una.
    pub(crate) fn solapa_en(&self, x: u32, y: u32) -> Option<Solapa> {
        let py = self.y_solapas();
        if y + 3 < py || y > py + bmo::GLIFO_ALTO + 4 {
            return None;
        }
        self.x_solapas().iter().position(|&(a, b)| x >= a && x < b).map(|k| Solapa::TODAS[k])
    }

    /// La y donde empieza el contenido de una solapa.
    fn y_contenido(&self) -> u32 {
        self.y_solapas() + bmo::GLIFO_ALTO + 16
    }

    /// La fila de PROCESOS bajo `(x, y)`, si hay una (en el orden de la
    /// tabla).
    pub(crate) fn fila_en(&self, x: u32, y: u32) -> Option<usize> {
        if self.solapa != Solapa::Procesos || !self.chrome.contains(x, y) {
            return None;
        }
        let y0 = self.y_contenido() + FILA_TABLA + 6;
        let k = (y.checked_sub(y0)? / FILA_TABLA) as usize;
        (k < FICHAS).then_some(k)
    }
}

// ===================================================================
//  Los programas
// ===================================================================

/// **Un programa de la tabla**: su ficha del kernel, lo que pidio y si el
/// escritorio lo puede cerrar.
#[derive(Clone, Copy)]
pub(crate) struct Fila {
    pub(crate) pid: u64,
    pub(crate) tid: u32,
    nombre: [u8; 40],
    nombre_n: usize,
    /// Lo que tiene PEDIDO ahora (`INFO_MEM_QUIEN_*`), si el kernel lo tiene.
    pide: Option<u64>,
    /// Lo que se mapeo de su imagen.
    mapeado: u64,
    /// `Some(vivo)` si lo lanzo el escritorio (puede cerrarlo); `None` si no
    /// (o ya no esta).
    pub(crate) vive: Option<bool>,
}

impl Fila {
    pub(crate) fn nombre(&self) -> &[u8] {
        &self.nombre[..self.nombre_n]
    }
}

pub(crate) const FILA_VACIA: Fila = Fila { pid: 0, tid: 0, nombre: [0; 40], nombre_n: 0, pide: None, mapeado: 0, vive: None };

/// Lo que el pid `pid` tiene pedido ahora (las ranuras de memoria por pid).
fn pedido(pid: u64) -> Option<u64> {
    for r in 0..16u64 {
        let q = bmo::info(bmo::INFO_MEM_QUIEN_PID | (r << 8));
        if q == 0 {
            break;
        }
        if q == pid {
            return Some(bmo::info(bmo::INFO_MEM_QUIEN_BYTES | (r << 8)));
        }
    }
    None
}

/// El nombre del pid `pid` en las fichas (el ultimo programa con ese pid).
fn nombre_de(pid: u64, dst: &mut [u8; 40]) -> usize {
    let mut n = 0;
    for k in 0..FICHAS as u64 {
        let quien = bmo::info(bmo::INFO_PROG_QUIEN | (k << 8));
        if quien == 0 {
            break;
        }
        if quien & 0xFFFF == pid {
            n = bmo::info_texto(bmo::INFO_TXT_PROG_NOMBRE | (k << 8), dst).min(40);
        }
    }
    n
}

/// **Los programas, ordenados** como dice `orden`. Devuelve cuantos.
pub(crate) fn filas(orden: Orden, v: &mut [Fila; FICHAS]) -> usize {
    let mut n = 0usize;
    for k in 0..FICHAS as u64 {
        let quien = bmo::info(bmo::INFO_PROG_QUIEN | (k << 8));
        if quien == 0 {
            break;
        }
        let mut f = FILA_VACIA;
        f.pid = quien & 0xFFFF;
        f.tid = ((quien >> 16) & 0xFFFF) as u32;
        f.nombre_n = bmo::info_texto(bmo::INFO_TXT_PROG_NOMBRE | (k << 8), &mut f.nombre).min(40);
        f.pide = pedido(f.pid);
        f.mapeado = bmo::info(bmo::INFO_PROG_IMAGEN | (k << 8)) >> 32;
        f.vive = bmo::Hijo::por_tid(f.tid).map(|h| h.vive());
        v[n] = f;
        n += 1;
    }
    let filas = &mut v[..n];
    match orden {
        Orden::Memoria => filas.sort_unstable_by(|a, b| b.pide.unwrap_or(0).cmp(&a.pide.unwrap_or(0)).then(a.pid.cmp(&b.pid))),
        Orden::Pid => filas.sort_unstable_by(|a, b| a.pid.cmp(&b.pid).then(a.tid.cmp(&b.tid))),
        Orden::Nombre => filas.sort_unstable_by(|a, b| a.nombre().cmp(b.nombre()).then(a.pid.cmp(&b.pid))),
    }
    n
}

// ===================================================================
//  Pintar
// ===================================================================

fn place(s: &[u8], dst: &mut [u8], n: &mut usize) {
    for &b in s {
        if *n < dst.len() {
            dst[*n] = b;
            *n += 1;
        }
    }
}

fn num(v: u64, dst: &mut [u8], n: &mut usize) {
    let mut d = [0u8; 10];
    let k = decimal(v, &mut d);
    place(&d[..k], dst, n);
}

/// Un numero con un decimal: `59.5`. Sin coma flotante, que aqui no hay.
fn with_decimal(whole: u64, tenths: u64, dst: &mut [u8], n: &mut usize) {
    num(whole, dst, n);
    place(b".", dst, n);
    num(tenths, dst, n);
}

/// Bytes a la escala que se lea.
fn tam(bytes: u64, dst: &mut [u8], n: &mut usize) {
    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;
    if bytes >= MIB {
        with_decimal(bytes / MIB, (bytes % MIB) * 10 / MIB, dst, n);
        place(b" MiB", dst, n);
    } else if bytes >= KIB {
        with_decimal(bytes / KIB, (bytes % KIB) * 10 / KIB, dst, n);
        place(b" KiB", dst, n);
    } else {
        num(bytes, dst, n);
        place(b" B", dst, n);
    }
}

/// Una barra `[####----]` de `width` casillas.
fn bar(used_one: u64, total: u64, width: usize, dst: &mut [u8], n: &mut usize) {
    place(b"[", dst, n);
    let full = if total == 0 { 0 } else { (used_one.min(total) * width as u64 / total) as usize };
    for i in 0..width {
        place(if i < full { b"#" } else { b"-" }, dst, n);
    }
    place(b"]", dst, n);
}

/// Una fila `etiqueta   valor`, con la etiqueta a ancho fijo.
fn row(p: &bmo::Pantalla, x: u32, y: u32, etiq: &str, b: &[u8], ink: u32) {
    p.texto(x, y, etiq, INK_DIM);
    if let Ok(s) = core::str::from_utf8(b) {
        p.texto(x + 150, y, s, ink);
    }
}

/// **Una grafica**: una barra por muestra, la mas nueva a la derecha y
/// encendida; `techo` arriba (0: el maximo que se vio).
fn grafica(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, s: &Serie<MUESTRAS>, techo: u64, color: u32, rotulo: &[u8]) {
    p.rect(x, y, w, h, VIT_BG);
    p.texto_bytes(x, y, rotulo, INK_DIM);
    let gy = y + bmo::GLIFO_ALTO + 4;
    let gh = h.saturating_sub(bmo::GLIFO_ALTO + 4);
    p.rect(x, gy + gh, w, 1, VIT_EDGE);
    let mut alturas = [0u32; MUESTRAS];
    let n = s.alturas(gh, techo, &mut alturas);
    let paso = (w / MUESTRAS as u32).max(2);
    let ancho = paso.saturating_sub(1).max(1);
    let x0 = x + w.saturating_sub(paso * n as u32);
    for (k, &a) in alturas[..n].iter().enumerate() {
        let a = if s.muestra(s.cuantas() - n + k).unwrap_or(0) > 0 { a.max(1) } else { 0 };
        let c = if k + 1 == n { color } else { crate::scene::globo::mezcla(VIT_BG, color, 140) };
        p.rect(x0 + k as u32 * paso, gy + gh - a, ancho, a, c);
    }
}

/// `vueltas` son las del bucle del escritorio en el ultimo segundo entero, y
/// `consumo` lo ultimo que midio el UNICO lector del escritorio
/// (`Tick::consumo`): llegan hechos porque una VISTA no mide lo que ya mide
/// otro.
pub(crate) fn paint(p: &bmo::Pantalla, c: &VitalsWindow, vueltas: u32, consumo: Option<bmo_juicio::consumo::Consumo>) {
    if c.chrome.minimized {
        return;
    }
    c.chrome.paint_chrome(p, VIT_EDGE, VIT_BG, VIT_TITLE_BG, VIT_CYAN);
    c.chrome.paint_buttons(p, VIT_TITLE_BG);

    let tx = c.chrome.x + 16;
    p.rect(tx, c.chrome.y + 9, 8, 8, VIT_CYAN);
    let px = p.texto(tx + 16, c.chrome.y + 8, "VITALES", INK);
    p.texto(
        px + 2 * bmo::GLIFO_ANCHO,
        c.chrome.y + 8,
        match c.solapa {
            Solapa::Cpu => "a que va y que gasta",
            Solapa::Memoria => "quien se la esta comiendo",
            Solapa::Procesos => "lo que corre, y su boton de finalizar",
        },
        VIT_CYAN_DIM,
    );

    // -- Las solapas: la de ahora encendida y subrayada --
    let py = c.y_solapas();
    for (k, t) in Solapa::TODAS.iter().enumerate() {
        let (a, b) = c.x_solapas()[k];
        let sel = *t == c.solapa;
        if sel {
            p.rect(a, py - 2, b - a, bmo::GLIFO_ALTO + 4, C_ELEGIDO);
            p.rect(a, py + bmo::GLIFO_ALTO + 2, b - a, 2, VIT_CYAN);
        }
        p.texto(a, py, t.rotulo(), if sel { VIT_CYAN } else { VIT_CYAN_DIM });
    }
    p.rect(tx, py + bmo::GLIFO_ALTO + 6, c.chrome.width.saturating_sub(32), 1, VIT_EDGE);

    let mut y = c.y_contenido();
    let step = bmo::GLIFO_ALTO + 4;
    let mut b = [0u8; 120];
    match c.solapa {
        Solapa::Cpu => paint_cpu(p, c, tx, &mut y, step, &mut b, vueltas, consumo),
        Solapa::Memoria => paint_memory(p, c, tx, &mut y, step, &mut b),
        Solapa::Procesos => paint_procesos(p, c, tx, y),
    }

    // La barra de abajo dice lo unico que hay que saber para usarla.
    let by = c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 8;
    if c.aviso_n > 0 {
        p.texto_bytes(tx, by - step, &c.aviso[..c.aviso_n], VIT_CYAN);
    }
    p.texto(
        tx,
        by,
        match c.solapa {
            Solapa::Procesos => "flechas elegir   O ordenar   F finalizar   1 2 3 / Tab solapa   ESC cierra",
            _ => "1 2 3 / Tab solapa   F7 CPU   F8 memoria   se repinta sola   ESC cierra",
        },
        INK_DIM,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_cpu(p: &bmo::Pantalla, c: &VitalsWindow, tx: u32, y: &mut u32, step: u32, b: &mut [u8; 120], vueltas: u32, consumo: Option<bmo_juicio::consumo::Consumo>) {
    // ** LO PRIMERO ES QUE SABE MEDIR, y no una medida: las filas de abajo
    // pueden salir vacias por dos motivos que se ven igual.
    let sensors = bmo::info(bmo::INFO_CPU_SENSORES);
    let mut n = 0usize;
    if sensors == 0 {
        place(b"nada: el perfil no declara sensores", b, &mut n);
    } else {
        if sensors & 1 != 0 {
            place(b"frecuencia", b, &mut n);
        }
        if sensors & 3 == 3 {
            place(b" + ", b, &mut n);
        }
        if sensors & 2 != 0 {
            place(b"consumo", b, &mut n);
        }
    }
    row(p, tx, *y, "mide", &b[..n], INK_DIM);
    *y += step;

    let mut n = 0usize;
    match c.cpu.ultima() {
        Some(v) => {
            num(v, b, &mut n);
            place(b" %  del nucleo de arranque (1 - reposo / tiempo)", b, &mut n);
        }
        None => place(b"-- (aun sin dos lecturas)", b, &mut n),
    }
    row(p, tx, *y, "ocupado", &b[..n], INK);
    *y += step;

    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let actual = consumo.map_or(0, |m| m.hz_nucleo);
    let mut n = 0usize;
    if actual == 0 {
        place(b"-- (aun sin dos lecturas)", b, &mut n);
    } else {
        with_decimal(actual / 1_000_000_000, (actual % 1_000_000_000) / 100_000_000, b, &mut n);
        place(b" GHz   de ", b, &mut n);
        with_decimal(hz / 1_000_000_000, (hz % 1_000_000_000) / 100_000_000, b, &mut n);
        place(b" base", b, &mut n);
    }
    row(p, tx, *y, "va a", &b[..n], if actual > hz { INK_OK } else { INK });
    *y += step;

    let mw = consumo.map_or(0, |m| m.mw_paquete);
    let mut n = 0usize;
    if mw == 0 {
        place(b"-- (sin RAPL)", b, &mut n);
    } else {
        with_decimal(mw / 1000, (mw % 1000) / 100, b, &mut n);
        place(b" W el paquete entero", b, &mut n);
    }
    row(p, tx, *y, "gasta", &b[..n], INK);
    *y += step;

    let alive_count = bmo::info(bmo::INFO_SMP_VIVOS);
    let mut n = 0usize;
    if alive_count == 0 {
        place(b"solo el BSP    (`smp all` levanta los demas)", b, &mut n);
    } else {
        num(alive_count + 1, b, &mut n);
        place(b" en pie de ", b, &mut n);
        num(bmo::info(bmo::INFO_CPU_HILOS), b, &mut n);
    }
    row(p, tx, *y, "nucleos", &b[..n], if alive_count == 0 { INK_DIM } else { INK_OK });
    *y += step;

    let mut n = 0usize;
    num(bmo::info(bmo::INFO_TAREAS_TOTAL), b, &mut n);
    place(b" tareas, ", b, &mut n);
    num(bmo::info(bmo::INFO_TAREAS_LISTAS), b, &mut n);
    place(b" listas", b, &mut n);
    row(p, tx, *y, "planificador", &b[..n], INK);
    *y += step;

    // LA VUELTA DEL ESCRITORIO: la cifra que dice si el lento es el
    // escritorio (ver `scene::double_click`).
    let mut n = 0usize;
    if vueltas == 0 && hz == 0 {
        place(b"-- (sin reloj de referencia)", b, &mut n);
    } else if vueltas == 0 {
        place(b"-- (aun sin un segundo entero)", b, &mut n);
    } else {
        num(vueltas as u64, b, &mut n);
        place(b" vueltas/s   del bucle, no fotogramas", b, &mut n);
    }
    row(p, tx, *y, "escritorio", &b[..n], if vueltas == 0 { INK_DIM } else { INK });
    *y += step + 10;

    // -- Las graficas: la CPU contra 100, los vatios contra lo mas alto --
    let w = c.chrome.width.saturating_sub(32);
    let pie = c.chrome.y + c.chrome.height - 2 * (bmo::GLIFO_ALTO + 4) - 12;
    let alto = (pie.saturating_sub(*y) / 2).saturating_sub(6).min(90);
    if alto >= 30 {
        grafica(p, tx, *y, w, alto, &c.cpu, 100, C_CPU, b"cpu % -- los ultimos 15 s");
        *y += alto + 6;
        grafica(p, tx, *y, w, alto, &c.vatios, 0, C_VATIOS, b"vatios del paquete -- los ultimos 15 s");
    }
}

fn paint_memory(p: &bmo::Pantalla, c: &VitalsWindow, tx: u32, y: &mut u32, step: u32, b: &mut [u8; 120]) {
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    let used = total.saturating_sub(libre);

    let mut n = 0usize;
    tam(used, b, &mut n);
    place(b" de ", b, &mut n);
    tam(total, b, &mut n);
    place(b"  ", b, &mut n);
    bar(used, total, 24, b, &mut n);
    row(p, tx, *y, "usada", &b[..n], INK);
    *y += step + 8;

    // ** LA MEMORIA POR QUIEN LA TIENE, y que CUADRE. Lo que cada programa pidio por
    // KIND_MEMORIA sumaba 129 MiB de 2518 en el metal (02-10): el resto no
    // era de nadie que se dijera. Aqui va cada parte, y la que el kernel aun
    // no apunta por proceso se dice con su nombre en vez de callarse.
    let kernel = bmo::info(bmo::INFO_KERNEL_BYTES);
    let mut programas = 0u64;
    let mut ranuras = 0u64;
    while ranuras < 16 {
        let pid = bmo::info(bmo::INFO_MEM_QUIEN_PID | (ranuras << 8));
        if pid == 0 {
            break;
        }
        programas += bmo::info(bmo::INFO_MEM_QUIEN_BYTES | (ranuras << 8));
        ranuras += 1;
    }
    let sin_quien = used.saturating_sub(kernel).saturating_sub(programas);
    for (etiq, bytes, nota, ink) in [
        ("kernel", kernel, " su codigo y sus datos", INK),
        ("programas", programas, " lo que pidieron (la tabla de abajo)", INK_OK),
        ("sin decir de quien", sin_quien, " ventana de reserva (VirtualAlloc de PROTON-X), tablas, buferes", C_VATIOS),
        ("libre", libre, "", INK_DIM),
    ] {
        let mut n = 0usize;
        tam(bytes, b, &mut n);
        for _ in n..14 {
            place(b" ", b, &mut n);
        }
        bar(bytes, total, 16, b, &mut n);
        place(nota.as_bytes(), b, &mut n);
        row(p, tx, *y, etiq, &b[..n], ink);
        *y += step;
    }
    *y += 8;

    // -- QUIEN: cada pid con su NOMBRE --
    p.texto(tx, *y, "pid", INK_DIM);
    p.texto(tx + 60, *y, "programa", INK_DIM);
    p.texto(tx + 340, *y, "pedido", INK_DIM);
    p.texto(tx + 470, *y, "veces", INK_DIM);
    *y += step;
    let mut fila = 0u64;
    let fin = c.chrome.y + c.chrome.height - 2 * (bmo::GLIFO_ALTO + 4) - 100;
    while fila < 16 && *y < fin {
        let campo = |base: u64| base | (fila << 8);
        let pid = bmo::info(campo(bmo::INFO_MEM_QUIEN_PID));
        if pid == 0 {
            break;
        }
        let bytes = bmo::info(campo(bmo::INFO_MEM_QUIEN_BYTES));
        let veces = bmo::info(campo(bmo::INFO_MEM_QUIEN_PETICIONES));
        let mut n = 0usize;
        num(pid, b, &mut n);
        p.texto_bytes(tx, *y, &b[..n], INK);
        let mut nom = [0u8; 40];
        let k = nombre_de(pid, &mut nom);
        p.texto_bytes(tx + 60, *y, if k == 0 { b"(sin ficha)" } else { &nom[..k] }, if k == 0 { INK_DIM } else { INK });
        let mut n = 0usize;
        tam(bytes, b, &mut n);
        p.texto_bytes(tx + 340, *y, &b[..n], INK);
        let mut n = 0usize;
        num(veces, b, &mut n);
        place(b" de 8", b, &mut n);
        // Ambar al acercarse al tope: la novena peticion la niega el kernel.
        p.texto_bytes(tx + 470, *y, &b[..n], if veces >= 7 { INK_BAD } else { INK_DIM });
        *y += step;
        fila += 1;
    }
    if fila == 0 {
        p.texto(tx, *y, "nadie ha pedido memoria todavia", INK_DIM);
        *y += step;
    }
    *y += 8;

    let w = c.chrome.width.saturating_sub(32);
    let pie = c.chrome.y + c.chrome.height - 2 * (bmo::GLIFO_ALTO + 4) - 12;
    let alto = pie.saturating_sub(*y).min(90);
    if alto >= 30 {
        grafica(p, tx, *y, w, alto, &c.memoria, total / (1024 * 1024), C_MEM, b"memoria usada (MiB) -- los ultimos 15 s, contra el total");
    }
}

/// El alto de una fila de la tabla de PROCESOS.
const FILA_TABLA: u32 = bmo::GLIFO_ALTO + 10;

fn paint_procesos(p: &bmo::Pantalla, c: &VitalsWindow, tx: u32, y: u32) {
    let cols: [(&str, u32); 6] = [("PROGRAMA", 0), ("PID", 34), ("TID", 41), ("PIDE", 48), ("MAPEADO", 60), ("ESTADO", 72)];
    let col = |k: usize| tx + 12 + cols[k].1 * bmo::GLIFO_ANCHO;
    for (k, (t, _)) in cols.iter().enumerate() {
        p.texto(col(k), y, t, INK_DIM);
    }
    let mut o = [0u8; 40];
    let mut n = 0usize;
    place(b"orden: ", &mut o, &mut n);
    place(c.orden.nombre().as_bytes(), &mut o, &mut n);
    let ancho = c.chrome.width.saturating_sub(32);
    p.texto_bytes(tx + ancho.saturating_sub(n as u32 * bmo::GLIFO_ANCHO), y, &o[..n], VIT_CYAN_DIM);
    let mut y = y + FILA_TABLA;
    p.rect(tx, y, ancho, 1, VIT_EDGE);
    y += 6;
    let mut v = [FILA_VACIA; FICHAS];
    let total = filas(c.orden, &mut v);
    if total == 0 {
        p.texto(tx, y + 4, "ningun programa lanzado desde el arranque", INK_DIM);
        return;
    }
    let mut b = [0u8; 24];
    for (k, f) in v[..total].iter().enumerate() {
        let ty = y + 5;
        if k == c.elegido.min(total - 1) {
            p.rect(tx, y, ancho, FILA_TABLA, C_ELEGIDO);
            p.rect(tx, y, 3, FILA_TABLA, VIT_CYAN);
        }
        p.texto_bytes(col(0), ty, &f.nombre()[..f.nombre().len().min(32)], INK);
        let mut n = 0usize;
        num(f.pid, &mut b, &mut n);
        p.texto_bytes(col(1), ty, &b[..n], INK);
        let mut n = 0usize;
        num(f.tid as u64, &mut b, &mut n);
        p.texto_bytes(col(2), ty, &b[..n], INK_DIM);
        let mut n = 0usize;
        match f.pide {
            Some(bytes) => tam(bytes, &mut b, &mut n),
            None => place(b"-", &mut b, &mut n),
        }
        p.texto_bytes(col(3), ty, &b[..n], INK);
        let mut n = 0usize;
        tam(f.mapeado, &mut b, &mut n);
        p.texto_bytes(col(4), ty, &b[..n], INK_DIM);
        let (estado, ink) = match f.vive {
            Some(true) => ("vive", INK_OK),
            Some(false) => ("acabo", INK_DIM),
            None => ("no es del escritorio", INK_DIM),
        };
        p.texto(col(5), ty, estado, ink);
        y += FILA_TABLA;
    }
    y += 8;
    p.texto(tx, y, "F finaliza el elegido si lo lanzo el escritorio y vive (como el administrador de tareas).", INK_DIM);
    p.texto(tx, y + bmo::GLIFO_ALTO + 4, "La tabla son las 8 fichas del kernel: los que ya acabaron siguen hasta que llega otro.", INK_DIM);
}
