//! **`gpu`: la grafica, preguntada.** Quien es, que modo barre, y si su VBLANK
//! se puede esperar sin firmware.
//!
//! [consumo] NADA      solo lee: seis `info`, y uno de ellos lee la linea que
//!                     barre la tarjeta en ese instante. `gpu vblank` (E2) es
//!                     la unica orden de aqui que ESCRIBE, y pasa por el candado
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

/// `gpu`, `gpu cegar`, `gpu ver`, `gpu vblank [off]`, `gpu traducir`, `gpu
/// prestar` desde el escritorio.
///
/// ** `traducir` y `prestar` (M0d, 2026-09-24): la 3060 pasa a ver SOLO lo que
/// su dominio de la IOMMU le presta, y se le presta la pagina de prueba que
/// leera el primer DMA de la tarjeta (M0d3). Es el camino del GSP.
///
/// ** `vblank` (E2, 2026-09-24): que la 3060 AVISE del VBLANK por MSI. Es la
/// primera escritura en la grafica que no es la IOMMU, y el kernel solo la
/// hace detras del CANDADO: IOMMU encendida y la 3060 ciega. Tras encenderlo
/// se cuentan los avisos de medio segundo: tienen que ser ~30.
///
/// ** `cegar` y `ver` (M0e, 2026-09-24) cambian la entrada de la 3060 en la
/// IOMMU: BLOQUEADA (su DMA no alcanza la RAM) o DE PASO. Son ordenes que
/// escriben en el hardware, asi que llevan el `save` de antes (modo
/// automatico), como `iommu encender`. Piden la IOMMU encendida.
pub(crate) fn gpu(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    if arg == b"vbios" {
        return super::vbios::orden(dsk, p);
    }
    if arg == b"fwsec" {
        return super::vbios::orden_fwsec(dsk, p);
    }
    if arg == b"gsp" {
        return super::gsp::orden(dsk, p);
    }
    if arg == b"radix" {
        return super::gsp::orden_radix(dsk, p);
    }
    if arg == b"libos" {
        return super::gsp::orden_libos(dsk, p);
    }
    if arg == b"despertar" {
        return super::gsp::orden_despertar(dsk, p);
    }
    if arg == b"cola" {
        return super::gspcola::orden(dsk, p);
    }
    if arg == b"vaciar" {
        return super::gspvaciar::orden(dsk, p);
    }
    if arg == b"sistema" {
        return super::gspsistema::orden(dsk, p);
    }
    let op = match arg {
        b"" => None,
        b"cegar" | b"ciega" => Some((bmo::IOMMU_OP_CEGAR_GPU, b"gpu cegar" as &[u8])),
        b"ver" => Some((bmo::IOMMU_OP_VER_GPU, b"gpu ver" as &[u8])),
        b"vblank" | b"e2" => Some((bmo::IOMMU_OP_E2_ENCENDER, b"gpu vblank" as &[u8])),
        b"vblank off" | b"e2 off" => Some((bmo::IOMMU_OP_E2_APAGAR, b"gpu vblank off" as &[u8])),
        b"traducir" => Some((bmo::IOMMU_OP_TRADUCIR_GPU, b"gpu traducir" as &[u8])),
        b"prestar" => Some((bmo::IOMMU_OP_PRESTAR_PRUEBA, b"gpu prestar" as &[u8])),
        b"fuego" => Some((bmo::IOMMU_OP_GPU_FUEGO, b"gpu fuego" as &[u8])),
        b"frontera" => Some((bmo::IOMMU_OP_GPU_FRONTERA, b"gpu frontera" as &[u8])),
        _ => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  gpu: `gpu`, `gpu cegar`, `gpu ver`, `gpu vblank [off]`, `gpu traducir`, `gpu prestar`, `gpu fuego`, `gpu frontera`, `gpu vbios`, `gpu fwsec`, `gpu gsp`, `gpu radix`, `gpu libos`, `gpu sistema`, `gpu despertar`, `gpu cola` o `gpu vaciar`\n");
            dsk.out.grid.with_ink(INK_PLAIN);
            dsk.field.n = 0;
            return After::Settle;
        }
    };
    if let Some((op, nombre)) = op {
        if !super::files::antes_de_arriesgar(dsk, p, nombre) {
            dsk.field.n = 0;
            return After::Settle;
        }
        let g = &mut dsk.out.grid;
        match bmo::iommu_orden(op) {
            Ok(_) if op == bmo::IOMMU_OP_E2_ENCENDER => {
                let (n, ms) = contar_vblanks(500);
                g.with_ink(if n > 0 { INK_GOOD } else { INK_ERR });
                g.text(b"  E2 ARMADO: ");
                g.dec(n);
                g.text(b" VBLANKs por interrupcion en ");
                g.dec(ms);
                g.text(if n > 0 {
                    b" ms -- la 3060 AVISA\n" as &[u8]
                } else {
                    b" ms -- NO llego ninguno: mira la escalera de abajo\n"
                });
            }
            Ok(v) if op == bmo::IOMMU_OP_TRADUCIR_GPU => {
                g.with_ink(INK_GOOD);
                g.text(b"  la 3060 esta TRADUCIDA (M0d): ve SOLO lo que su dominio presta -- hoy nada; la invalidacion volvio en ");
                g.dec(v & 0xFFFF_FFFF);
                g.text(b" us\n");
            }
            Ok(v) if op == bmo::IOMMU_OP_PRESTAR_PRUEBA => {
                g.with_ink(INK_GOOD);
                g.text(b"  PRESTADA a la 3060: la pagina de prueba en 0x10000000 -> fisica 0x");
                g.hex(v, 8);
                g.text(b", solo lectura, y el ORACULO la releyo por las tablas\n");
            }
            Ok(v) if op == bmo::IOMMU_OP_GPU_FUEGO => {
                g.with_ink(if v == 1024 { INK_GOOD } else { INK_ERR });
                g.text(if v == 1024 {
                    b"  FUEGO: la 3060 LEYO tu RAM por la IOMMU -- " as &[u8]
                } else {
                    b"  FUEGO a medias: "
                });
                g.dec(v);
                g.text(b" de 1024 palabras cuadran con la pagina prestada\n");
            }
            Ok(v) if op == bmo::IOMMU_OP_GPU_FRONTERA => {
                g.with_ink(if v != 0 { INK_GOOD } else { INK_ERR });
                g.text(if v != 0 {
                    b"  FRONTERA: la IOMMU PARO el DMA de la 3060 a 0x20000000 (no prestada) y lo apunto\n" as &[u8]
                } else {
                    b"  FRONTERA NO VISTA: la IOMMU no apunto el fallo de la 3060 -- mira `iommu` y la fila `frontera`\n"
                });
            }
            Ok(v) if op == bmo::IOMMU_OP_E2_APAGAR => {
                g.with_ink(INK_GOOD);
                g.text(if v != 0 {
                    b"  E2 APAGADO: aviso quitado y Bus Master de la 3060 retirado\n" as &[u8]
                } else {
                    b"  E2 no estaba encendido\n"
                });
            }
            Ok(v) => {
                g.with_ink(INK_GOOD);
                g.text(if op == bmo::IOMMU_OP_CEGAR_GPU {
                    b"  la 3060 esta CIEGA: su DMA no alcanza la RAM; la invalidacion volvio en " as &[u8]
                } else {
                    b"  la 3060 VE otra vez (de paso); la invalidacion volvio en "
                });
                g.dec(v & 0xFFFF_FFFF);
                g.text(b" us\n");
            }
            Err(m) => {
                g.with_ink(INK_ERR);
                g.text(b"  NO: ");
                g.text(super::iommu::motivo(m));
                g.byte(b'\n');
            }
        }
        g.with_ink(INK_PLAIN);
        super::iommu::report_iommu(&mut dsk.out.grid);
    }
    report_gpu(&mut dsk.out.grid, Some(p.rayo()));
    paint_status(p, &dsk.run_box, "grafica", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// Motivo propio del escritorio (no del kernel): E2 armado y mudo. Fuera del
/// rango de `IOMMU_NO_*` para no chocar nunca con uno del kernel.
pub(crate) const NO_E2_MUDO: u32 = 0x100;
/// M0d3, motivos del escritorio: el DMA acabo y no trajo la pagina; la
/// frontera no dejo evento.
pub(crate) const NO_FUEGO_A_MEDIAS: u32 = 0x101;
pub(crate) const NO_SIN_FRONTERA: u32 = 0x102;

/// **M0d3: la prueba de fuego y la frontera**, si se intentaron.
fn fila_fuego(s: &mut Output) {
    let f = bmo::info(bmo::INFO_GPU_FUEGO);
    if f & bmo::FUEGO_INTENTADO != 0 {
        campo(s, b"fuego");
        let bien = f & 0x7FF;
        let motivo = (f >> bmo::FUEGO_MOTIVO_SHIFT) & 0xF;
        if f & bmo::FUEGO_HECHO != 0 {
            s.with_ink(INK_GOOD);
            s.text(b"la 3060 LEYO la pagina prestada: 1024 de 1024 palabras, por el DMA del falcon del GSP");
        } else if motivo != 0 {
            s.with_ink(INK_ERR);
            s.text(match motivo {
                1 => b"el falcon NO CONTESTA (error del anillo PRIV)" as &[u8],
                2 => b"el falcon no acabo de limpiar su memoria en 20 ms",
                3 => b"el falcon no dio su nucleo FALCON (seguia el RISC-V)",
                4 => b"la DMEM del falcon es mas chica que 4 KiB",
                5 => b"el DMA NO ACABO en 50 ms",
                _ => b"no llego a empezar",
            });
        } else {
            s.with_ink(INK_ERR);
            s.dec(bien);
            s.text(b" de 1024 cuadran; la primera mala, la ");
            s.dec((f >> bmo::FUEGO_PRIMERA_SHIFT) & 0x7FF);
            s.text(b": 0x");
            s.hex(bmo::info(bmo::INFO_GPU_FUEGO_LEIDO) & 0xFFFF_FFFF, 8);
        }
        s.with_ink(INK_ECHO);
        s.text(b"   DMEM ");
        s.dec((f >> bmo::FUEGO_DMEM_SHIFT) & 0xFF);
        s.text(b" KiB, seguridad ");
        s.dec((f >> bmo::FUEGO_SEGURIDAD_SHIFT) & 3);
        s.text(b", DMA ");
        s.dec(bmo::info(bmo::INFO_GPU_FUEGO_LEIDO) >> 32);
        s.text(b" us, eventos nuevos ");
        s.dec((f >> bmo::FUEGO_EVENTOS_SHIFT) & 0xFF);
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
        super::datos::anotar(b"gpu fuego", f, b"");
    }
    let b = bmo::info(bmo::INFO_GPU_FRONTERA);
    if b & bmo::FUEGO_INTENTADO != 0 {
        campo(s, b"frontera");
        if b & bmo::FUEGO_HECHO != 0 {
            s.with_ink(INK_GOOD);
            s.text(b"AGUANTA: la IOMMU paro el DMA de la 3060 a 0x20000000 (no prestada) con su evento");
        } else {
            s.with_ink(INK_ERR);
            s.text(b"NO VISTA:");
            s.text(if b & bmo::FRONTERA_EVENTO != 0 { b" evento+" as &[u8] } else { b" evento-" });
            s.text(if b & bmo::FRONTERA_BDF != 0 { b" bdf+" as &[u8] } else { b" bdf-" });
            s.text(if b & bmo::FRONTERA_DIR != 0 { b" dir+" as &[u8] } else { b" dir-" });
        }
        s.with_ink(INK_ECHO);
        s.text(b"   el DMA ");
        s.text(if b & bmo::FRONTERA_DMA_ACABO != 0 { b"acabo" as &[u8] } else { b"NO acabo" });
        s.text(b", tipo del evento ");
        s.dec(b & 0xF);
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
        super::datos::anotar(b"gpu frontera", b, b"");
    }
}

/// **Cuenta los VBLANKs de `ms` milisegundos**, cediendo el turno mientras
/// tanto: los avisos entran entre syscall y syscall. `(avisos, ms de verdad)`.
pub(crate) fn contar_vblanks(ms: u64) -> (u64, u64) {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let antes = bmo::info(bmo::INFO_GPU_VBLANK) & 0xFFFF_FFFF;
    let t0 = bmo::ciclos();
    let fin = t0 + hz / 1000 * ms;
    while bmo::ciclos() < fin {
        bmo::yield_screen();
    }
    let despues = bmo::info(bmo::INFO_GPU_VBLANK) & 0xFFFF_FFFF;
    (
        despues.wrapping_sub(antes) & 0xFFFF_FFFF,
        (bmo::ciclos() - t0) / (hz / 1000),
    )
}

/// **E2: la 3060 AVISA?** La cuenta, y la ESCALERA: cada peldano es un sitio
/// donde el aviso se puede quedar. En el metal, el primero que falte dice
/// donde mirar.
fn fila_e2(s: &mut Output) {
    let v = bmo::info(bmo::INFO_GPU_VBLANK);
    let e = bmo::info(bmo::INFO_GPU_E2);
    campo(s, b"e2");
    if v & bmo::E2_ARMADO != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"ARMADO: ");
    } else if v & bmo::E2_CALLADA != 0 {
        s.with_ink(INK_ERR);
        s.text(b"CALLADO desde la interrupcion (`gpu vblank off` retira el Bus Master): ");
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"apagado: ");
    }
    s.dec(v & 0xFFFF_FFFF);
    s.text(b" VBLANKs por interrupcion, ");
    s.dec((v >> bmo::E2_ENTRADAS_SHIFT) & 0xFF_FFFF);
    s.text(b" entradas al vector 50");
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu e2 vblanks", v & 0xFFFF_FFFF, b"");
    if e & bmo::E2_VALIDA == 0 {
        return;
    }
    campo(s, b"ladder");
    for (bit, nombre) in [
        (bmo::E2_VECTOR, b"vector" as &[u8]),
        (bmo::E2_CIEGA, b"ciega"),
        (bmo::E2_MSI, b"MSI"),
        (bmo::E2_BME, b"BME"),
        (bmo::E2_ENCENDIDO, b"aviso"),
        (bmo::E2_EVENTO, b"evento"),
        (bmo::E2_HOJA, b"hoja"),
        (bmo::E2_CIMA, b"cima"),
    ] {
        s.with_ink(if e & bit != 0 { INK_GOOD } else { INK_ECHO });
        s.text(nombre);
        s.text(if e & bit != 0 { b"+ " as &[u8] } else { b"- " });
    }
    if e & bmo::E2_MSI_MASCARA != 0 {
        s.with_ink(INK_ERR);
        s.text(b"MSI ENMASCARADO ");
    }
    // Un Bus Master encendido con la 3060 viendo la RAM es justo lo que el
    // candado impide: si alguna vez sale, se grita.
    if e & bmo::E2_BME != 0 && e & bmo::E2_CIEGA == 0 {
        s.with_ink(INK_ERR);
        s.text(b"BME SIN CEGAR ");
    }
    s.with_ink(INK_ECHO);
    let ajenos = (e >> bmo::E2_AJENOS_SHIFT) & 0xFFFF;
    let otras = (e >> bmo::E2_OTRAS_SHIFT) & 0xFF;
    if ajenos > 0 || otras > 0 {
        s.text(b"  ajenos ");
        s.dec(ajenos);
        s.text(b", de otras cabezas ");
        s.dec(otras);
    }
    let motivo = (e >> bmo::E2_MOTIVO_SHIFT) & 0xFF;
    if motivo != 0 {
        s.with_ink(if motivo == bmo::E2_APAGADO_ORDEN { INK_ECHO } else { INK_ERR });
        s.text(match motivo {
            bmo::E2_APAGADO_ORDEN => b"  (apagado por orden)" as &[u8],
            bmo::E2_APAGADO_TORMENTA => b"  (CALLADO: TORMENTA, mas de 1000 avisos en un segundo)",
            bmo::E2_APAGADO_CANDADO => b"  (apagado por el candado: la IOMMU se apago o la 3060 volvio a ver)",
            bmo::E2_APAGADO_NO_CONTESTA => b"  (CALLADO: la tarjeta no contesto)",
            _ => b"  (apagado)",
        });
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// **El cuadro de la grafica.** Lo usan `gpu` y el `save`.
pub(crate) fn report_gpu(s: &mut Output, rayo: Option<bmo::CuentasRayo>) {
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
    // `vini` es la ULTIMA visible: el borrado empieza en la siguiente y da
    // la vuelta hasta `vfin` (ver `bmo_gpu_ga10x::Modo`).
    s.text(b" lineas (de la ");
    s.dec(if vini + 1 >= vt { 0 } else { vini + 1 });
    s.text(b" a la ");
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
    let paradas = (l >> bmo::GPU_LINEA_PARADAS_SHIFT) & 0xFFFF;
    campo(s, b"now");
    if l & bmo::GPU_LINEA_VALIDA == 0 {
        s.with_ink(INK_ERR);
        s.text(if paradas > 0 {
            b"el RAYO esta PARADO: la linea no se mueve (nadie le espera)" as &[u8]
        } else {
            b"la linea no se pudo leer ahora"
        });
        s.with_ink(INK_PLAIN);
    } else {
        s.text(b"linea ");
        s.dec(l & 0xFFFF);
        s.text(if l & bmo::GPU_LINEA_VBLANK != 0 { b": en VBLANK" as &[u8] } else { b": pintando" });
    }
    if paradas > 0 {
        s.with_ink(INK_ERR);
        s.text(b"   se hallo PARADO ");
        s.dec(paradas);
        s.text(b" vez/veces");
        s.with_ink(INK_PLAIN);
        super::datos::anotar(b"gpu rayo paradas", paradas, b"");
    }
    s.byte(b'\n');

    if let Some(r) = rayo {
        fila_rayo(s, &r, px);
    }
    fila_e2(s);
    fila_fuego(s);
    super::vbios::fila(s);
    super::gsp::fila(s);
    super::gspsistema::fila(s);
    super::gspcola::fila(s);
    super::gspvaciar::fila(s);
    if medido {
        veredicto(s, true, b"la linea da la vuelta: el VBLANK se espera por MMIO, SIN firmware");
    } else {
        veredicto(s, false, b"la linea no dio la vuelta: por MMIO no hay VBLANK que esperar");
    }
}

/// ** EL VOLCADO DETRAS DEL RAYO (E1): cuantas cajas tuvieron que esperar a
/// que la tarjeta no las estuviera leyendo, cuanto, y cuantas no cabian ni
/// esperando -- esas son las que todavia se pueden partir, y las arregla el
/// page flip (M1 de `PLAN_LA_3060.md`).
fn fila_rayo(s: &mut Output, r: &bmo::CuentasRayo, px: u64) {
    campo(s, b"compose");
    if !r.activo {
        s.with_ink(INK_ECHO);
        s.text(b"el volcado NO mira al rayo (sin medir, o sin volcar todavia)\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    s.dec(r.preguntas);
    s.text(b" cajas; ");
    s.dec(r.esperas);
    s.text(b" esperaron al rayo (");
    s.dec(r.esperado_ns / 1000);
    s.text(b" us en total, la peor ");
    s.dec(r.peor_ns / 1000);
    s.text(b" us)");
    if r.rendidas > 0 {
        s.with_ink(INK_ERR);
        s.text(b"   ");
        s.dec(r.rendidas);
        s.text(b" RENDIDAS (desperto tarde: copiada sin esperar otro cuadro)");
    }
    if r.no_caben > 0 {
        s.with_ink(INK_ERR);
        s.text(b"   ");
        s.dec(r.no_caben);
        s.text(b" NO CABEN ni esperando");
    }
    s.with_ink(INK_ECHO);
    // Por pixel, y traducido a lo que cuesta la fila MAS ancha: la de la
    // pantalla entera (`px` del modo), que es la que decide si una caja cabe.
    s.text(b"   copia ");
    s.dec(r.ps_px as u64);
    s.text(b" ps/pixel (");
    s.dec(r.ps_px as u64 * px / 1000);
    s.text(b" ns la fila de ");
    s.dec(px);
    s.text(b")");
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu rayo esperas", r.esperas, b"");
    super::datos::anotar(b"gpu rayo no caben", r.no_caben, b"");
    super::datos::anotar(b"gpu rayo rendidas", r.rendidas, b"");
    super::datos::anotar(b"gpu copia pixel", r.ps_px as u64, b"ps");
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

