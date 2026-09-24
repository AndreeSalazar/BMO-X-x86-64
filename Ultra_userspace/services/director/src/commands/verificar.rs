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
//! Los pasos viven en [`PASOS`], en el orden en que hay que darlos. E2 (el
//! VBLANK por interrupcion) es la tercera fila desde el 2026-09-24.
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

/// CIEGA o TRADUCIDA: las dos le quitan la RAM. Sin lo segundo, `save mode`
/// volveria a cegar una 3060 ya traducida y desharia M0d.
fn gpu_ciega() -> bool {
    bmo::info(bmo::INFO_IOMMU_GPU) & (bmo::IOMMU_GPU_CIEGA | bmo::IOMMU_GPU_TRADUCIDA) != 0
}

fn gpu_traducida() -> bool {
    bmo::info(bmo::INFO_IOMMU_GPU) & bmo::IOMMU_GPU_TRADUCIDA != 0
}

fn traducir_gpu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_TRADUCIR_GPU)
}

fn prueba_prestada() -> bool {
    bmo::info(bmo::INFO_GPU_PRUEBA) & bmo::GPU_PRUEBA_PRESTADA != 0
}

fn prestar_prueba() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_PRESTAR_PRUEBA)
}

fn fuego_hecho() -> bool {
    bmo::info(bmo::INFO_GPU_FUEGO) & bmo::FUEGO_HECHO != 0
}

/// La prueba de fuego cuenta solo con las 1024 palabras: 1023 es un NO.
fn fuego() -> Result<u64, u32> {
    let v = bmo::iommu_orden(bmo::IOMMU_OP_GPU_FUEGO)?;
    if !fuego_hecho() {
        return Err(super::gpu::NO_FUEGO_A_MEDIAS);
    }
    Ok(v)
}

fn frontera_hecha() -> bool {
    bmo::info(bmo::INFO_GPU_FRONTERA) & bmo::FUEGO_HECHO != 0
}

fn frontera() -> Result<u64, u32> {
    let v = bmo::iommu_orden(bmo::IOMMU_OP_GPU_FRONTERA)?;
    if v == 0 {
        return Err(super::gpu::NO_SIN_FRONTERA);
    }
    Ok(v)
}

fn encender_iommu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_ENCENDER)
}

fn cegar_gpu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_CEGAR_GPU)
}

/// E2 cuenta como hecho si esta ARMADO y ya llego al menos un VBLANK: un
/// aviso encendido que no avisa no es un paso dado.
fn e2_avisa() -> bool {
    let v = bmo::info(bmo::INFO_GPU_VBLANK);
    v & bmo::E2_ARMADO != 0 && v & 0xFFFF_FFFF > 0
}

/// Encender E2 y ESCUCHAR 300 ms: a 60 Hz son ~18 avisos. Si no llega
/// ninguno, el paso no salio -- aunque el kernel dijera que si.
fn encender_e2() -> Result<u64, u32> {
    let v = bmo::iommu_orden(bmo::IOMMU_OP_E2_ENCENDER)?;
    let (n, _) = super::gpu::contar_vblanks(300);
    if n == 0 {
        return Err(super::gpu::NO_E2_MUDO);
    }
    Ok(v)
}

/// **Los pasos, en el orden en que hay que darlos.** Ver
/// `docs/plan/PLAN_LA_3060.md`: M0c, M0e, E2, M0d2 y M0d3.
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
    Paso {
        nombre: b"e2",
        que: b"la 3060 AVISA del VBLANK por MSI, con el Bus Master tras el candado (E2)",
        hecho: e2_avisa,
        dar: encender_e2,
        pide: Some(b"gpu"),
        consejo: b"teclea `gpu`: la fila `e2` tiene que subir ~60 por segundo y la escalera en `+`; `iommu` con los eventos en 0",
    },
    Paso {
        nombre: b"traducir",
        que: b"la 3060 TRADUCIDA por su dominio: ve SOLO lo prestado, hoy nada (M0d2)",
        hecho: gpu_traducida,
        dar: traducir_gpu,
        pide: Some(b"gpu"),
        consejo: b"`iommu`: la fila 3060 dice TRADUCIDA, sin fila `event`; y `gpu`: la fila `e2` sigue subiendo (el MSI pasa la entrada traducida)",
    },
    Paso {
        nombre: b"prestar",
        que: b"prestarle la pagina de prueba, solo lectura, en 0x10000000 (M0d2)",
        hecho: prueba_prestada,
        dar: prestar_prueba,
        pide: Some(b"traducir"),
        consejo: b"`iommu`: la fila `domain` dice 1 pagina prestada; lo siguiente es M0d3, que un falcon de la 3060 la LEA por DMA",
    },
    Paso {
        nombre: b"fuego",
        que: b"LA PRUEBA DE FUEGO: el DMA del falcon del GSP lee la pagina prestada (M0d3)",
        hecho: fuego_hecho,
        dar: fuego,
        pide: Some(b"prestar"),
        consejo: b"`gpu`: la fila `fuego` dice 1024 de 1024 -- la 3060 leyo tu RAM, y solo lo prestado",
    },
    Paso {
        nombre: b"frontera",
        que: b"LA FRONTERA: una direccion NO prestada sale como fallo de pagina de la 3060 (M0d3)",
        hecho: frontera_hecha,
        dar: frontera,
        pide: Some(b"fuego"),
        consejo: b"`iommu`: la fila `event` dice FALLO de pagina, BDF 29:00.0, direccion 0x20000000 -- la venda existe",
    },
    Paso {
        nombre: b"vbios",
        que: b"leer la VBIOS, hallar FWSEC y su firma para este fusible; solo lectura (L0a)",
        hecho: super::vbios::lista,
        dar: super::vbios::paso,
        pide: None,
        consejo: b"`gpu`: filas `vbios`, `fwsec`, `fusible`, `vram` y `wpr2`; la ROM queda en datos/vbios.rom -- lo siguiente es L0b, correr FWSEC-FRTS",
    },
    Paso {
        nombre: b"fwsec",
        que: b"CORRER FWSEC-FRTS: el firmware firmado de la VBIOS monta la WPR2 (L0b)",
        hecho: super::vbios::hay_wpr2,
        dar: super::vbios::correr_fwsec,
        pide: Some(b"vbios"),
        consejo: b"`gpu`: la fila `wpr2` dice YA montada donde se pidio, y `frts` CORRIO -- la puerta del booter y del GSP",
    },
    Paso {
        nombre: b"gsp",
        que: b"leer el firmware del GSP de fw/gsp/, la firma del booter para el fusible del SEC2 y el reparto de la VRAM; solo lectura (L0c1)",
        hecho: super::gsp::hecho,
        dar: super::gsp::leer,
        pide: None,
        consejo: b"`gpu`: filas `booter`, `fusible`, `bootldr`, `gsp-rm`, `mapa` y `cuadra` -- lo siguiente es L0c2, prestar el GSP-RM por la radix3",
    },
    Paso {
        nombre: b"radix",
        que: b"PRESTAR el GSP-RM a la 3060 por su radix3 y releerlo entero por la IOMMU, con su BLAKE3; no arranca nada (L0c2)",
        hecho: super::gsp::radix_hecho,
        dar: super::gsp::radix,
        pide: Some(b"gsp"),
        consejo: b"`gpu`: la fila `radix` dice PRESTADO y la 570.144 entera, blake3 e7856ee2b387917b; `iommu`: `domain` ~15.600 paginas y sin eventos nuevos -- lo siguiente es L0c3, el booter en el SEC2",
    },
    Paso {
        nombre: b"libos",
        que: b"PRESTAR para escribir lo que el GSP escribe (LIBOS, logs, rmargs, colas) y seguir cada puntero por la IOMMU; no arranca nada (L0c3a)",
        hecho: super::gsp::libos_hecho,
        dar: super::gsp::libos,
        pide: Some(b"radix"),
        consejo: b"`gpu`: la fila `libos` dice PRESTADO para escribir y cada puntero lleva a lo suyo; `iommu`: `domain` 180 paginas mas y sin eventos nuevos -- lo siguiente es L0c3b, despertar el GSP",
    },
    Paso {
        nombre: b"sistema",
        que: b"ESCRIBIRLE AL GSP por primera vez: SetSystemInfo y SetRegistry en la cola de la CPU, antes de despertarlo (L0c4b2a)",
        hecho: super::gspsistema::mandado,
        dar: super::gspsistema::mandar,
        pide: Some(b"libos"),
        consejo: b"`gpu`: la fila `sistema` dice los dos con suma 0, `sysinfo` las BAR y el PCI de tu 3060, y `leyo` que el GSP LOS LEYO (su puntero en 2) -- lo siguiente es L0c4b2b, leer el secuenciador",
    },
    Paso {
        nombre: b"despertar",
        que: b"DESPERTAR EL GSP: sus argumentos en el buzon, el booter firmado en el SEC2, y su RISC-V encendido (L0c3b)",
        hecho: super::gsp::despierto,
        dar: super::gsp::despertar,
        pide: Some(b"sistema"),
        consejo: b"`gpu`: la fila `despierto` dice el RISC-V ACTIVO y `gsplog` que el GSP ESCRIBIO; sus logs en datos/gsplog.bin -- lo siguiente es L0c4, hablarle por sus colas",
    },
    Paso {
        nombre: b"cola",
        que: b"LEER lo que el GSP ya dijo: los mensajes de su cola, sin contestar ni mover un puntero (L0c4a)",
        hecho: super::gspcola::leida,
        dar: super::gspcola::leer,
        pide: Some(b"despertar"),
        consejo: b"`gpu`: la fila `cola` dice cuantos mensajes, todos con firma y suma; `dijo` que tipos; la cola cruda en datos/gspcola.bin -- lo siguiente es L0c4b1, vaciarla",
    },
    Paso {
        nombre: b"vaciar",
        que: b"CONSUMIR lo que el GSP solo cuenta (NOCAT, LIBOS_PRINT) moviendo el puntero de lectura de la CPU; lo que pide algo se queda (L0c4b1)",
        hecho: super::gspvaciar::vaciada,
        dar: super::gspvaciar::vaciar,
        pide: Some(b"cola"),
        consejo: b"`gpu`: la fila `vacia` dice cuantos consumidos, `nocat` lo que traian en claro y `pide` el primero que espera respuesta; crudos en datos/gspnocat.bin -- lo siguiente es L0c4b2, contestarle",
    },
];

/// Cuantos pasos caben. Eran 8 y `vbios` hizo el octavo (24-09): con L0 en
/// camino, se deja sitio -- y la prueba de abajo dice NO si se pasa. Eran 16
/// y `sistema` hizo el decimosexto (L0c4b2a, 24-09): L0c4b2b y L0c4b2c vienen.
const MAX_PASOS: usize = 24;
const _: () = assert!(PASOS.len() <= MAX_PASOS, "save mode: mas pasos que MAX_PASOS");

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
fn quitados_de(args: &[u8]) -> Result<[bool; MAX_PASOS], &[u8]> {
    let mut quitados = [false; MAX_PASOS];
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
fn correr(dsk: &mut Desktop, p: &bmo::Pantalla, quitados: &[bool; MAX_PASOS], args: &[u8], tumbo: Option<usize>) {
    {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_GOOD);
        g.text(b"  VERIFICACION TOTAL: ");
        g.dec(PASOS.len() as u64);
        g.text(b" paso(s) en orden, con un save antes de cada uno\n");
        g.with_ink(INK_PLAIN);
    }
    let armado = leer_modo().is_some();
    let mut salio = [Salio::Quitado; MAX_PASOS];
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
/// arrancar, y al acabar `save mode`. Los pasos salen de [`PASOS`]: E2 entro
/// como fila el 24-09 y el consejero lo recomienda sin que nadie lo toque.
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
            g.text(b"); lo siguiente del plan es L0c4b2b: leer el secuenciador que pide el GSP, orden a orden, sin correrlo (todavia no es un paso)\n");
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
fn notas(dsk: &mut Desktop, salio: &[Salio; MAX_PASOS], armado: bool) {
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
