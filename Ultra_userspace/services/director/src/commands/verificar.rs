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
//!
//! Y queda ARMADO en `datos/modo.txt`: se repite solo en cada arranque, sin el
//! paso que tumbo la maquina si lo hubo (ver `EL MODO ARMADO`, mas abajo). El
//! CONSEJERO dice lo siguiente recomendado cada vez que Ctrl+Alt abre la caja.

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

// == EL MODO ARMADO: SOBREVIVE A UN REINICIO Y A UN FALLO DEL KERNEL ==========
//
// Peticion del propietario (24-09): *"save mode es para automatizar en caso que
// la PC se reinicie o kernel fault"*. Asi que `save mode` no solo corre: queda
// ARMADO en `datos/modo.txt`, y en cada arranque el escritorio lo repite solo.
//
// ** Y con MEMORIA, porque repetir a ciegas es un bucle de caidas: si un paso
// tumba la maquina, el arranque siguiente lo volveria a dar, y el otro, y el
// otro. Antes de CADA paso se escribe `en curso: <paso>` en el fichero (y el
// kernel hace FLUSH del disco antes de tocar la IOMMU, asi que llega); al
// acabar, se borra. Si un arranque encuentra un `en curso`, ese paso TUMBO la
// maquina la vez anterior: se QUITA solo (`-paso`), se apunta `tumbo: paso` y
// el consejero lo dice. Se repite todo menos lo que tumbo.
//
// ```text
//    save mode -gpu        lo corre YA y lo deja ARMADO con esos `-`
//    save mode off         lo desarma: al arrancar ya no se repite
// ```

/// Donde vive el modo armado. 8.3, como todo en FAT32.
const MODO: &[u8] = b"datos/modo.txt";

/// El modo leido del disco.
struct Modo {
    /// Los argumentos tal cual (`-gpu ...`), sin el `save mode`.
    args: [u8; 64],
    n: usize,
    /// El paso que estaba EN CURSO cuando se apago la maquina.
    en_curso: Option<usize>,
    /// El paso que ya se sabe que tumbo la maquina.
    tumbo: Option<usize>,
}

impl Modo {
    fn args(&self) -> &[u8] {
        &self.args[..self.n]
    }
}

fn paso_por_nombre(nombre: &[u8]) -> Option<usize> {
    PASOS.iter().position(|p| p.nombre == nombre)
}

/// **Lee `datos/modo.txt`.** `None` = no esta armado.
fn leer_modo() -> Option<Modo> {
    let a = bmo::Archivo::leer_de(MODO).ok()?;
    let mut buf = [0u8; 256];
    let n = a.read(&mut buf);
    a.close();
    let mut m = Modo { args: [0; 64], n: 0, en_curso: None, tumbo: None };
    let mut armado = false;
    for linea in buf[..n].split(|&b| b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)) {
        if let Some(r) = linea.strip_prefix(b"save mode") {
            let r = r.strip_prefix(b" ").unwrap_or(r);
            let k = r.len().min(m.args.len());
            m.args[..k].copy_from_slice(&r[..k]);
            m.n = k;
            armado = true;
        } else if let Some(r) = linea.strip_prefix(b"en curso: ") {
            m.en_curso = paso_por_nombre(r);
        } else if let Some(r) = linea.strip_prefix(b"tumbo: ") {
            m.tumbo = paso_por_nombre(r);
        }
    }
    armado.then_some(m)
}

/// **Escribe el modo**: armado con `args`, el paso en curso y el que tumbo.
/// `false` si no se pudo escribir.
fn escribir_modo(args: &[u8], en_curso: Option<usize>, tumbo: Option<usize>) -> bool {
    let Ok(a) = bmo::Archivo::create(MODO) else { return false };
    a.write(b"save mode");
    if !args.is_empty() {
        a.write(b" ");
        a.write(args);
    }
    a.write(b"\n");
    if let Some(i) = en_curso {
        a.write(b"en curso: ");
        a.write(PASOS[i].nombre);
        a.write(b"\n");
    }
    if let Some(i) = tumbo {
        a.write(b"tumbo: ");
        a.write(PASOS[i].nombre);
        a.write(b"\n");
    }
    a.close()
}

/// **Desarma**: el fichero queda, pero sin `save mode` dentro.
fn desarmar() -> bool {
    let Ok(a) = bmo::Archivo::create(MODO) else { return false };
    a.write(b"apagado\n");
    a.close()
}

/// Los `-paso` de unos argumentos. `Err(t)` con el que no es un paso.
fn quitados_de(args: &[u8]) -> Result<[bool; 8], &[u8]> {
    let mut quitados = [false; 8];
    for t in args.split(|&b| b == b' ').filter(|t| !t.is_empty()) {
        match t.strip_prefix(b"-").and_then(paso_por_nombre) {
            Some(i) => quitados[i] = true,
            None => return Err(t),
        }
    }
    Ok(quitados)
}

/// **`save mode [-nombre ...]` / `save mode off`**. `None` si `arg` no es
/// `mode ...`.
pub(crate) fn save_mode(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> Option<After> {
    let resto = if arg == b"mode" {
        &b""[..]
    } else if let Some(r) = arg.strip_prefix(b"mode ") {
        r
    } else {
        return None;
    };
    if resto == b"off" || resto == b"apagar" {
        let ok = desarmar();
        let g = &mut dsk.out.grid;
        g.with_ink(if ok { INK_GOOD } else { INK_ERR });
        g.text(if ok {
            b"  save mode DESARMADO: al arrancar ya no se repite\n" as &[u8]
        } else {
            b"  save mode: no se pudo escribir datos/modo.txt\n"
        });
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return Some(After::Settle);
    }
    // Un nombre que no es un paso se dice y no se corre nada: una
    // verificacion con una errata no es una verificacion.
    let quitados = match quitados_de(resto) {
        Ok(q) => q,
        Err(t) => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            g.text(b"  save mode: `");
            g.text(t);
            g.text(b"` no es un paso. Los pasos se quitan con `-`:");
            for p in PASOS {
                g.text(b" -");
                g.text(p.nombre);
            }
            g.text(b"   (y `save mode off` lo desarma)\n");
            g.with_ink(INK_PLAIN);
            dsk.field.n = 0;
            return Some(After::Settle);
        }
    };
    let armado = escribir_modo(resto, None, None);
    {
        let g = &mut dsk.out.grid;
        g.with_ink(if armado { INK_GOOD } else { INK_ERR });
        g.text(if armado {
            b"  save mode ARMADO en datos/modo.txt: al arrancar se repite solo\n" as &[u8]
        } else {
            b"  save mode: NO se pudo armar (datos/modo.txt): corre solo esta vez\n"
        });
        g.with_ink(INK_PLAIN);
    }
    correr(dsk, p, &quitados, resto, None);
    dsk.field.n = 0;
    Some(After::Settle)
}

/// **Al arrancar**: si el modo esta armado, se repite -- sin el paso que
/// tumbo la maquina, si lo hubo. Lo llama el arranque del escritorio.
pub(crate) fn al_arrancar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let Some(m) = leer_modo() else {
        consejero(&mut dsk.out.grid);
        return;
    };
    let mut args = [0u8; 64];
    let mut n = m.n;
    args[..n].copy_from_slice(m.args());
    // El paso en curso cuando se cayo: se QUITA, y se apunta que tumbo.
    let tumbo = m.en_curso.or(m.tumbo);
    if let Some(i) = m.en_curso {
        let nombre = PASOS[i].nombre;
        if n + 2 + nombre.len() <= args.len() {
            args[n] = b' ';
            args[n + 1] = b'-';
            args[n + 2..n + 2 + nombre.len()].copy_from_slice(nombre);
            n += 2 + nombre.len();
        }
        escribir_modo(&args[..n], None, tumbo);
    }
    let Ok(quitados) = quitados_de(&args[..n]) else {
        consejero(&mut dsk.out.grid);
        return;
    };
    {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_GOOD);
        g.text(b"  save mode ARMADO: se repite al arrancar (`save mode off` lo desarma)\n");
        if let Some(i) = m.en_curso {
            g.with_ink(INK_ERR);
            g.text(b"  el paso `");
            g.text(PASOS[i].nombre);
            g.text(b"` estaba EN CURSO cuando se apago la maquina: TUMBO, y queda quitado\n");
        }
        g.with_ink(INK_PLAIN);
    }
    let copia = args;
    correr(dsk, p, &quitados, &copia[..n], tumbo);
}

/// **Los pasos, en orden**, con un save antes de cada uno y la marca `en
/// curso` alrededor. Lo comparten la orden y el arranque.
fn correr(dsk: &mut Desktop, p: &bmo::Pantalla, quitados: &[bool; 8], args: &[u8], tumbo: Option<usize>) {
    {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_GOOD);
        g.text(b"  VERIFICACION TOTAL: ");
        g.dec(PASOS.len() as u64);
        g.text(b" paso(s) en orden, con un save antes de cada uno\n");
        g.with_ink(INK_PLAIN);
    }
    let armado = leer_modo().is_some();
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
            // La marca: si la maquina cae AHORA, el arranque siguiente lo sabe.
            if armado {
                escribir_modo(args, Some(i), tumbo);
            }
            let r = (paso.dar)();
            if armado {
                escribir_modo(args, None, tumbo);
            }
            match r {
                Ok(_) => Salio::Bien,
                Err(m) => Salio::No(m),
            }
        };
        fila(dsk, paso, salio[i]);
    }
    // Y el save de despues: lo que quedo, tambien en el disco.
    let _ = super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo());
    notas(dsk, &salio, armado);
    super::iommu::report_iommu(&mut dsk.out.grid);
    consejero(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "verificacion total", INK_DIM);
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

/// **EL CONSEJERO** (24-09): lo que la caja recomienda AHORA, mirando la
/// maquina y no un texto fijo. Sale cada vez que Ctrl+Alt invoca la caja, al
/// arrancar, y al acabar `save mode`. Los pasos salen de [`PASOS`]: el dia que
/// E2 sea un paso, el consejero lo recomienda sin que nadie lo toque.
pub(crate) fn consejero(g: &mut crate::scene::output::Output) {
    let modo = leer_modo();
    g.with_ink(INK_GOOD);
    g.text(b"  CONSEJERO: ");
    g.with_ink(INK_PLAIN);
    let siguiente = PASOS.iter().position(|p| !(p.hecho)());
    match (modo.as_ref().and_then(|m| m.tumbo), siguiente) {
        (Some(i), _) => {
            g.text(b"el paso `");
            g.text(PASOS[i].nombre);
            g.text(b"` TUMBO la maquina un arranque atras y quedo quitado; antes de volver a darlo, `save` y a mano\n");
        }
        (None, Some(i)) => {
            g.text(b"lo siguiente es `");
            g.text(PASOS[i].nombre);
            g.text(b"` (");
            g.text(PASOS[i].que);
            g.text(b"): `save mode` lo da con un save antes\n");
        }
        (None, None) => {
            g.text(b"todo verificado (");
            for (i, p) in PASOS.iter().enumerate() {
                if i > 0 {
                    g.text(b", ");
                }
                g.text(p.nombre);
            }
            g.text(b"); lo siguiente del plan es E2, el VBLANK de la 3060 por MSI (todavia no es un paso)\n");
        }
    }
    g.with_ink(INK_ECHO);
    match &modo {
        Some(m) => {
            g.text(b"             save mode ARMADO (`save mode");
            if m.n > 0 {
                g.text(b" ");
                g.text(m.args());
            }
            g.text(b"`): se repite solo si la maquina se reinicia o cae; `save mode off` lo desarma\n");
        }
        None => g.text(b"             save mode SIN ARMAR: `save mode` lo corre y lo deja armado para el proximo arranque\n"),
    }
    g.text(b"             quita pasos con");
    for p in PASOS {
        g.text(b" -");
        g.text(p.nombre);
    }
    g.text(b"   |   `save auto` / `save manual`: guardar solo antes de lo arriesgado\n");
    g.with_ink(INK_PLAIN);
}

/// **Notas y consejos**: que mirar ahora, paso a paso.
fn notas(dsk: &mut Desktop, salio: &[Salio; 8], armado: bool) {
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
    g.text(if armado {
        b"    al reiniciar la maquina APAGA todo esto, y el modo ARMADO lo vuelve a dar solo\n" as &[u8]
    } else {
        b"    al reiniciar todo esto vuelve a APAGADO, y el modo no esta armado: no se repite\n"
    });
    g.with_ink(INK_PLAIN);
}
