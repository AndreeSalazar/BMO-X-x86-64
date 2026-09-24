//! **`save mode`: LA VERIFICACION TOTAL** -- todos los pasos arriesgados de la
//! GPU, en orden, con un `save` antes de cada uno, y al final notas y consejos.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea
//!
//! # Por que existe (2026-09-24)
//!
//! Peticion del propietario, para el dia que BMO-X ejecute la RTX 3060 por
//! primera vez: *"verificacion total"*. Cada paso nuevo hacia la GPU es una
//! orden que escribe en el hardware, y cada una puede tumbar la maquina. Hacerlos
//! a mano, uno por uno, es acordarse del orden, del `save` de antes y de que
//! mirar despues. Esto lo hace la maquina:
//!
//! ```text
//!    save mode              TODOS los pasos, en orden
//!    save mode -gpu         todos menos ese (y cualquier otro con `-nombre`)
//! ```
//!
//! Antes de CADA paso, el informe maestro se guarda -- siempre, este en
//! `save auto` o en `save manual`: esta orden ES un save. Un paso que ya esta
//! hecho no se repite; uno que pide otro que no esta, no se intenta, y se dice.
//! Si un `save` no se puede escribir, se para todo: sin red no se salta.
//!
//! Los pasos viven en [`PASOS`], en el orden en que hay que darlos. Cuando
//! exista E2 (el VBLANK por interrupcion) sera una fila mas.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};
use crate::DEFAULT_DUMP;

/// Un paso de la verificacion.
struct Paso {
    /// El nombre con el que se quita: `-iommu`, `-gpu`.
    nombre: &'static [u8],
    /// Que hace, en una linea.
    que: &'static [u8],
    /// Ya esta hecho (no se repite).
    hecho: fn() -> bool,
    /// Darlo. `Ok` con el valor del kernel, o el motivo del NO.
    dar: fn() -> Result<u64, u32>,
    /// El paso que tiene que estar hecho antes, si lo hay.
    pide: Option<&'static [u8]>,
    /// Lo que mirar despues si salio bien.
    consejo: &'static [u8],
}

fn iommu_encendida() -> bool {
    bmo::info(bmo::INFO_IOMMU_VIVA) & bmo::IOMMU_VIVA_ENCENDIDA != 0
}

fn gpu_ciega() -> bool {
    bmo::info(bmo::INFO_IOMMU_GPU) & bmo::IOMMU_GPU_CIEGA != 0
}

fn encender_iommu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_ENCENDER)
}

fn cegar_gpu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_CEGAR_GPU)
}

/// **Los pasos, en el orden en que hay que darlos.** Ver
/// `docs/plan/PLAN_LA_3060.md`: M0c, M0e, y despues E2.
const PASOS: &[Paso] = &[
    Paso {
        nombre: b"iommu",
        que: b"encender la IOMMU, todo de paso (M0c)",
        hecho: iommu_encendida,
        dar: encender_iommu,
        pide: None,
        consejo: b"usa la maquina un par de minutos (DOOM, el disco, el raton) y teclea `iommu`: los eventos tienen que seguir en 0",
    },
    Paso {
        nombre: b"gpu",
        que: b"cegar la 3060: su DMA no alcanza la RAM (M0e)",
        hecho: gpu_ciega,
        dar: cegar_gpu,
        pide: Some(b"iommu"),
        consejo: b"la 3060 ya no puede tocar tu RAM ni con el Bus Master encendido; lo siguiente es E2, su VBLANK por MSI",
    },
];

/// Que salio de cada paso.
#[derive(Clone, Copy, PartialEq)]
enum Salio {
    Quitado,
    /// Un save de antes no se pudo escribir: de ahi en adelante, nada.
    Parado,
    YaEstaba,
    FaltaOtro,
    Bien,
    No(u32),
}

/// **`save mode [-nombre ...]`**. `None` si `arg` no es `mode ...`.
pub(crate) fn save_mode(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> Option<After> {
    let resto = if arg == b"mode" {
        &b""[..]
    } else if let Some(r) = arg.strip_prefix(b"mode ") {
        r
    } else {
        return None;
    };
    // Los que se quitan: `-nombre`. Un nombre que no es un paso se dice y no
    // se corre nada: una verificacion con una errata no es una verificacion.
    let mut quitados = [false; 8];
    for t in resto.split(|&b| b == b' ').filter(|t| !t.is_empty()) {
        let nombre = t.strip_prefix(b"-").unwrap_or(t);
        match PASOS.iter().position(|p| p.nombre == nombre) {
            Some(i) if t.starts_with(b"-") => quitados[i] = true,
            _ => {
                let g = &mut dsk.out.grid;
                g.with_ink(INK_ERR);
                g.text(b"  save mode: `");
                g.text(t);
                g.text(b"` no es un paso. Los pasos se quitan con `-`:");
                for p in PASOS {
                    g.text(b" -");
                    g.text(p.nombre);
                }
                g.byte(b'\n');
                g.with_ink(INK_PLAIN);
                dsk.field.n = 0;
                return Some(After::Settle);
            }
        }
    }

    {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_GOOD);
        g.text(b"  VERIFICACION TOTAL: ");
        g.dec(PASOS.len() as u64);
        g.text(b" paso(s) en orden, con un save antes de cada uno\n");
        g.with_ink(INK_PLAIN);
    }
    let mut salio = [Salio::Quitado; 8];
    let mut parado = false;
    for (i, paso) in PASOS.iter().enumerate() {
        salio[i] = if parado {
            Salio::Parado
        } else if quitados[i] {
            Salio::Quitado
        } else if (paso.hecho)() {
            Salio::YaEstaba
        } else if paso.pide.map_or(false, |otro| !PASOS.iter().any(|p| p.nombre == otro && (p.hecho)())) {
            Salio::FaltaOtro
        } else {
            // El save de antes: si no se puede, se para TODO.
            if super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo()).is_err() {
                let g = &mut dsk.out.grid;
                g.with_ink(INK_ERR);
                g.text(b"  el save antes de `");
                g.text(paso.nombre);
                g.text(b"` no se pudo escribir: la verificacion se PARA aqui\n");
                g.with_ink(INK_PLAIN);
                parado = true;
                salio[i] = Salio::Parado;
                fila(dsk, paso, Salio::Parado);
                continue;
            }
            match (paso.dar)() {
                Ok(_) => Salio::Bien,
                Err(m) => Salio::No(m),
            }
        };
        fila(dsk, paso, salio[i]);
    }
    // Y el save de despues: lo que quedo, tambien en el disco.
    let _ = super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo());
    notas(dsk, &salio);
    super::iommu::report_iommu(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "verificacion total", INK_DIM);
    dsk.field.n = 0;
    Some(After::Settle)
}

fn fila(dsk: &mut Desktop, paso: &Paso, s: Salio) {
    let g = &mut dsk.out.grid;
    g.text(b"    ");
    g.text(paso.nombre);
    g.text(b"  ");
    match s {
        Salio::Quitado => {
            g.with_ink(INK_ECHO);
            g.text(b"QUITADO (-");
            g.text(paso.nombre);
            g.text(b")");
        }
        Salio::YaEstaba => {
            g.with_ink(INK_GOOD);
            g.text(b"ya estaba hecho");
        }
        Salio::Parado => {
            g.with_ink(INK_ERR);
            g.text(b"PARADO: el save de antes no se pudo escribir");
        }
        Salio::FaltaOtro => {
            g.with_ink(INK_ERR);
            g.text(b"NO SE INTENTA: pide `");
            g.text(paso.pide.unwrap_or(b""));
            g.text(b"` antes");
        }
        Salio::Bien => {
            g.with_ink(INK_GOOD);
            g.text(b"HECHO");
        }
        Salio::No(m) => {
            g.with_ink(INK_ERR);
            g.text(b"NO: ");
            g.text(super::iommu::motivo(m));
        }
    }
    g.with_ink(INK_ECHO);
    g.text(b"   -- ");
    g.text(paso.que);
    g.with_ink(INK_PLAIN);
    g.byte(b'\n');
}

/// **Notas y consejos**: que mirar ahora, paso a paso.
fn notas(dsk: &mut Desktop, salio: &[Salio; 8]) {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_GOOD);
    g.text(b"  NOTAS Y CONSEJOS\n");
    g.with_ink(INK_PLAIN);
    for (i, paso) in PASOS.iter().enumerate() {
        g.text(b"    ");
        g.text(paso.nombre);
        g.text(b": ");
        match salio[i] {
            Salio::Bien | Salio::YaEstaba => g.text(paso.consejo),
            Salio::Quitado => g.text(b"quitado a proposito; `save mode` sin el `-` lo da"),
            Salio::Parado => g.text(b"no se dio: sin save de antes no se arriesga nada; mira `disco`"),
            Salio::FaltaOtro => g.text(b"sin el paso que pide no se da: quita el `-` de ese, o daselo a mano antes"),
            Salio::No(_) => g.text(b"no salio: la foto de la fila de arriba y el `save` de antes estan en el disco; mira `cabina fallos`"),
        }
        g.byte(b'\n');
    }
    g.with_ink(INK_ECHO);
    g.text(b"    al reiniciar todo esto vuelve a APAGADO: nada de lo que hace queda escrito en ningun sitio\n");
    g.with_ink(INK_PLAIN);
}
