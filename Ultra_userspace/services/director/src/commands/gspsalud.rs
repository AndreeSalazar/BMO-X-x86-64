//! **`gpu salud`: LA 3060 POR DENTRO** -- la temperatura del sensor y el
//! enlace PCIe (lecturas del kernel, sin el GSP-RM), y el P-state preguntado
//! al GSP-RM por `GSP_RM_CONTROL` sobre NUESTRO subdispositivo (L1b).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     dos lecturas y, con el RM, una pregunta de hasta 5 s
//!
//! Lo mismo, resumido, esta siempre en el panel (`scene::lateral_gsp`): esto
//! es la version con los crudos, para el INFORME.
//!
//! Los VATIOS de la 3060 no salen aqui, y no por olvido: la orden del RM que
//! los da no la publica NVIDIA (ver `bmo_gpu_ga10x::salud`).

use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL, GSP_RM_CONTROL};
use bmo_gpu_ga10x::salud;
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// **Una orden de control contestada**: su respuesta, el `rpc_result`, lo que
/// tardo y el numero de la RPC.
#[derive(Clone, Copy)]
pub(crate) struct Contestada {
    pub r: control::Respuesta,
    pub resultado: u32,
    pub espera_us: u64,
    pub numero: u32,
}

impl Contestada {
    /// NV_OK en los dos sitios donde el RM lo dice.
    pub(crate) fn bien(&self) -> bool {
        self.r.estado == 0 && self.resultado == 0
    }
}

/// **Preguntar `c`** (una de las PREGUNTAS de `control::Control`) y esperar
/// su respuesta: los datos quedan en `d`, que mide la cabecera y los
/// parametros. `Err(motivo)` si no salio o no llego.
pub(crate) fn controlar(c: Control, d: &mut [u8]) -> Result<Contestada, u32> {
    let que = Control::TODOS.iter().position(|&k| k == c).unwrap_or(0) as u64;
    let numero = (bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_CONTROL, que)? >> 32) as u32;
    let (m, espera_us) = esperar(GSP_RM_CONTROL, d, &mut Otros::default())?;
    let r = control::leer(d).ok_or(NO_CONTROL_NEGADO)?;
    Ok(Contestada { r, resultado: m.resultado, espera_us, numero })
}

static mut PSTATE: Option<Result<Contestada, u32>> = None;

fn ultimo() -> Option<Result<Contestada, u32>> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(PSTATE) }
}

/// Motivo del escritorio: el RM contesto, pero la orden de control no salio.
pub(crate) const NO_CONTROL_NEGADO: u32 = 0x123;

/// **Preguntar el P-state.** `Ok(la mascara)`.
pub(crate) fn preguntar() -> Result<u64, u32> {
    let u = controlar(Control::Pstate, &mut [0u8; CABECERA_CONTROL + 4]);
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of_mut!(PSTATE) = Some(u) };
    let c = u?;
    if !c.bien() {
        return Err(NO_CONTROL_NEGADO);
    }
    if let Some(k) = control::pstate(c.r.valor) {
        crate::scene::lateral_gsp::pstate(k);
    }
    Ok(c.r.valor as u64)
}

/// Lo pregunta `save mode`: el P-state ya se contesto.
pub(crate) fn hecho() -> bool {
    matches!(ultimo(), Some(Ok(c)) if c.bien())
}

/// `gpu salud`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    // El P-state solo con el GSP-RM y nuestros objetos: sin ellos, las
    // lecturas del kernel valen igual.
    if super::gspobjeto::listos() {
        paint_status(p, &dsk.run_box, "preguntandole el P-state al GSP-RM", INK_DIM);
        let _ = preguntar();
    }
    let g = &mut dsk.out.grid;
    g.with_ink(INK_GOOD);
    g.text(b"  LA 3060 POR DENTRO: temperatura y enlace en solo lectura; el P-state, al RM\n");
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "salud", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Las filas**: `temp` y `pcie` siempre que haya 3060; `pstate` si se
/// pregunto.
pub(crate) fn fila(s: &mut Output) {
    if bmo::info(bmo::INFO_GPU_CHIP) & bmo::GPU_HALLADA == 0 {
        return;
    }
    let crudo = bmo::info(bmo::INFO_GPU_SALUD) as u32;
    campo(s, b"temp");
    match salud::lectura(crudo) {
        Some((g, fe)) => {
            s.with_ink(if g >= 83 { INK_ERR } else { INK_GOOD });
            s.dec(g as u64);
            s.text(b" grados");
            s.with_ink(INK_PLAIN);
            if fe == salud::Fe::Probable {
                s.text(b" (PROBABLE: bit 29 caido, 31 puesto; la historia del panel lo dira)");
            }
        }
        None => s.text(b"sin dato (el bit de validez del sensor, caido)"),
    }
    s.with_ink(INK_ECHO);
    s.text(b"   sensor 0x020460 = 0x");
    s.hex(crudo as u64, 8);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    let e = bmo::info(bmo::INFO_GPU_SALUD | 1 << 8);
    campo(s, b"pcie");
    match salud::enlace(e as u16, (e >> 32) as u32) {
        Some(l) => {
            let gts = |s: &mut Output, gen: u8| {
                let v = salud::gts(gen);
                s.dec(v as u64 / 10);
                s.byte(b'.');
                s.dec(v as u64 % 10);
                s.text(b" GT/s");
            };
            s.text(b"Gen");
            s.dec(l.gen as u64);
            s.text(b" x");
            s.dec(l.ancho as u64);
            s.text(b" (");
            gts(s, l.gen);
            s.text(b") de Gen");
            s.dec(l.gen_max as u64);
            s.text(b" x");
            s.dec(l.ancho_max as u64);
            if l.gen == 1 && l.gen < l.gen_max {
                s.with_ink(INK_ECHO);
                s.text(b"; en Gen1 porque arranca asi: subirlo es del RM");
                s.with_ink(INK_PLAIN);
            }
        }
        None => s.text(b"sin capacidad PCI Express leida"),
    }
    s.byte(b'\n');

    // ** LA FOTO AL LLEGAR: cruda, sin veredicto. El metal (24-09 14:09) la
    // leyo "con techo" y aun asi FWSEC-FRTS encontro la WPR2 vacia: al sondear
    // el firmware de arranque de la tarjeta aun no acabo. Solo para mirar.
    let w = bmo::info(bmo::INFO_GPU_SALUD | 2 << 8);
    if w >> 63 != 0 {
        campo(s, b"al llegar");
        let f = bmo::info(bmo::INFO_GPU_SALUD | 3 << 8);
        s.with_ink(INK_ECHO);
        s.text(b"crudo, antes del firmware de arranque: WPR2 0x");
        s.hex(w as u32 as u64, 8);
        s.text(b" / 0x");
        s.hex((w >> 32) as u32 as u64, 8);
        if let Some(l) = salud::enlace(f as u16, (f >> 32) as u32) {
            s.text(b", enlace Gen");
            s.dec(l.gen as u64);
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }

    cargador(s);
    super::gsprelojes::fila(s);

    if let Some(u) = ultimo() {
        campo(s, b"pstate");
        match u {
            Err(m) => {
                s.with_ink(INK_ERR);
                s.text(b"NO: ");
                s.text(super::iommu::motivo(m));
            }
            Ok(c) => {
                let (r, bien) = (c.r, c.bien());
                s.with_ink(if bien { INK_GOOD } else { INK_ERR });
                match control::pstate(r.valor) {
                    Some(k) if bien => {
                        s.byte(b'P');
                        s.dec(k as u64);
                        s.text(if k == 0 { b" (a todo lo que da)" as &[u8] } else if k >= 8 { b" (reposo)" } else { b"" });
                    }
                    _ => {
                        s.text(bmo_gpu_ga10x::objeto::estado(r.estado));
                        s.text(b" (0x");
                        s.hex(r.estado as u64, 2);
                        s.byte(b')');
                    }
                }
                s.with_ink(INK_PLAIN);
                s.text(b"   PERF_GET_CURRENT_PSTATE en ");
                s.dec(c.espera_us / 1000);
                s.text(b" ms (numero ");
                s.dec(c.numero as u64);
                s.text(b"), mascara 0x");
                s.hex(r.valor as u64, 4);
            }
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
}

// == LO QUE HIZO EL CARGADOR (25-09) =========================================
//
// Las banderas de `boot_context::GPU_REINICIO_*` (el cargador `s1_cpu` las
// escribe, el kernel las publica en `INFO_GPU_SALUD` 4, 5 y 6). Copiadas
// aqui con el mismo valor: el director no enlaza con `boot_context`.
const R_HALLADA: u64 = 1 << 1;
const R_CALIENTE: u64 = 1 << 2;
const R_APAGADO: u64 = 1 << 3;
const R_SIEMPRE: u64 = 1 << 4;
const R_HECHO: u64 = 1 << 5;
const R_VOLVIO: u64 = 1 << 6;
const R_GFW: u64 = 1 << 7;
const R_GOP: u64 = 1 << 8;
const R_SIN_PUENTE: u64 = 1 << 9;
const R_SIN_HANDLE: u64 = 1 << 10;
const R_POR_RISCV: u64 = 1 << 11;
const R_POR_WPR2: u64 = 1 << 12;

/// Las dos lecturas crudas: RISC-V del GSP abajo, WPR2 arriba.
fn lecturas(s: &mut Output, v: u64) {
    s.text(b"riscv 0x");
    s.hex(v & 0xFFFF_FFFF, 8);
    s.text(b" wpr2 0x");
    s.hex(v >> 32, 8);
}

/// **La fila `cargador`**: si la 3060 llego caliente, y si el cargador la
/// reinicio por el bus antes de que BMO-X la mirara.
fn cargador(s: &mut Output) {
    let f = bmo::info(bmo::INFO_GPU_SALUD | 4 << 8);
    if f & R_HALLADA == 0 {
        return;
    }
    let antes = bmo::info(bmo::INFO_GPU_SALUD | 5 << 8);
    campo(s, b"cargador");
    if f & R_HECHO != 0 {
        let bien = f & (R_VOLVIO | R_GFW | R_GOP) == R_VOLVIO | R_GFW | R_GOP;
        s.with_ink(if bien { INK_GOOD } else { INK_ERR });
        s.text(if f & R_CALIENTE != 0 { b"venia CALIENTE" as &[u8] } else { b"fria, pero `siempre`" });
        if f & R_POR_RISCV != 0 {
            s.text(b" (su GSP seguia en marcha)");
        } else if f & R_POR_WPR2 != 0 {
            s.text(b" (la WPR2 seguia arriba)");
        }
        s.text(b": REINICIADA por el bus en ");
        s.dec(f >> 32 & 0xFFFF);
        s.text(b" ms; ");
        s.text(if f & R_VOLVIO != 0 { b"volvio" as &[u8] } else { b"NO volvio" });
        s.text(if f & R_GFW != 0 { b", su firmware arranco" as &[u8] } else { b", su firmware NO acabo" });
        s.text(if f & R_GOP != 0 { b", la pantalla volvio" as &[u8] } else { b", la pantalla NO volvio" });
        s.with_ink(INK_ECHO);
        s.text(b"   antes ");
        lecturas(s, antes);
        s.text(b", despues ");
        lecturas(s, bmo::info(bmo::INFO_GPU_SALUD | 6 << 8));
        s.text(b"; Gen");
        s.dec(f >> 16 & 0xF);
        s.text(b" -> Gen");
        s.dec(f >> 20 & 0xF);
    } else if f & R_CALIENTE != 0 {
        s.with_ink(INK_ERR);
        s.text(b"venia CALIENTE y NO se reinicio: ");
        s.text(if f & R_APAGADO != 0 {
            b"compilado con BMO_GPU_REINICIO=no" as &[u8]
        } else if f & R_SIN_PUENTE != 0 {
            b"no hay puente encima de la 3060 que pulsar"
        } else if f & R_SIN_HANDLE != 0 {
            b"la UEFI no dio su asa: su driver no se podia parar"
        } else {
            b"sin BAR0 de memoria"
        });
        s.with_ink(INK_ECHO);
        s.text(b"   ");
        lecturas(s, antes);
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"fria al llegar (GSP parado, sin WPR2): el cargador no la toco   ");
        lecturas(s, antes);
        if f & R_SIEMPRE != 0 {
            s.text(b"; `siempre` sin reiniciar: mira el puerto serie");
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}
