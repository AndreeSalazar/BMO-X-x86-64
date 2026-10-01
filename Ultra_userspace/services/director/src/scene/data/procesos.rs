//! **PROCESOS** -- lo que corre y a que velocidad, EN VIVO (2026-10-01).
//!
//! [consumo] LATE      dos muestras por segundo SOLO con la solapa a la vista
//!                     y la ventana sin tapar; en otra solapa, NADA (L6h)
//!
//! El propietario, con la maqueta animada de ESTRATOS delante: *"que tengan su
//! benchmark en tiempo real como se ejecutan, en velocidad"*. Cuatro medidores
//! con su grafica de barras y la tabla de los programas:
//!
//! ```text
//!    CPU       ocupacion del nucleo de arranque: 1 - reposo / tiempo
//!              (`INFO_BSP_TICKS_REPOSO`, la misma cuenta que el panel)
//!    MEMORIA   la RAM usada: total - libre
//!    LEE       MiB/s leidos de TODOS los discos  (`INFO_DISCO_LEIDO`)
//!    ESCRIBE   MiB/s escritos                    (`INFO_DISCO_ESCRITO`)
//! ```
//!
//! ** Un MiB/s es una RESTA: dos lecturas del contador y el tiempo entre ellas
//! por el TSC. Nada aqui lee el disco para medirlo -- medir leyendo seria
//! medir la medida. Lo que no se mide por programa (la CPU de cada uno) no se
//! inventa: la tabla dice lo que el kernel sabe de cada ficha.

use bmo_userland as bmo;
use core::ptr::{addr_of, addr_of_mut};

use super::*;
use crate::scene::zonas::Zona;
use crate::text::decimal;

/// Muestras en la grafica de cada medidor: veinte segundos a dos por segundo.
const HISTORIA: usize = 40;
/// Fichas de programa que se miran (las del kernel son 8; se pide de mas y el
/// 0 corta).
const FICHAS: u64 = 12;

#[derive(Clone, Copy)]
struct Medidor {
    /// Lo ultimo medido, en la unidad de `historia`.
    ahora: u32,
    historia: [u32; HISTORIA],
}

const MEDIDOR_VACIO: Medidor = Medidor { ahora: 0, historia: [0; HISTORIA] };

struct Estado {
    proximo: u64,
    tsc: u64,
    reposo: u64,
    leido: u64,
    escrito: u64,
    /// CPU en %, memoria en MiB, disco en KiB/s.
    cpu: Medidor,
    memoria: Medidor,
    lee: Medidor,
    escribe: Medidor,
    muestras: u64,
}

static mut ESTADO: Estado = Estado {
    proximo: 0,
    tsc: 0,
    reposo: 0,
    leido: 0,
    escrito: 0,
    cpu: MEDIDOR_VACIO,
    memoria: MEDIDOR_VACIO,
    lee: MEDIDOR_VACIO,
    escribe: MEDIDOR_VACIO,
    muestras: 0,
};

fn estado() -> &'static mut Estado {
    // SAFETY: el director es una tarea; nadie mas toca esto.
    unsafe { &mut *addr_of_mut!(ESTADO) }
}

fn empujar(m: &mut Medidor, v: u32) {
    m.historia.copy_within(1.., 0);
    m.historia[HISTORIA - 1] = v;
    m.ahora = v;
}

/// **Toma una muestra si toca** (cada medio segundo). `true` si la tomo: quien
/// llama repinta.
pub(crate) fn muestrear() -> bool {
    let e = estado();
    let ahora = bmo::ciclos();
    if ahora < e.proximo {
        return false;
    }
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    e.proximo = ahora + hz / 2;
    let reposo = bmo::info(bmo::INFO_BSP_TICKS_REPOSO);
    let leido = bmo::info(bmo::INFO_DISCO_LEIDO);
    let escrito = bmo::info(bmo::INFO_DISCO_ESCRITO);
    if e.tsc != 0 && ahora > e.tsc {
        let dt = ahora - e.tsc;
        let dr = reposo.saturating_sub(e.reposo).min(dt);
        empujar(&mut e.cpu, (100 - dr * 100 / dt) as u32);
        // KiB/s = bytes * hz / dt / 1024, sin desbordar: dt en ms primero.
        let ms = (dt * 1000 / hz).max(1);
        let kib = |b: u64| (b.saturating_mul(1000) / ms / 1024).min(u32::MAX as u64) as u32;
        empujar(&mut e.lee, kib(leido.saturating_sub(e.leido)));
        empujar(&mut e.escribe, kib(escrito.saturating_sub(e.escrito)));
    }
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let usada = total.saturating_sub(bmo::info(bmo::INFO_RAM_LIBRE)) / (1024 * 1024);
    empujar(&mut e.memoria, usada as u32);
    e.tsc = ahora;
    e.reposo = reposo;
    e.leido = leido;
    e.escrito = escrito;
    e.muestras += 1;
    true
}

/// Al ENTRAR en la solapa: la siguiente vuelta muestrea ya.
pub(crate) fn entrar() {
    estado().proximo = 0;
}

// ===================================================================
//  Pintar
// ===================================================================

const GAP: u32 = 12;
const MEDIDOR_H: u32 = 132;
const FILA: u32 = 30;

/// Los colores de los cuatro: el neon de ESTRATOS para la CPU, el cian del gato
/// para la memoria, ambar y rosa para lo que entra y sale del disco.
const C_CPU: u32 = 0x0039_FF88;
const C_MEM: u32 = 0x005E_F2E6;
const C_LEE: u32 = 0x00F0_B060;
const C_ESCRIBE: u32 = 0x00FF_6FA8;

fn panel(p: &bmo::Pantalla, z: &Zona) {
    crate::scene::borde::marco(p, z.x, z.y, z.w, z.h, crate::scene::RADIUS, DATA_EDGE, NODE_BG);
    let r = crate::scene::RADIUS;
    p.rect(z.x + r, z.y, z.w.saturating_sub(2 * r), 1, DATA_TITLE);
}

/// `v` decimas como "12.3".
fn decimas(v: u64, dst: &mut [u8; 16]) -> usize {
    let mut b = [0u8; 10];
    let n = decimal(v / 10, &mut b);
    dst[..n].copy_from_slice(&b[..n]);
    dst[n] = b'.';
    dst[n + 1] = b'0' + (v % 10) as u8;
    n + 2
}

/// Un medidor: el nombre, la cifra grande y la grafica de barras.
fn medidor(p: &bmo::Pantalla, z: &Zona, nombre: &str, cifra: &[u8], unidad: &str, m: &Medidor, tope: u32, color: u32) {
    panel(p, z);
    p.texto(z.x + 14, z.y + 12, nombre, INK_DIM);
    let x = p.texto_escala(z.x + 14, z.y + 32, core::str::from_utf8(cifra).unwrap_or("?"), color, 2);
    p.texto(x + 6, z.y + 32 + bmo::GLIFO_ALTO, unidad, INK_DIM);
    // La grafica: una barra por muestra, la mas nueva a la derecha.
    let gx = z.x + 14;
    let gw = z.w.saturating_sub(28);
    let gh = 40u32;
    let gy = z.y + z.h - gh - 12;
    p.rect(gx, gy + gh, gw, 1, DATA_EDGE);
    let paso = (gw / HISTORIA as u32).max(2);
    let ancho = paso.saturating_sub(1).max(1);
    let tope = tope.max(1);
    let x0 = gx + gw.saturating_sub(paso * HISTORIA as u32);
    for (k, &v) in m.historia.iter().enumerate() {
        let h = ((v.min(tope) as u64 * gh as u64) / tope as u64) as u32;
        let h = if v > 0 { h.max(1) } else { 0 };
        let c = if k == HISTORIA - 1 { color } else { crate::scene::globo::mezcla(NODE_BG, color, 150) };
        p.rect(x0 + k as u32 * paso, gy + gh - h, ancho, h, c);
    }
}

/// El tope de una grafica: lo mas alto que se vio, redondeado arriba, para
/// que la barra mas alta no toque siempre el techo ni se pierda abajo.
fn tope_de(m: &Medidor, minimo: u32) -> u32 {
    let mx = m.historia.iter().copied().max().unwrap_or(0).max(minimo);
    let mut t = 1u32;
    while t < mx {
        t = t.saturating_mul(2);
    }
    t
}

/// La tabla de los programas que el kernel tiene fichados.
fn tabla(p: &bmo::Pantalla, z: &Zona) {
    panel(p, z);
    let cols: [(&str, u32); 6] = [("PROGRAMA", 0), ("PID", 34), ("TID", 44), ("PIDE", 54), ("MAPEADO", 68), ("ESTADO", 82)];
    let col = |k: usize| z.x + 16 + cols[k].1 * bmo::GLIFO_ANCHO;
    let mut y = z.y + 12;
    for k in 0..cols.len() {
        p.texto(col(k), y, cols[k].0, INK_DIM);
    }
    y += bmo::GLIFO_ALTO + 8;
    p.rect(z.x + 10, y, z.w.saturating_sub(20), 1, DATA_TITLE);
    y += 6;
    let mut b = [0u8; 10];
    let mut n = 0u64;
    while n < FICHAS && y + FILA <= z.y + z.h {
        let quien = bmo::info(bmo::INFO_PROG_QUIEN | (n << 8));
        if quien == 0 {
            break;
        }
        let pid = quien & 0xFFFF;
        let imagen = bmo::info(bmo::INFO_PROG_IMAGEN | (n << 8));
        let ty = y + (FILA - bmo::GLIFO_ALTO) / 2;
        let admitido = quien >> 63 != 0;
        let color = if admitido { C_CPU } else { INK_BAD };
        p.rect(z.x + 12, y + 6, 3, FILA - 12, color);
        let mut nombre = [0u8; 30];
        let k = bmo::info_texto(bmo::INFO_TXT_PROG_NOMBRE | (n << 8), &mut nombre).min(30);
        p.texto_bytes(col(0) + 6, ty, &nombre[..k], INK);
        let k = decimal(pid, &mut b);
        p.texto_bytes(col(1), ty, &b[..k], INK);
        let k = decimal((quien >> 16) & 0xFFFF, &mut b);
        p.texto_bytes(col(2), ty, &b[..k], INK_DIM);
        // Lo que ese pid tiene PEDIDO ahora (las ranuras de memoria por pid).
        let mut pide = None;
        for r in 0..16u64 {
            let q = bmo::info(bmo::INFO_MEM_QUIEN_PID | (r << 8));
            if q == 0 {
                break;
            }
            if q == pid {
                pide = Some(bmo::info(bmo::INFO_MEM_QUIEN_BYTES | (r << 8)));
                break;
            }
        }
        match pide {
            Some(bytes) => {
                let k = decimal(bytes / (1024 * 1024), &mut b);
                let x = p.texto_bytes(col(3), ty, &b[..k], INK);
                p.texto(x + 4, ty, "MiB", INK_DIM);
            }
            None => {
                p.texto(col(3), ty, "-", INK_DIM);
            }
        }
        let k = decimal((imagen >> 32) / 1024, &mut b);
        let x = p.texto_bytes(col(4), ty, &b[..k], INK);
        p.texto(x + 4, ty, "KiB", INK_DIM);
        p.texto(col(5), ty, if admitido { "admitido" } else { "NO paso" }, color);
        y += FILA;
        n += 1;
    }
    if n == 0 {
        p.texto(z.x + 16, y + 6, "ningun programa lanzado desde el arranque", INK_DIM);
    }
}

/// **Pinta la solapa entera** en `z`.
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona) {
    if !z.hay() {
        return;
    }
    let e = unsafe { &*addr_of!(ESTADO) };
    let w4 = z.w.saturating_sub(5 * GAP) / 4;
    let caja = |k: u32| Zona { x: z.x + GAP + k * (w4 + GAP), y: z.y + GAP, w: w4, h: MEDIDOR_H };
    let mut t = [0u8; 16];
    let mut b = [0u8; 10];

    let n = decimal(e.cpu.ahora as u64, &mut b);
    medidor(p, &caja(0), "CPU", &b[..n], "%", &e.cpu, 100, C_CPU);
    let n = decimal(e.memoria.ahora as u64, &mut b);
    medidor(p, &caja(1), "MEMORIA", &b[..n], "MiB", &e.memoria, tope_de(&e.memoria, 64), C_MEM);
    // KiB/s -> decimas de MiB/s.
    let n = decimas(e.lee.ahora as u64 * 10 / 1024, &mut t);
    medidor(p, &caja(2), "DISCO LEE", &t[..n], "MiB/s", &e.lee, tope_de(&e.lee, 1024), C_LEE);
    let n = decimas(e.escribe.ahora as u64 * 10 / 1024, &mut t);
    medidor(p, &caja(3), "DISCO ESCRIBE", &t[..n], "MiB/s", &e.escribe, tope_de(&e.escribe, 1024), C_ESCRIBE);

    let ty = z.y + GAP + MEDIDOR_H + GAP;
    let resto = Zona { x: z.x + GAP, y: ty, w: z.w.saturating_sub(2 * GAP), h: (z.y + z.h).saturating_sub(ty + GAP) };
    if resto.h > 3 * FILA {
        tabla(p, &resto);
    }
}
