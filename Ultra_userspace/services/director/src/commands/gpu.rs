//! **`gpu`: la grafica, preguntada.** Quien es, que modo barre, y si su VBLANK
//! se puede esperar sin firmware.
//!
//! [consumo] NADA      solo lee: seis `info`, y uno de ellos lee la linea que
//!                     barre la tarjeta en ese instante
//!
//! # Por que existe (2026-09-23)
//!
//! `docs/maestro/GPU_NVIDIA_MAESTRO.md` (6b) decidio el orden: primero el VBLANK
//! de la RTX 3060, sin el firmware del GSP. Y antes de construirlo, preguntar
//! si SE PUEDE. La respuesta no la da este fichero: la da la fila `verdict`,
//! con lo que el kernel midio en el arranque (`dev/gpu.rs`).
//!
//! Va tambien en el `save`, capitulo 2: una pregunta que solo se contesta si
//! alguien teclea `gpu` es una pregunta que se olvida.

use bmo_userland as bmo;

use super::tabla::{campo, section};
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// `gpu` desde el escritorio.
pub(crate) fn gpu(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_gpu(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "grafica", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **El cuadro de la grafica.** Lo usan `gpu` y el `save`.
pub(crate) fn report_gpu(s: &mut Output) {
    section(s, b"grafica -- lo que la tarjeta contesta, en solo lectura");
    let c = bmo::info(bmo::INFO_GPU_CHIP);
    campo(s, b"card");
    if c & bmo::GPU_HALLADA == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"no hay grafica NVIDIA en el bus\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let boot0 = c & bmo::GPU_BOOT0_MASK;
    s.text(b"NVIDIA 10DE:");
    s.hex((c >> bmo::GPU_DEVICE_SHIFT) & 0xFFFF, 4);
    s.text(b"   ");
    let nombre = bmo_gpu_ga10x::Chip(boot0 as u32).nombre();
    s.text(if nombre.is_empty() { b"chip desconocido" as &[u8] } else { nombre.as_bytes() });
    s.text(if c & bmo::GPU_AMPERE != 0 { b" (Ampere)" as &[u8] } else { b" (NO es Ampere)" });
    s.text(b" rev ");
    s.hex(boot0 & 0xFF, 2);
    s.with_ink(INK_ECHO);
    s.text(b"   BOOT_0 0x");
    s.hex(boot0, 8);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu boot0", boot0, b"");
    if c & bmo::GPU_AMPERE == 0 {
        veredicto(s, false, b"el chip no es un Ampere: este codigo no sabe leerlo");
        return;
    }

    let mascara = (c >> bmo::GPU_CABEZAS_SHIFT) & 0xFF;
    campo(s, b"heads");
    s.dec(mascara.count_ones() as u64);
    s.text(b" existen (mascara 0x");
    s.hex(mascara, 2);
    s.text(b")");

    let m = bmo::info(bmo::INFO_GPU_MODO);
    if m & bmo::GPU_MODO_VALIDO == 0 {
        s.text(b"; ninguna con un modo que leer\n");
        veredicto(s, false, b"ninguna cabeza pinta: no hay barrido que esperar");
        return;
    }
    s.text(b"; pinta la ");
    s.dec((c >> bmo::GPU_CABEZA_SHIFT) & 0x7);
    s.byte(b'\n');

    let (px, lin, ht, vt) = (m & 0xFFFF, (m >> 16) & 0xFFFF, (m >> 32) & 0xFFFF, (m >> 48) & 0x7FFF);
    let b = bmo::info(bmo::INFO_GPU_BORRADO);
    let (vini, vfin, khz) = (b & 0xFFFF, (b >> 16) & 0xFFFF, (b >> 32) & 0x7FFF_FFFF);
    campo(s, b"mode");
    s.dec(px);
    s.text(b" x ");
    s.dec(lin);
    s.with_ink(INK_ECHO);
    s.text(b"   total ");
    s.dec(ht);
    s.text(b" x ");
    s.dec(vt);
    s.text(b"   reloj ");
    s.dec(khz / 1000);
    s.byte(b'.');
    let c = khz % 1000 / 10;
    s.byte(b'0' + (c / 10) as u8);
    s.byte(b'0' + (c % 10) as u8);
    s.text(b" MHz\n");
    s.with_ink(INK_PLAIN);

    // El refresco: el DICHO por el modo y el MEDIDO por la linea.
    let dicho_mhz = if ht * vt > 0 { khz * 1_000_000 / (ht * vt) } else { 0 };
    let t = bmo::info(bmo::INFO_GPU_TIEMPO);
    let medido = t & bmo::GPU_TIEMPO_MEDIDO != 0;
    let ns = t & 0xFFFF_FFFF;
    campo(s, b"refresh");
    s.text(b"dicho ");
    mili(s, dicho_mhz);
    s.text(b" Hz   ");
    if medido && ns > 0 {
        s.with_ink(INK_GOOD);
        s.text(b"MEDIDO ");
        mili(s, 1_000_000_000_000 / ns);
        s.text(b" Hz");
        s.with_ink(INK_ECHO);
        s.text(b" (");
        s.dec(ns / 1000);
        s.text(b" us el cuadro, ");
        s.dec((t >> 32) & 0xFFFF);
        s.text(b" vueltas de la linea)");
        super::datos::anotar(b"gpu cuadro", ns / 1000, b"us");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"SIN MEDIR: la linea no dio dos vueltas (cambios ");
        s.dec((t >> 48) & 0x7FFF);
        s.text(b")");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    // La ventana del VBLANK: lo que dura, que es el tiempo que el compositor
    // tiene para cambiar lo que se ve sin partir el cuadro.
    let lineas_vb = vt.saturating_sub(lin);
    campo(s, b"vblank");
    s.dec(lineas_vb);
    s.text(b" lineas (");
    s.dec(vini);
    s.text(b" .. ");
    s.dec(vfin);
    s.text(b")");
    if medido && vt > 0 {
        let us = lineas_vb * (ns / 1000) / vt;
        s.with_ink(INK_ECHO);
        s.text(b" = ");
        s.dec(us);
        s.text(b" us por cuadro para cambiar lo que se ve sin partirlo");
        s.with_ink(INK_PLAIN);
        super::datos::anotar(b"gpu vblank", us, b"us");
    }
    s.byte(b'\n');

    let l = bmo::info(bmo::INFO_GPU_LINEA);
    campo(s, b"now");
    if l & bmo::GPU_LINEA_VALIDA == 0 {
        s.with_ink(INK_ERR);
        s.text(b"la linea no se pudo leer ahora\n");
        s.with_ink(INK_PLAIN);
    } else {
        s.text(b"linea ");
        s.dec(l & 0xFFFF);
        s.text(if l & bmo::GPU_LINEA_VBLANK != 0 { b": en VBLANK\n" as &[u8] } else { b": pintando\n" });
    }

    if medido {
        veredicto(s, true, b"la linea da la vuelta: el VBLANK se espera por MMIO, SIN firmware");
    } else {
        veredicto(s, false, b"la linea no dio la vuelta: por MMIO no hay VBLANK que esperar");
    }
}

fn veredicto(s: &mut Output, si: bool, frase: &[u8]) {
    campo(s, b"verdict");
    s.with_ink(if si { INK_GOOD } else { INK_ERR });
    s.text(if si { b"SE PUEDE: " as &[u8] } else { b"NO ASI: " });
    s.text(frase);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu vblank sin firmware", si as u64, b"");
}

/// Milesimas como `60.000`.
fn mili(s: &mut Output, v: u64) {
    s.dec(v / 1000);
    s.byte(b'.');
    let r = v % 1000;
    s.byte(b'0' + (r / 100) as u8);
    s.byte(b'0' + (r / 10 % 10) as u8);
    s.byte(b'0' + (r % 10) as u8);
}

