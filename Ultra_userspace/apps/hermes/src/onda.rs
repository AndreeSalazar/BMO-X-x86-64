//! **La ONDA** (seccion 5 de HERMES): el DISCO de la pieza que gira mientras
//! suena, su ficha, la vuelta del bucle y un ecualizador que sigue al medidor
//! del MAESTRO. Aparte de `pintar.rs` porque es la seccion mas grande.

use crate::canvas::Canvas;
use crate::mates::{coseno, fase, onda, seno};
use crate::piezas::{t_grande, ancho_txt, txt, txt_cabe};
use crate::pintar::{suelo, ancho_centro, llega, nivel, Vista, BLANCO, CABECERA, CIAN, FONDO, GRIS, LIMA, LINEA, NEGRO, PANEL2, ROSA, TENUE, TEXTO, AMBAR, AZUL, MORADO, VERDE};
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_fondo::{Estilo, PIEZAS};

/// El color de una pieza: el suyo en el disco, en la ficha y en las barras.
pub(crate) fn color_pieza(k: usize) -> Color {
    if PIEZAS[k % PIEZAS.len()].estilo == Estilo::NekoPhonk {
        return ROSA;
    }
    [CIAN, LIMA, AZUL, MORADO, AMBAR, VERDE, CIAN, LIMA, AZUL, MORADO][k % 10]
}

/// Lo que tarda una vuelta del bucle, en ms: ocho compases de cuatro pulsos
/// (`bmo_fondo::compositor::PASOS` son 128 semicorcheas).
pub(crate) fn vuelta_ms(bpm: u32) -> u32 {
    32 * 60_000 / bpm.max(1)
}

/// `m:ss` en `b`; devuelve cuantos bytes.
pub(crate) fn reloj(ms: u32, b: &mut [u8; 8]) -> usize {
    let s = ms / 1000;
    let mut n = crate::fmt_num((s / 60) as u64, &mut b[..]);
    b[n] = b':';
    b[n + 1] = b'0' + (s % 60 / 10) as u8;
    b[n + 2] = b'0' + (s % 10) as u8;
    n += 3;
    n
}

/// Los picos del ecualizador, que caen despacio: el unico estado de la ONDA
/// entre fotogramas (la app es un solo hilo).
static PICOS: [core::sync::atomic::AtomicI32; 48] = [const { core::sync::atomic::AtomicI32::new(0) }; 48];

/// **La ONDA**: el DISCO de la pieza que gira mientras suena, su ficha, la
/// vuelta del bucle y un ecualizador que sigue al medidor del MAESTRO.
pub(crate) fn la_onda(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho_centro() - 40;
    let k = v.pedida.unwrap_or(v.item) % PIEZAS.len();
    let p = &PIEZAS[k];
    let suena = v.pedida == Some(k) && v.sonando;
    let color = color_pieza(k);
    let entra = llega(v.ms, v.desde_item, 0, 450);
    let y = CABECERA + 24;

    // ** EL DISCO: surcos, el brillo que gira, la etiqueta y el agujero. Gira
    // a 33 vueltas por minuto mientras suena; parado, se queda donde estaba.
    const R: i32 = 96;
    let (cx, cy) = (x0 + 20 + R + 8, y + R + 12);
    let giro = if suena { fase(v.ms.wrapping_sub(v.pedida_desde), 1818) } else { 0 };
    cv.disc(cx + 4, cy + 6, R, mezclar(NEGRO, FONDO, 140, 256));
    cv.disc(cx, cy, R, 0x0007_080C);
    let mut r = R - 4;
    while r > 34 {
        let pasos = (r * 3).max(24);
        for q in 0..pasos {
            let a = q * 256 / pasos;
            // El brillo: dos lobulos opuestos que giran con el disco.
            let rel = (a - giro).rem_euclid(128);
            let luz = if rel < 22 { (22 - rel) * 7 } else { 0 } as u32;
            let base = if r % 8 == 0 { 0x0028_2E3C } else if r % 4 == 0 { 0x0018_1C26 } else { 0x0010_1219 };
            let c = mezclar(mezclar(color, BLANCO, 110, 256), base, luz.min(140), 256);
            cv.put(cx + coseno(a) * r / 256, cy + seno(a) * r / 256, c);
        }
        r -= 2;
    }
    cv.disc(cx, cy, 34, color);
    cv.disc(cx, cy, 30, mezclar(color, NEGRO, 60, 256));
    // Una marca en la etiqueta, para que se VEA girar.
    let (mx, my) = (cx + coseno(giro) * 22 / 256, cy + seno(giro) * 22 / 256);
    cv.disc(mx, my, 3, BLANCO);
    cv.disc(cx, cy, 4, FONDO);
    // El brazo: posado en el surco si suena, levantado si no.
    let (bx, by) = (cx + R + 22, cy - R + 6);
    let (px, py) = if suena { (cx + R / 2, cy + 8) } else { (cx + R + 6, cy + 30) };
    cv.disc(bx, by, 7, PANEL2);
    cv.disc(bx, by, 3, GRIS);
    for g in 0..3 {
        cv.line((bx + g - 1, by), (px + g - 1, py), GRIS);
    }
    cv.rect(px - 5, py - 3, 10, 7, if suena { color } else { TENUE });

    // ** LA FICHA, a la derecha del disco.
    let tx = cx + R + 48;
    let tw = (x0 + 20 + w - tx).max(80);
    let ty = y + 8 + (256 - entra) * 16 / 256;
    let etiqueta: &[u8] = if suena { b"AHORA SUENA" } else if v.pedida == Some(k) { b"PEDIDA" } else { b"ELIGE Y SUENA" };
    txt(cv, tx, ty, etiqueta, if suena { color } else { TENUE });
    let escala = if (p.nombre.len() as i32) * 24 <= tw { 3 } else { 2 };
    t_grande(cv, tx, ty + 22, p.nombre.as_bytes(), mezclar(BLANCO, FONDO, entra as u32, 256), escala);
    // Las fichas chicas: pulsos, timbre, escala, estilo.
    let mut d = [0u8; 12];
    let nd = crate::fmt_num(p.bpm as u64, &mut d);
    let timbre: &[u8] = match p.timbre {
        bmo_fondo::Timbre::Seno => b"seno",
        bmo_fondo::Timbre::Cuadrada => b"cuadrada 8 bits",
        bmo_fondo::Timbre::Sierra => b"sierra",
        bmo_fondo::Timbre::Triangulo => b"triangulo",
    };
    let modo: &[u8] = match p.escala {
        bmo_fondo::Escala::Frigia => b"frigia",
        bmo_fondo::Escala::Menor => b"menor",
        bmo_fondo::Escala::Mayor => b"mayor",
        bmo_fondo::Escala::Penta => b"pentatonica",
        bmo_fondo::Escala::Dorica => b"dorica",
    };
    let estilo: &[u8] = if p.estilo == Estilo::NekoPhonk { b"NEKO PHONK" } else { b"ambiente" };
    let mut fx = tx;
    let fy = ty + 22 + escala * 16 + 14;
    for (kk, ficha) in [&d[..nd], timbre, modo, estilo].iter().enumerate() {
        let fw = ancho_txt(ficha) + 16 + if kk == 0 { ancho_txt(b" pulsos") } else { 0 };
        if fx + fw > tx + tw {
            break;
        }
        cv.rect(fx, fy, fw, 22, mezclar(color, FONDO, 36, 256));
        cv.frame(fx, fy, fw, 22, 1, mezclar(color, FONDO, 120, 256));
        let fin = txt(cv, fx + 8, fy + 3, ficha, TEXTO);
        if kk == 0 {
            txt(cv, fx + 8 + fin, fy + 3, b" pulsos", TENUE);
        }
        fx += fw + 8;
    }
    // La vuelta del bucle: por donde va y cuanto mide (la musica de fondo
    // es un bucle SIN COSTURA: al acabar sigue, no se para).
    let total = vuelta_ms(p.bpm);
    let va = if suena { v.ms.wrapping_sub(v.pedida_desde) % total } else { 0 };
    let (ly, lw) = (fy + 40, tw.min(520));
    cv.rect(tx, ly, lw, 4, LINEA);
    cv.rect(tx, ly, (lw as u64 * va as u64 / total as u64) as i32, 4, color);
    if suena {
        let px = tx + (lw as u64 * va as u64 / total as u64) as i32;
        cv.disc(px, ly + 1, 5, BLANCO);
    }
    let mut b1 = [0u8; 8];
    let mut b2 = [0u8; 8];
    let (n1, n2) = (reloj(va, &mut b1), reloj(total, &mut b2));
    txt(cv, tx, ly + 12, &b1[..n1], TEXTO);
    txt(cv, tx + lw - ancho_txt(&b2[..n2]), ly + 12, &b2[..n2], TENUE);
    txt(cv, tx + ancho_txt(&b1[..n1]) + 16, ly + 12, b"en bucle, sin costura", TENUE);
    let estado: &[u8] = if suena {
        b"suena en el ESCRITORIO: sigue con HERMES cerrado"
    } else if v.pedida == Some(k) {
        b"pedida al escritorio: si no suena, `fondo` en Ejecutar dice por que"
    } else {
        b"un clic en la lista y suena: la compone y la toca el escritorio"
    };
    txt_cabe(cv, tx, ly + 34, estado, if suena { mezclar(color, TEXTO, 120, 256) } else { TENUE }, tw);

    // ** EL ECUALIZADOR: 48 barras finas en degradado. La fuerza es la del
    // medidor del MAESTRO (lo que sale por el cable), con una curva de
    // espectro (los graves arriba) y un baile propio de cada barra; los picos
    // caen despacio, y el suelo refleja.
    let ey = cy + R + 40;
    let eh = (suelo() - ey - 70).max(60);
    for q in 1..4 {
        cv.rect(x0 + 20, ey + eh * q / 4, w, 1, mezclar(LINEA, FONDO, 140, 256));
    }
    let barras = 48;
    let paso = (w / barras).max(4);
    let ancho_b = (paso - 3).max(2);
    let crece = llega(v.ms, v.desde_sec, 0, 600);
    for b in 0..barras {
        let lado = (b % 2) as usize;
        // Los graves (a la izquierda) suben mas que los agudos.
        let curva = 256 - b * 120 / barras;
        let fuerza = if v.sonando { nivel(v.rms[lado]) } else { 0 };
        let baila = if v.sonando { onda(v.ms + b as u32 * 71, 210 + (b as u32 % 9) * 37) } else { 0 };
        let a = ((fuerza * curva / 256 * 3 / 4 + baila * fuerza / 256 / 3) * eh / 256 * crece / 256).clamp(2, eh * 9 / 10);
        let bx = x0 + 20 + b * paso;
        let c = mezclar(color, CIAN, b as u32 * 256 / barras as u32, 256);
        // El degradado: cuatro tramos, mas oscuros abajo.
        for t in 0..4 {
            let (y0, y1) = (ey + eh - a * (t + 1) / 4, ey + eh - a * t / 4);
            cv.rect(bx, y0, ancho_b, y1 - y0, mezclar(c, FONDO, 256 - (3 - t as u32) * 45, 256));
        }
        cv.rect(bx, ey + eh - a, ancho_b, 2, mezclar(BLANCO, c, 110, 256));
        // El reflejo, que se apaga.
        for t in 0..3 {
            let h = (a / 10).max(1);
            cv.rect(bx, ey + eh + 3 + t * h, ancho_b, h, mezclar(c, FONDO, [60, 30, 12][t as usize], 256));
        }
        // El pico, que sube de golpe y cae despacio.
        let pk = &PICOS[b as usize];
        let antes = pk.load(core::sync::atomic::Ordering::Relaxed);
        // El pico sale del medidor (la punta, no la fuerza): mas alto que la barra.
        let punta = if v.sonando { (nivel(v.pico[lado]) * curva / 256 * 3 / 4 * eh / 256 * crece / 256).min(eh * 9 / 10) } else { 0 };
        let sube = a.max(punta);
        let ahora = if sube > antes { sube } else { (antes - 2).max(0) };
        pk.store(ahora, core::sync::atomic::Ordering::Relaxed);
        if v.sonando && ahora > a + 3 {
            cv.rect(bx, ey + eh - ahora, ancho_b, 2, mezclar(BLANCO, c, 160, 256));
        }
    }
}

/// **La portada chica** de una pieza, para su fila de la lista: un cuadro
/// redondo de su color con su ecualizador -- quieto (de su semilla) o
/// bailando si es la que suena.
pub(crate) fn portada(cv: &mut Canvas, x: i32, y: i32, lado: i32, k: usize, suena: bool, ms: u32) {
    let color = color_pieza(k);
    crate::piezas::redonda(cv, x, y, lado, lado, 8, mezclar(color, NEGRO, 46, 256));
    let barras = 5;
    let paso = (lado - 8) / barras;
    for b in 0..barras {
        let a = if suena {
            4 + onda(ms + b as u32 * 97, 300 + b as u32 * 70) * (lado - 14) / 256
        } else {
            4 + (crate::mates::azar(k as u32 * 31 + b as u32) % (lado as u32 / 2)) as i32
        };
        let bx = x + 5 + b * paso;
        cv.rect(bx, y + lado - 5 - a, paso - 2, a, if suena { color } else { mezclar(color, NEGRO, 150, 256) });
    }
}
