//! **`gpu sistema`: L0c4b2a, LO PRIMERO QUE LA CPU LE ESCRIBE AL GSP.**
//! Pide al kernel `GSP_SET_SYSTEM_INFO` y `SET_REGISTRY` en la cola de la CPU,
//! ANTES de despertar el GSP (como nouveau y OpenRM), y despues los relee de
//! la cola: su forma, su suma, lo que dicen de la 3060, y si el GSP ya los leyo.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: una orden al
//!                     kernel y unas 300 preguntas de 8 bytes
//!
//! # Lo que NO hace
//!
//! Armar los bytes: los arma el kernel con lo que el mismo lee del PCI
//! (`bmo_gpu_ga10x::orden`). Desde aqui solo se dice "ya" y se mira.
//!
//! # Como se sabe
//!
//! Los dos mensajes, releidos de la cola, tienen VRPC y suman 0; y despues de
//! `despertar`, el puntero con el que el GSP lee la cola de la CPU (+32 de la
//! cabecera de SU cola) pasa de 0 a 2: los leyo.

use bmo_gpu_ga10x::orden;
use bmo_gpu_ga10x::rpc::{self, Mensaje};
use bmo_userland as bmo;

use super::gspcola::{mem, COLA_GSP, GSPMEM};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// La cola de la CPU dentro de `INFO_GPU_GSP_MEM`: su `writePtr` y sus datos.
const COLA_CPU: u64 = GSPMEM + 0x1000;
const CPU_ESCRITO: u64 = COLA_CPU + 16;
const CPU_DATOS: u64 = COLA_CPU + 0x1000;
/// Hasta donde leyo el GSP la cola de la CPU: el `readPtr` de la cabecera de
/// la cola DEL GSP (nova-core `gsp_read_ptr`, `gspq.rx`).
const GSP_LEYO: u64 = COLA_GSP + 32;

fn u32_en(o: u64) -> u32 {
    (mem(o & !7) >> ((o & 7) * 8)) as u32
}

/// **Mandarlos** (el paso de `save mode`). `Ok(el writePtr nuevo)`.
pub(crate) fn mandar() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_GSP_SISTEMA)
}

/// Lo pregunta `save mode`: la cola de la CPU ya tiene algo.
pub(crate) fn mandado() -> bool {
    u32_en(CPU_ESCRITO) != 0
}

/// Uno de los dos, releido de la pagina `k`: su cabecera y si suma 0.
fn releido(k: u64) -> (Mensaje, bool) {
    let base = CPU_DATOS + k * 4096;
    let mut c = [0u8; rpc::CABECERA];
    for i in 0..rpc::CABECERA / 8 {
        c[i * 8..i * 8 + 8].copy_from_slice(&mem(base + i as u64 * 8).to_le_bytes());
    }
    let m = Mensaje::de(&c);
    if !m.bien_formado() {
        return (m, false);
    }
    let mut s = rpc::Suma::default();
    let n = m.bytes_sumados() as u64;
    let mut o = 0;
    while o < n {
        let w = mem(base + o).to_le_bytes();
        s.mas(&w[..(n - o).min(8) as usize]);
        o += 8;
    }
    (m, s.valor() == 0)
}

/// `gpu sistema`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "escribiendo en la cola de la CPU", INK_DIM);
    let r = mandar();
    let g = &mut dsk.out.grid;
    match r {
        Ok(_) => {
            g.with_ink(INK_GOOD);
            g.text(b"  ESCRITO: SetSystemInfo y SetRegistry en la cola de la CPU; el GSP los leera al despertar\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  NO: ");
            g.text(super::iommu::motivo(m));
            g.byte(b'\n');
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "sistema", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Las filas de L0c4b2a**, si se mandaron.
pub(crate) fn fila(s: &mut Output) {
    let escrito = u32_en(CPU_ESCRITO);
    if escrito == 0 {
        return;
    }
    campo(s, b"sistema");
    let (a, a_bien) = releido(0);
    let (b, b_bien) = releido(1);
    let bien = a_bien && b_bien && a.funcion == orden::SET_SYSTEM_INFO && b.funcion == orden::SET_REGISTRY;
    s.with_ink(if bien { INK_GOOD } else { INK_ERR });
    for (i, (m, ok)) in [(a, a_bien), (b, b_bien)].into_iter().enumerate() {
        if i > 0 {
            s.text(b" y ");
        }
        s.text(rpc::nombre(m.funcion));
        s.text(b" (");
        s.dec(m.datos() as u64);
        s.text(if ok { b" B, suma 0)" as &[u8] } else { b" B, SIN FORMA o suma MAL)" });
    }
    s.with_ink(INK_PLAIN);
    s.text(b" en la cola de la CPU, paginas 0..");
    s.dec(escrito as u64 - 1);
    s.byte(b'\n');

    // Lo que le dice de la 3060, releido de los datos de SetSystemInfo.
    campo(s, b"sysinfo");
    let d = CPU_DATOS + rpc::CABECERA as u64;
    let bdf = mem(d + 0x20);
    s.with_ink(INK_ECHO);
    s.text(b"BAR0 0x");
    s.hex(mem(d), 9);
    s.text(b", BAR1 0x");
    s.hex(mem(d + 8), 10);
    s.text(b", BAR3 0x");
    s.hex(mem(d + 0x10), 10);
    s.text(b"; PCI ");
    s.hex(bdf >> 8 & 0xFF, 2);
    s.byte(b':');
    s.hex(bdf >> 3 & 0x1F, 2);
    s.byte(b'.');
    s.dec(bdf & 7);
    let id = u32_en(d + 0x58);
    s.text(b" ");
    s.hex((id & 0xFFFF) as u64, 4);
    s.byte(b':');
    s.hex((id >> 16) as u64, 4);
    let sub = u32_en(d + 0x5C);
    s.text(b" sub ");
    s.hex((sub & 0xFFFF) as u64, 4);
    s.byte(b':');
    s.hex((sub >> 16) as u64, 4);
    s.text(b" rev ");
    s.hex(u32_en(d + 0x60) as u64, 2);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    // Y si el GSP ya los leyo.
    campo(s, b"leyo");
    let leyo = u32_en(GSP_LEYO);
    let despierto = bmo::info(bmo::INFO_GPU_DESPIERTO) & bmo::DESPIERTO_VISTO != 0;
    if leyo >= escrito {
        s.with_ink(INK_GOOD);
        s.text(b"el GSP LOS LEYO: su puntero sobre la cola de la CPU esta en ");
        s.dec(leyo as u64);
    } else if despierto {
        s.with_ink(INK_ERR);
        s.text(b"el GSP desperto y NO los ha leido: su puntero sigue en ");
        s.dec(leyo as u64);
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"esperan en la cola: el GSP todavia no ha despertado");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"gpu gsp leyo de la cpu", leyo as u64, b"");
}
