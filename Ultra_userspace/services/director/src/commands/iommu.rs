//! **`iommu`: la IOMMU, preguntada.** Si el firmware la dejo encendida, que
//! sabe hacer, a quien atiende y que memoria exige.
//!
//! [consumo] NADA      solo lee: unos `info` de lo que el kernel leyo al arrancar
//!
//! # Por que existe (2026-09-23, M0a de `docs/plan/PLAN_LA_3060.md`)
//!
//! El VBLANK de la 3060 por MSI pide su Bus Master, y el propietario decidio
//! que eso va detras de la IOMMU. Antes de encenderla se pregunta, como se
//! pregunto la grafica -- y la respuesta la da la fila `verdict`.
//!
//! Va tambien en el `save`, capitulo 2, por lo mismo que `gpu`: una pregunta
//! que solo se contesta si alguien teclea la orden es una pregunta que se
//! olvida.

use bmo_userland as bmo;

use super::tabla::{campo, section};
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};
use bmo_iommu_amdvi as amdvi;

/// `iommu` desde el escritorio.
pub(crate) fn iommu(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_iommu(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "iommu", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// Un BDF como `bb:dd.f`.
fn bdf(s: &mut Output, v: u64) {
    s.hex((v >> 8) & 0xFF, 2);
    s.byte(b':');
    s.hex((v >> 3) & 0x1F, 2);
    s.byte(b'.');
    s.dec(v & 7);
}

/// Una bandera con nombre, si esta.
fn si(s: &mut Output, esta: bool, nombre: &[u8]) {
    if esta {
        s.byte(b' ');
        s.text(nombre);
    }
}

/// **El cuadro de la IOMMU.** Lo usan `iommu` y el `save`.
pub(crate) fn report_iommu(s: &mut Output) {
    section(s, b"iommu -- la frontera del DMA, en solo lectura");
    let d = bmo::info(bmo::INFO_IOMMU_DONDE);
    campo(s, b"where");
    if d & bmo::IOMMU_HALLADA == 0 {
        s.with_ink(INK_ERR);
        s.text(b"el IVRS no trae una IOMMU que se lea: nada limita el DMA\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let base = (d & bmo::IOMMU_BASE_PAGINAS_MASK) << 12;
    s.text(b"IVHD tipo 0x");
    s.hex((d >> bmo::IOMMU_TIPO_SHIFT) & 0xFF, 2);
    s.text(b"   registros en 0x");
    s.hex(base, 8);
    s.with_ink(INK_ECHO);
    s.text(b"   su BDF ");
    bdf(s, (d >> bmo::IOMMU_BDF_SHIFT) & 0xFFFF);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu base", base, b"");

    let control = bmo::info(bmo::INFO_IOMMU_CONTROL);
    let funciones = bmo::info(bmo::INFO_IOMMU_FUNCIONES);
    if d & bmo::IOMMU_MUDA != 0 {
        veredicto(s, false, b"sus registros no contestan (todo unos, o fuera del physmap)");
        return;
    }

    let c = amdvi::Control(control);
    campo(s, b"state");
    if c.encendida() {
        s.with_ink(INK_ERR);
        s.text(b"ENCENDIDA por el firmware");
    } else {
        s.with_ink(INK_GOOD);
        s.text(b"APAGADA: nadie traduce, todo aparato ve toda la RAM");
    }
    s.with_ink(INK_ECHO);
    s.text(b"   control 0x");
    s.hex(control, 16);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu control", control, b"");

    let f = amdvi::Funciones(funciones);
    campo(s, b"can");
    s.text(b"paginas de hasta ");
    s.dec(f.niveles() as u64);
    s.text(b" niveles;");
    si(s, f.nx(), b"NX");
    si(s, f.x2apic(), b"x2APIC");
    si(s, f.ga(), b"GA");
    si(s, f.invalidar_todo(), b"INVALIDAR-TODO");
    si(s, f.gt(), b"GT");
    si(s, f.ppr(), b"PPR");
    si(s, f.prefetch(), b"PREFETCH");
    si(s, f.he(), b"HE");
    si(s, f.contadores(), b"CONTADORES");
    s.with_ink(INK_ECHO);
    s.text(b"   EFR 0x");
    s.hex(funciones, 16);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu efr", funciones, b"");

    let t = bmo::info(bmo::INFO_IOMMU_TABLA);
    let e = amdvi::Estado(bmo::info(bmo::INFO_IOMMU_ESTADO));
    campo(s, b"tables");
    match amdvi::Tabla::de_registro(t) {
        Some(tb) => {
            s.text(b"ya hay tabla de dispositivos en 0x");
            s.hex(tb.base, 8);
            s.text(b" (");
            s.dec(tb.entradas());
            s.text(b" BDF)");
        }
        None => s.text(b"ninguna armada"),
    }
    s.text(if e.ordenes_corren() { b"; ordenes CORREN" as &[u8] } else { b"; ordenes paradas" });
    s.text(if e.eventos_corren() { b"; eventos CORREN" as &[u8] } else { b"; eventos parados" });
    s.byte(b'\n');

    let n = bmo::info(bmo::INFO_IOMMU_CENSO);
    if n & bmo::IOMMU_CENSO_VALIDO != 0 {
        let max = n & 0xFFFF;
        let campo8 = |sh: u64| (n >> sh) & 0xFF;
        campo(s, b"serves");
        s.dec(campo8(bmo::IOMMU_CENSO_UNOS_SHIFT));
        s.text(b" sueltos, ");
        s.dec(campo8(bmo::IOMMU_CENSO_RANGOS_SHIFT));
        s.text(b" rangos, ");
        s.dec(campo8(bmo::IOMMU_CENSO_ALIAS_SHIFT));
        s.text(b" alias");
        if n & bmo::IOMMU_CENSO_TODOS != 0 {
            s.text(b", TODOS los BDF");
        }
        s.text(b"; el mayor ");
        bdf(s, max);
        s.with_ink(INK_ECHO);
        s.text(b" -> tabla de ");
        s.dec(amdvi::tabla_para(max as u16) / 1024);
        s.text(b" KiB");
        s.with_ink(INK_PLAIN);
        let raras = (n >> bmo::IOMMU_CENSO_RARAS_SHIFT) & 0xF;
        if raras > 0 {
            s.text(b"; ");
            s.dec(raras);
            s.text(b" entradas por HID o sin nombre");
        }
        if n & bmo::IOMMU_CENSO_CORTADO != 0 {
            s.with_ink(INK_ERR);
            s.text(b"  CORTADO: una entrada se salia del bloque");
            s.with_ink(INK_PLAIN);
        }
        s.byte(b'\n');
        super::datos::anotar(b"iommu mayor bdf", max, b"");

        for i in 0..campo8(bmo::IOMMU_CENSO_ESPECIALES_SHIFT).min(8) {
            let x = bmo::info(bmo::INFO_IOMMU_ESPECIAL | i << bmo::IOMMU_INDICE_SHIFT);
            if x & bmo::IOMMU_VALIDA == 0 {
                continue;
            }
            campo(s, b"special");
            s.text(match (x >> 24) & 0xFF {
                1 => b"IOAPIC id 0x" as &[u8],
                2 => b"HPET   n.  0x",
                _ => b"??     0x",
            });
            s.hex((x >> 16) & 0xFF, 2);
            s.text(b" pide como ");
            bdf(s, x & 0xFFFF);
            s.with_ink(INK_ECHO);
            s.text(b"   banderas 0x");
            s.hex((x >> 32) & 0xFF, 2);
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }

        let nm = campo8(bmo::IOMMU_CENSO_IVMD_SHIFT).min(8);
        if nm == 0 {
            campo(s, b"ivmd");
            s.text(b"ninguno: el firmware no exige memoria por DMA\n");
        }
        for i in 0..nm {
            let sel = bmo::INFO_IOMMU_IVMD | i << bmo::IOMMU_INDICE_SHIFT;
            let meta = bmo::info(sel | 2 << bmo::IOMMU_PARTE_SHIFT);
            if meta & bmo::IOMMU_VALIDA == 0 {
                continue;
            }
            let inicio = bmo::info(sel);
            let largo = bmo::info(sel | 1 << bmo::IOMMU_PARTE_SHIFT);
            let ban = (meta >> 40) & 0xFF;
            campo(s, b"ivmd");
            s.text(b"0x");
            s.hex(inicio, 8);
            s.text(b" + ");
            s.dec(largo / 1024);
            s.text(b" KiB");
            si(s, ban & 0x01 != 0, b"IDENTIDAD");
            si(s, ban & 0x02 != 0, b"lee");
            si(s, ban & 0x04 != 0, b"escribe");
            si(s, ban & 0x08 != 0, b"EXCLUSION");
            s.text(match (meta >> 32) & 0xFF {
                0x20 => b"   para TODOS" as &[u8],
                0x21 => b"   para ",
                _ => b"   para el rango ",
            });
            if (meta >> 32) & 0xFF != 0x20 {
                bdf(s, meta & 0xFFFF);
                if (meta >> 32) & 0xFF == 0x22 {
                    s.text(b"..");
                    bdf(s, (meta >> 16) & 0xFFFF);
                }
            }
            s.byte(b'\n');
        }
    }

    match amdvi::veredicto(control, funciones) {
        amdvi::Veredicto::SePuede => veredicto(s, true, b"apagada y contesta: se puede encender con tablas de BMO-X"),
        amdvi::Veredicto::EncendidaPorElFirmware => {
            veredicto(s, false, b"el firmware la dejo TRADUCIENDO: hay que heredar sus tablas, no pisarlas")
        }
        amdvi::Veredicto::Muda => veredicto(s, false, b"sus registros no contestan"),
    }
}

fn veredicto(s: &mut Output, si: bool, frase: &[u8]) {
    campo(s, b"verdict");
    s.with_ink(if si { INK_GOOD } else { INK_ERR });
    s.text(if si { b"SE PUEDE: " as &[u8] } else { b"NO ASI: " });
    s.text(frase);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu se puede encender", si as u64, b"");
}
