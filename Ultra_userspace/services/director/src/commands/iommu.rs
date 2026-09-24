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

/// `iommu`, `iommu encender`, `iommu apagar` desde el escritorio.
///
/// ** `encender` y `apagar` son ORDENES ARRIESGADAS (M0c): escriben en la
/// frontera del DMA de toda la maquina. En modo `save` automatico el informe
/// maestro se guarda ANTES, y si no se puede guardar la orden no se hace. Y el
/// kernel hace `FLUSH CACHE` del disco antes de tocar nada.
pub(crate) fn iommu(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    let op = match arg {
        b"" => None,
        b"encender" | b"on" => Some((bmo::IOMMU_OP_ENCENDER, b"iommu encender" as &[u8])),
        b"apagar" | b"off" => Some((bmo::IOMMU_OP_APAGAR, b"iommu apagar" as &[u8])),
        _ => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  iommu: `iommu`, `iommu encender` o `iommu apagar`\n");
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
            Ok(v) if op == bmo::IOMMU_OP_ENCENDER => {
                g.with_ink(INK_GOOD);
                g.text(b"  IOMMU ENCENDIDA y OBEDECE: el COMPLETION_WAIT volvio en ");
                g.dec(v & 0xFFFF_FFFF);
                g.text(b" us; ");
                g.dec(v >> 32);
                g.text(b" eventos en su registro\n");
            }
            Ok(_) => {
                g.with_ink(INK_GOOD);
                g.text(b"  IOMMU APAGADA: el control vuelve a como lo dejo el firmware\n");
            }
            Err(m) => {
                g.with_ink(INK_ERR);
                g.text(b"  NO: ");
                g.text(motivo(m));
                g.byte(b'\n');
            }
        }
        g.with_ink(INK_PLAIN);
    }
    report_iommu(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "iommu", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// El motivo de un NO de `TASK_OP_IOMMU`, en palabras.
fn motivo(m: u32) -> &'static [u8] {
    match m {
        bmo::IOMMU_NO_ESCRITORIO => b"solo el escritorio (quien tiene la pantalla) la mueve",
        bmo::IOMMU_NO_TABLAS => b"no hay tablas de M0b releidas iguales (mira la fila `ours`)",
        bmo::IOMMU_NO_YA_ENCENDIDA => b"ya estaba encendida: no se pisa",
        bmo::IOMMU_NO_CONTESTA => b"el COMPLETION_WAIT no volvio en 10 ms: se APAGO sola otra vez",
        bmo::IOMMU_NO_APAGADA => b"no la encendio BMO-X: desde aqui no se apaga",
        _ => b"el kernel dijo que no, sin motivo conocido",
    }
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
    section(s, b"iommu -- la frontera del DMA de todo aparato");
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
    let viva = bmo::info(bmo::INFO_IOMMU_VIVA);
    campo(s, b"state");
    if c.encendida() && viva & bmo::IOMMU_VIVA_ENCENDIDA != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"ENCENDIDA por BMO-X (M0c): todo DE PASO por ahora");
    } else if c.encendida() {
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
    fila_armado(s, viva & bmo::IOMMU_VIVA_ENCENDIDA != 0);
    fila_viva(s, viva);

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

    // ** Encendida por BMO-X, el veredicto de la sonda (que mira el control
    // como si fuera la foto del arranque) diria "la dejo el firmware". No.
    if viva & bmo::IOMMU_VIVA_ENCENDIDA != 0 && c.encendida() {
        veredicto(s, true, b"ENCENDIDA POR BMO-X y obedece (M0c), todo de paso; lo siguiente es TRADUCIR (M0d)");
        return;
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

/// ** M0b: las tablas de BMO-X, armadas en RAM y SIN ENTREGAR. Si la fila
/// dice `releida igual`, M0c tiene lo que darle a la IOMMU.
fn fila_armado(s: &mut Output, entregada: bool) {
    let a = bmo::info(bmo::INFO_IOMMU_ARMADO);
    let c = bmo::info(bmo::INFO_IOMMU_COLAS);
    campo(s, b"ours");
    if a & bmo::IOMMU_ARMADO_SI == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"sin armar (no se pudo encender, o no hubo paginas contiguas: `cabina fallos`)\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let paginas = (a >> bmo::IOMMU_ARMADO_PAGINAS_SHIFT) & 0xFFF;
    s.text(b"tabla de ");
    s.dec(paginas * 4);
    s.text(b" KiB en 0x");
    s.hex((a & bmo::IOMMU_BASE_PAGINAS_MASK) << 12, 8);
    s.text(b", todo DE PASO; ");
    s.dec((a >> bmo::IOMMU_ARMADO_BANDERAS_SHIFT) & 0x3FFF);
    s.text(b" con banderas del IVHD; colas de ");
    s.dec((c >> 36) & 0xFFFF);
    s.text(b" en 0x");
    s.hex((c & bmo::IOMMU_BASE_PAGINAS_MASK) << 12, 8);
    if a & bmo::IOMMU_ARMADO_COMPROBADO != 0 {
        s.with_ink(INK_GOOD);
        s.text(if entregada {
            b"   releida igual, ENTREGADA: es la que la IOMMU usa\n" as &[u8]
        } else {
            b"   releida igual, SIN ENTREGAR\n"
        });
    } else {
        s.with_ink(INK_ERR);
        s.text(b"   releida DISTINTA de lo escrito\n");
    }
    s.with_ink(INK_PLAIN);
    super::datos::anotar(b"iommu tabla armada", a, b"");
}

/// ** M0c: lo que paso al encenderla. Solo sale si alguien lo intento.
fn fila_viva(s: &mut Output, v: u64) {
    let intentos = (v >> bmo::IOMMU_VIVA_INTENTOS_SHIFT) & 0xF;
    if intentos == 0 {
        return;
    }
    campo(s, b"live");
    if v & bmo::IOMMU_VIVA_ENCENDIDA != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"ENCENDIDA; el COMPLETION_WAIT volvio en ");
        s.dec(v & 0xFFFF_FFFF);
        s.text(b" us; eventos ");
        s.dec((v >> bmo::IOMMU_VIVA_EVENTOS_SHIFT) & 0xFFFF);
    } else if v & bmo::IOMMU_VIVA_CONTESTO != 0 {
        s.with_ink(INK_ECHO);
        s.text(b"se encendio y contesto; ahora APAGADA");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"no se pudo: ");
        s.text(motivo(((v >> bmo::IOMMU_VIVA_MOTIVO_SHIFT) & 0xFF) as u32));
    }
    s.with_ink(INK_ECHO);
    s.text(b"   (");
    s.dec(intentos);
    s.text(b" intento(s))\n");
    s.with_ink(INK_PLAIN);
    super::datos::anotar(b"iommu viva", v, b"");
}
