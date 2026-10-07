//! **LOS VITALES COMO INSTRUMENTOS** (HM4 de `docs/plan/PLAN_EL_HUD.md`, 07-10):
//! con el escritorio de mision, cada solapa de VITALES lleva arriba su
//! instrumento, el de `docs/arte/maqueta_hud_nasa.html`:
//!
//! ```text
//!    F7 CPU       PROPULSION       un motor por hilo (como esta: maestro,
//!                                  obrero, dormido, ausente) y su llama si
//!                                  esta despierto; el EMPUJE (la CPU, %) y
//!                                  la POTENCIA (el paquete, W)
//!    F8 MEMORIA   SOPORTE VITAL    un tanque por quien la tiene: el kernel,
//!                                  los programas, lo que nadie apunta y lo
//!                                  libre, llenos a su parte del total
//!    F6 RED       ENLACE           la onda de lo que llega (tramas/s, los
//!                                  ultimos 15 s), lo RECIBIDO y el enlace
//! ```
//!
//! [consumo] NADA      se pinta con la ventana, cada cuarto de segundo y
//!                     solo abierta y sin tapar (L6h); no mide nada propio
//!
//! ** Cada numero es uno que la ventana YA tiene (sus series) o que el kernel
//! ya contesta (`bmo::info`, `bmo::smp_hilo`): el instrumento no inventa una
//! medida. Lo que no se sabe (la carga de CADA hilo) no se dibuja como si se
//! supiera: el motor dice como esta, no cuanto empuja.
//!
//! Lo de debajo (las filas, el SMP con sus mandos, la tabla) sigue igual y
//! baja con la banda: sus zonas del raton salen de la misma cuenta
//! (`VitalsWindow::y_contenido`).

use bmo_userland as bmo;

use super::hud::{lectura, marco, miles};
use super::tema_gen::{MISION_BORDE, MISION_CUIDADO, MISION_FONDO, MISION_GO, MISION_NEON, MISION_OJO, MISION_TENUE, MISION_TINTA};
use super::vitals::{Solapa, VitalsWindow};

/// Lo que ocupa la banda, con su aire de abajo.
pub(crate) const BANDA: u32 = 150;
/// Por debajo de este alto de ventana no cabe: la ventana es la de siempre.
const ALTO_MINIMO: u32 = 560;

/// **Se pinta la banda en esta ventana?** Con el escritorio de mision, en CPU,
/// MEMORIA y RED, y si la ventana tiene alto para ella.
pub(crate) fn se_ve(c: &VitalsWindow) -> bool {
    super::fondo::es_mision() && c.solapa != Solapa::Procesos && c.chrome.height >= ALTO_MINIMO
}

/// **Pinta la banda** en `(x, y)` con ancho `w`.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &VitalsWindow, x: u32, y: u32, w: u32, consumo: Option<bmo_juicio::consumo::Consumo>) {
    let caja = (x, y, w, BANDA - 14);
    match c.solapa {
        Solapa::Cpu => propulsion(p, c, caja, consumo),
        Solapa::Memoria => soporte_vital(p, caja),
        Solapa::Red => enlace(p, c, caja),
        Solapa::Procesos => {}
    }
}

/// F7: un motor por hilo, y el empuje y la potencia.
fn propulsion(p: &bmo::Pantalla, c: &VitalsWindow, caja: (u32, u32, u32, u32), consumo: Option<bmo_juicio::consumo::Consumo>) {
    let hilos = (bmo::info(bmo::INFO_CPU_HILOS) as usize).clamp(1, 64);
    let mut estados = [3u32; 64];
    for (id, e) in estados.iter_mut().enumerate().take(hilos) {
        *e = bmo::smp_hilo(id as u32).0;
    }
    let mw = consumo.map(|m| m.mw_paquete).filter(|&v| v > 0);
    propulsion_con(p, caja, c.cpu.ultima().unwrap_or(0), mw, &estados[..hilos]);
}

/// La pintura de F7, con lo medido ya en la mano (la usa tambien el banco
/// del anfitrion, donde no hay kernel).
pub(crate) fn propulsion_con(p: &bmo::Pantalla, caja: (u32, u32, u32, u32), empuje: u64, mw: Option<u64>, estados: &[u32]) {
    let (x, y, w, h) = caja;
    marco(p, caja, b"PROPULSION", b"F7");
    let mut b = [0u8; 24];
    let n = miles(empuje, &mut b);
    lectura(p, x + 16, y + 30, b"EMPUJE", &b[..n], b"%", MISION_OJO);
    match mw {
        Some(mw) => {
            let mut v = [0u8; 24];
            let n = miles(mw / 1000, &mut b);
            v[..n].copy_from_slice(&b[..n]);
            v[n] = b',';
            v[n + 1] = b'0' + ((mw % 1000) / 100) as u8;
            lectura(p, x + 150, y + 30, b"POTENCIA", &v[..n + 2], b"W", MISION_NEON);
        }
        None => lectura(p, x + 150, y + 30, b"POTENCIA", b"--", b"sin sensor", MISION_TENUE),
    }
    // Los motores, de izquierda a derecha, tantos como quepan.
    let hilos = estados.len() as u32;
    let (x0, paso) = (x + 300, 34u32);
    let caben = (x + w).saturating_sub(x0 + 12) / paso;
    let chica = bmo::Estilo::normal(10);
    for id in 0..hilos.min(caben) {
        let (cuerpo, llama) = match estados[id as usize] {
            0 => (MISION_OJO, true),
            1 => (MISION_GO, true),
            2 => (MISION_CUIDADO, false),
            _ => (MISION_BORDE, false),
        };
        let mx = x0 + id * paso;
        let my = y + 40;
        // La tobera: un cuerpo con su borde, mas ancho abajo.
        p.rect(mx + 4, my, 20, 30, MISION_BORDE);
        p.rect(mx + 5, my + 1, 18, 28, MISION_FONDO);
        p.rect(mx + 7, my + 4, 14, 14, cuerpo);
        p.rect(mx + 1, my + 30, 26, 4, MISION_BORDE);
        if llama {
            // La llama: tres escalones que se afinan hacia abajo.
            p.rect(mx + 7, my + 35, 14, 6, MISION_NEON);
            p.rect(mx + 9, my + 41, 10, 6, MISION_CUIDADO);
            p.rect(mx + 12, my + 47, 4, 5, MISION_TINTA);
        }
        let n = miles(id as u64, &mut b);
        p.letra((mx + 8) as i32, (my + h.saturating_sub(48)) as i32, &b[..n], MISION_TENUE, chica);
    }
    if hilos > caben {
        p.letra((x0 + caben * paso) as i32, (y + 70) as i32, b"...", MISION_TENUE, chica);
    }
}

/// F8: un tanque por quien tiene la memoria.
fn soporte_vital(p: &bmo::Pantalla, caja: (u32, u32, u32, u32)) {
    let total = bmo::info(bmo::INFO_RAM_TOTAL).max(1);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    let usada = total.saturating_sub(libre);
    let kernel = bmo::info(bmo::INFO_KERNEL_BYTES);
    let mut programas = 0u64;
    for ranura in 0..16u64 {
        if bmo::info(bmo::INFO_MEM_QUIEN_PID | (ranura << 8)) == 0 {
            break;
        }
        programas += bmo::info(bmo::INFO_MEM_QUIEN_BYTES | (ranura << 8));
    }
    let sin_quien = usada.saturating_sub(kernel).saturating_sub(programas);
    soporte_vital_con(p, caja, total, [kernel, programas, sin_quien, libre]);
}

/// La pintura de F8: `partes` son el kernel, los programas, lo que nadie
/// apunta y lo libre.
pub(crate) fn soporte_vital_con(p: &bmo::Pantalla, caja: (u32, u32, u32, u32), total: u64, partes: [u64; 4]) {
    let (x, y, w, _) = caja;
    marco(p, caja, b"SOPORTE VITAL", b"F8");
    let total = total.max(1);
    let [kernel, programas, sin_quien, libre] = partes;
    let usada = total.saturating_sub(libre);
    let mut b = [0u8; 24];
    let n = miles(usada / (1024 * 1024), &mut b);
    lectura(p, x + 16, y + 30, b"EN USO", &b[..n], b"MiB", MISION_OJO);
    let tanques: [(&[u8], u64, u32); 4] = [(b"KERNEL", kernel, MISION_OJO), (b"PROGRAMAS", programas, MISION_GO), (b"SIN DECIR", sin_quien, MISION_CUIDADO), (b"LIBRE", libre, MISION_TENUE)];
    let (x0, paso) = (x + 220, ((w.saturating_sub(240)) / 4).clamp(70, 150));
    let (alto, ancho) = (78u32, 34u32);
    let chica = bmo::Estilo::normal(10).espaciado(120);
    for (k, (nombre, bytes, tinta)) in tanques.iter().enumerate() {
        let tx = x0 + k as u32 * paso;
        let ty = y + 26;
        p.borde_redondo(tx as i32, ty as i32, ancho as i32, alto as i32, (ancho / 2) as i32, 1, MISION_BORDE);
        // Lleno a su parte del total, desde abajo (la parte que se ve: al
        // menos un pixel si tiene algo).
        let lleno = ((alto - 6) as u64 * bytes / total) as u32;
        let lleno = if *bytes > 0 { lleno.max(2) } else { 0 };
        if lleno > 0 {
            let r = (lleno / 2).min((ancho - 6) / 2);
            p.caja_redonda((tx + 3) as i32, (ty + alto - 3 - lleno) as i32, (ancho - 6) as i32, lleno as i32, r as i32, *tinta);
        }
        p.letra((tx + ancho + 8) as i32, (ty + 20) as i32, nombre, MISION_TINTA, chica);
        let pct = bytes * 100 / total;
        let mut v = [0u8; 24];
        let n = miles(pct, &mut v);
        v[n] = b' ';
        v[n + 1] = b'%';
        p.letra((tx + ancho + 8) as i32, (ty + 38) as i32, &v[..n + 2], *tinta, chica);
    }
}

/// F6: la onda de lo que llega, lo recibido y el enlace.
fn enlace(p: &bmo::Pantalla, c: &VitalsWindow, caja: (u32, u32, u32, u32)) {
    enlace_con(p, caja, &c.red, bmo::info(bmo::INFO_NET_MEGABITS));
}

/// La pintura de F6, con la serie de tramas por segundo y el enlace.
pub(crate) fn enlace_con(p: &bmo::Pantalla, caja: (u32, u32, u32, u32), red: &bmo_registro::Serie<{ super::vitals::MUESTRAS }>, mbit: u64) {
    let (x, y, w, h) = caja;
    marco(p, caja, b"ENLACE DE TELEMETRIA", b"F6");
    let mut b = [0u8; 24];
    let n = miles(red.ultima().unwrap_or(0), &mut b);
    lectura(p, x + 16, y + 30, b"RECIBIDO", &b[..n], b"tramas/s", MISION_OJO);
    if mbit > 0 {
        let n = miles(mbit, &mut b);
        lectura(p, x + 16, y + 76, b"ENLACE", &b[..n], b"Mbit", MISION_GO);
    } else {
        lectura(p, x + 16, y + 76, b"ENLACE", b"--", b"sin cable", MISION_CUIDADO);
    }
    // La onda: una linea que une las muestras, a lo ancho de lo que queda.
    let (ox, oy, ow, oh) = (x + 220, y + 30, w.saturating_sub(240), h.saturating_sub(44));
    if ow < 40 || oh < 20 {
        return;
    }
    p.rect(ox, oy + oh, ow, 1, MISION_BORDE);
    let mut alturas = [0u32; super::vitals::MUESTRAS];
    let techo = red.maximo().max(1);
    let k = red.alturas(oh, techo, &mut alturas);
    if k < 2 {
        return;
    }
    let paso = ow / (k as u32 - 1).max(1);
    let mut antes = oy + oh - alturas[0];
    for (i, &a) in alturas[..k].iter().enumerate().skip(1) {
        let ahora = oy + oh - a;
        let (x1, x2) = (ox + (i as u32 - 1) * paso, ox + i as u32 * paso);
        // Un tramo: lo horizontal a la altura de antes y lo vertical hasta la de ahora.
        p.rect(x1, antes, x2 - x1, 2, MISION_OJO);
        let (a0, a1) = (antes.min(ahora), antes.max(ahora));
        p.rect(x2, a0, 2, a1 - a0 + 2, MISION_OJO);
        antes = ahora;
    }
}
