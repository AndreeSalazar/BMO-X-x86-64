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

use super::pasos::{Paso, PASOS};

/// Cuantos pasos caben. Eran 8 y `vbios` hizo el octavo (24-09): con L0 en
/// camino, se deja sitio -- y la prueba de abajo dice NO si se pasa. Eran 16
/// y `sistema` hizo el decimosexto (L0c4b2a, 24-09): L0c4b2b y L0c4b2c vienen.
/// Y eran 24 y L1c los llevo a 23 (24-09): con L1d y M5 detras, 32. Y `gr`
/// (M5 G0, 24-09) hizo el trigesimo segundo: con G1..G4 y el triangulo, 48.
/// Y `escena` (M5d E, 24-09) hizo el cuadragesimo septimo: con T1b y T1c
/// detras, 64.
const MAX_PASOS: usize = 64;
const _: () = assert!(PASOS.len() <= MAX_PASOS, "save mode: mas pasos que MAX_PASOS");

/// Que salio de cada paso.
#[derive(Clone, Copy, PartialEq)]
enum Salio {
    Quitado,
    /// Un save de antes no se pudo escribir: de ahi en adelante, nada.
    Parado,
    YaEstaba,
    FaltaOtro,
    /// L0c5: el GSP ya se apago en orden en este arranque; la 3060 no trabaja.
    GspApagado,
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
/// Pasos que ya no existen: un `-nombre` suyo en un `datos/modo.txt` de antes
/// se ignora, en vez de desarmar el modo por un nombre desconocido.
const RETIRADOS: &[&[u8]] = &[b"-apagado"];

fn quitados_de(args: &[u8]) -> Result<[bool; MAX_PASOS], &[u8]> {
    let mut quitados = [false; MAX_PASOS];
    for t in args.split(|&b| b == b' ').filter(|t| !t.is_empty() && !RETIRADOS.contains(t)) {
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
        // Si el panel ya salio tras el gato y ahora no se lee el modo, se
        // devuelve el escritorio (sin 3060: no corrio nada).
        crate::desktop::arranque::acabar(dsk, p);
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
        crate::desktop::arranque::acabar(dsk, p);
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
    // ** EL ARRANQUE ORQUESTADO (25-09): mientras se repite, la pantalla es
    // el panel del arranque, no el escritorio; al final, la 3060 toma el
    // control (`desktop::arranque`).
    crate::desktop::arranque::seguir(p, PASOS.len());
    correr(dsk, p, &quitados, &copia[..n], tumbo);
}

/// **Justo tras el gato de la intro**: si `save mode` esta armado, el panel
/// del arranque orquestado sale YA, y el escritorio se prepara detras (sin
/// verse a trozos entre el gato y el panel). Lo llama `desktop::boot`.
pub(crate) fn antes_del_escritorio(p: &bmo::Pantalla) {
    if leer_modo().is_some() {
        crate::desktop::arranque::empezar(p, PASOS.len());
    }
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
    // ** EL SAVE DE EMERGENCIA, ANTES DE TODO (24-09): aunque todos los pasos
    // esten hechos y no se arriesgue nada, lo que la maquina es AHORA queda en
    // el disco (datos/) desde el primer instante. Los de antes de cada paso
    // vienen despues.
    {
        let ok = super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo()).is_ok();
        let g = &mut dsk.out.grid;
        g.with_ink(if ok { INK_GOOD } else { INK_ERR });
        g.text(if ok {
            b"  save de emergencia ESCRITO antes de todo: el informe y los datos ya estan en datos/\n" as &[u8]
        } else {
            b"  el save de emergencia NO se pudo escribir: mira `disco` (los pasos lo intentan otra vez)\n"
        });
        g.with_ink(INK_PLAIN);
    }
    let mut salio = [Salio::Quitado; MAX_PASOS];
    let mut tiempo = [0u64; MAX_PASOS];
    let mut intentos = [0u8; MAX_PASOS];
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let mut parado = false;
    // ** L0c5 (metal 24-09 20:36): tras `apagado` la 3060 no trabaja hasta el
    // siguiente arranque. Un `save mode` otra vez en el MISMO arranque volvia
    // a dar `fwsec` (su WPR2 ya estaba abajo, que es justo lo que se busca)
    // y salia en rojo. Lo que no esta hecho no se intenta: se dice por que.
    let apagado = super::gspapagar::hecho();
    if apagado {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ECHO);
        g.text(b"  el GSP ya se APAGO EN ORDEN en este arranque: los pasos de la 3060 que falten se dan en el siguiente\n");
        g.with_ink(INK_PLAIN);
    }
    for (i, paso) in PASOS.iter().enumerate() {
        salio[i] = if parado {
            Salio::Parado
        } else if quitados[i] {
            Salio::Quitado
        } else if (paso.hecho)() {
            Salio::YaEstaba
        } else if apagado {
            Salio::GspApagado
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
                fila(dsk, paso, Salio::Parado, 0, 0);
                continue;
            }
            // La marca: si la maquina cae AHORA, el arranque siguiente lo sabe.
            if armado {
                escribir_modo(args, Some(i), tumbo);
            }
            // Por donde va, en la linea de estado: un paso de varios segundos
            // sin decir cual es parece una maquina colgada.
            let arranque = crate::desktop::arranque::activo();
            if arranque {
                crate::desktop::arranque::paso(p, i, paso.nombre, paso.que);
            } else {
                crate::scene::sugerir::pista(p, &dsk.run_box, b"save mode", paso.nombre);
            }
            let desde = bmo::ciclos();
            let mut r = (paso.dar)();
            intentos[i] = 1;
            // ** EL REINTENTO (24-09): un paso que solo PREGUNTA y no sale se
            // da otra vez, una sola: una respuesta que tarda o un mensaje
            // del GSP por medio no deberian tumbar todo lo que va detras.
            if r.is_err() && reintentable(paso.nombre) {
                r = (paso.dar)();
                intentos[i] = 2;
            }
            tiempo[i] = (bmo::ciclos() - desde) * 1_000_000 / hz;
            if armado {
                escribir_modo(args, None, tumbo);
            }
            if paso.repinta && r.is_ok() {
                if arranque {
                    crate::desktop::arranque::repintar();
                } else {
                    crate::repintar_escritorio(p, dsk, "save mode");
                }
            }
            match r {
                Ok(_) => Salio::Bien,
                Err(m) => Salio::No(m),
            }
        };
        fila(dsk, paso, salio[i], tiempo[i], intentos[i]);
        crate::desktop::arranque::salio(i, paso.nombre, al_panel(salio[i]), tiempo[i]);
    }
    // Y el save de despues: lo que quedo, tambien en el disco.
    let _ = super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo());
    let escrito = escribir_pasos(&salio, &tiempo, &intentos, tumbo);
    resumen(dsk, &salio, tumbo, escrito);
    notas(dsk, &salio, armado);
    avisar_fin(&salio);
    // El arranque orquestado acaba aqui: la 3060 toma el control y el
    // escritorio vuelve (antes de armar el volcado, que es de ese escritorio).
    crate::desktop::arranque::acabar(dsk, p);
    // ** Y con el volcado verificado, la 3060 vuelca CADA fotograma desde ya.
    super::gspvolcado::activar(&mut dsk.out.grid, p);
    super::iommu::report_iommu(&mut dsk.out.grid);
    consejero(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "verificacion total", INK_DIM);
}

/// Como sale un paso en el panel del arranque orquestado.
fn al_panel(s: Salio) -> u8 {
    use crate::scene::arranque as sa;
    match s {
        Salio::Bien => sa::BIEN,
        Salio::YaEstaba => sa::YA,
        Salio::No(_) => sa::MAL,
        Salio::Quitado | Salio::Parado | Salio::FaltaOtro | Salio::GspApagado => sa::SALTO,
    }
}

/// Los pasos que solo PREGUNTAN al GSP-RM (o leen): darlos dos veces no
/// cambia nada en la 3060, asi que un fallo se reintenta una vez.
const REINTENTABLES: &[&[u8]] = &[b"estatica", b"objetos", b"salud", b"espacio", b"motores", b"ficha", b"gr", b"fichagr"];

fn reintentable(nombre: &[u8]) -> bool {
    REINTENTABLES.contains(&nombre)
}

fn fila(dsk: &mut Desktop, paso: &Paso, s: Salio, us: u64, intentos: u8) {
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
        Salio::GspApagado => {
            g.with_ink(INK_ECHO);
            g.text(b"NO SE INTENTA: el GSP ya se apago en orden en este arranque");
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
        Salio::No(_) => {
            g.with_ink(INK_ERR);
            g.text(b"NO");
        }
    }
    if matches!(s, Salio::Bien | Salio::No(_)) {
        g.with_ink(INK_ECHO);
        g.text(b" en ");
        if us >= 10_000 {
            g.dec(us / 1000);
            g.text(b" ms");
        } else {
            g.dec(us);
            g.text(b" us");
        }
        if intentos > 1 {
            g.text(b" (al 2o intento)");
        }
    }
    match s {
        Salio::Quitado | Salio::YaEstaba | Salio::Parado | Salio::FaltaOtro | Salio::GspApagado | Salio::Bien => {}
        Salio::No(m) => {
            g.with_ink(INK_ERR);
            g.text(b": ");
            g.text(super::iommu::motivo(m));
        }
    }
    g.with_ink(INK_ECHO);
    g.text(b"   -- ");
    g.text(paso.que);
    g.with_ink(INK_PLAIN);
    g.byte(b'\n');
}

/// **EL RESUMEN** (24-09): una linea con la cuenta, y el primer paso que no
/// salio con su motivo -- lo primero que hay que leer.
fn resumen(dsk: &mut Desktop, salio: &[Salio; MAX_PASOS], tumbo: Option<usize>, escrito: bool) {
    let n = PASOS.len();
    let cuenta = |f: fn(&Salio) -> bool| salio[..n].iter().filter(|s| f(s)).count() as u64;
    let bien = cuenta(|s| matches!(s, Salio::Bien | Salio::YaEstaba));
    let g = &mut dsk.out.grid;
    g.separar();
    g.with_ink(if bien as usize == n { INK_GOOD } else { INK_PLAIN });
    g.text(b"  RESUMEN  ");
    g.dec(bien);
    g.text(b" de ");
    g.dec(n as u64);
    g.text(b" pasos bien (");
    g.dec(cuenta(|s| matches!(s, Salio::Bien)));
    g.text(b" hechos ahora, ");
    g.dec(cuenta(|s| matches!(s, Salio::YaEstaba)));
    g.text(b" ya estaban)");
    let quitados = cuenta(|s| matches!(s, Salio::Quitado));
    if quitados > 0 {
        g.text(b", ");
        g.dec(quitados);
        g.text(b" quitados");
    }
    let tras_apagar = cuenta(|s| matches!(s, Salio::GspApagado));
    if tras_apagar > 0 {
        g.text(b", ");
        g.dec(tras_apagar);
        g.text(b" para el siguiente arranque (el GSP ya se apago)");
    }
    g.with_ink(INK_PLAIN);
    g.byte(b'\n');
    if let Some(i) = salio[..n].iter().position(|s| matches!(s, Salio::No(_) | Salio::Parado)) {
        g.with_ink(INK_ERR);
        g.text(b"           se paro en `");
        g.text(PASOS[i].nombre);
        g.text(b"`: ");
        g.text(match salio[i] {
            Salio::No(m) => super::iommu::motivo(m),
            _ => b"el save de antes no se pudo escribir",
        });
        g.with_ink(INK_PLAIN);
        g.byte(b'\n');
    }
    if let Some(i) = tumbo {
        g.with_ink(INK_ERR);
        g.text(b"           `");
        g.text(PASOS[i].nombre);
        g.text(b"` TUMBO la maquina una vez: sigue quitado\n");
        g.with_ink(INK_PLAIN);
    }
    g.with_ink(INK_ECHO);
    g.text(if escrito {
        b"           una linea por paso en datos/pasos.txt: pega ESE fichero, no el informe entero\n" as &[u8]
    } else {
        b"           datos/pasos.txt no se pudo escribir\n"
    });
    g.with_ink(INK_PLAIN);
}

/// **`datos/pasos.txt`** (24-09): una linea por paso -- nombre, que salio,
/// cuanto tardo y el motivo --, para pegarlo entero en vez del informe. Lo
/// que no cabe en una linea esta en el informe maestro de al lado.
fn escribir_pasos(salio: &[Salio; MAX_PASOS], tiempo: &[u64; MAX_PASOS], intentos: &[u8; MAX_PASOS], tumbo: Option<usize>) -> bool {
    let Ok(a) = bmo::Archivo::create(b"datos/pasos.txt") else { return false };
    a.write(b"save mode -- una linea por paso (el detalle, en el informe maestro)\n");
    for (i, paso) in PASOS.iter().enumerate() {
        let mut t = [0u8; 160];
        let estado: &[u8] = match salio[i] {
            Salio::Quitado => b"QUITADO",
            Salio::Parado => b"PARADO",
            Salio::YaEstaba => b"ya estaba",
            Salio::FaltaOtro => b"no se intento",
            Salio::GspApagado => b"no se intento: el GSP ya se apago en este arranque",
            Salio::Bien => b"HECHO",
            Salio::No(_) => b"NO",
        };
        let mut us = [0u8; 20];
        let nu = decimal(&mut us, tiempo[i]);
        let mut n = juntar(&mut t, &[paso.nombre, b": ", estado]);
        if matches!(salio[i], Salio::Bien | Salio::No(_)) {
            n += juntar(&mut t[n..], &[b" en ", &us[..nu], b" us"]);
            if intentos[i] > 1 {
                n += juntar(&mut t[n..], &[b" (2o intento)"]);
            }
        }
        if let Salio::No(m) = salio[i] {
            n += juntar(&mut t[n..], &[b" -- ", super::iommu::motivo(m)]);
        }
        if tumbo == Some(i) {
            n += juntar(&mut t[n..], &[b" -- TUMBO la maquina una vez"]);
        }
        a.write(&t[..n]);
        a.write(b"\n");
    }
    a.close()
}

/// `v` en decimal en `t`. Devuelve cuantos bytes.
fn decimal(t: &mut [u8; 20], mut v: u64) -> usize {
    let mut k = t.len();
    loop {
        k -= 1;
        t[k] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let n = t.len() - k;
    t.copy_within(k.., 0);
    n
}

/// Escribe `partes` en `t` hasta donde quepa. Devuelve cuanto escribio.
fn juntar(t: &mut [u8], partes: &[&[u8]]) -> usize {
    let mut n = 0;
    for p in partes {
        let k = p.len().min(t.len() - n);
        t[n..n + k].copy_from_slice(&p[..k]);
        n += k;
    }
    n
}

/// **LA PISTA** (24-09): el consejero en UNA linea, para la linea de estado
/// de la caja. `(etiqueta, cuanto de t)`. Ver `desktop::paint::pista_consejero`.
pub(crate) fn pista(t: &mut [u8]) -> (&'static [u8], usize) {
    let modo = leer_modo();
    let armado: &[u8] = if modo.is_some() { b"  (armado)" } else { b"" };
    // Caliente va primero: con la 3060 asi, ningun paso de despues sale, y
    // "siguiente: despertar" mandaria a repetir lo que no puede salir.
    if !super::gsp::despierto() && super::gsp::caliente() {
        return (b"cuidado", juntar(t, &[super::gsp::CALIENTE]));
    }
    match (modo.as_ref().and_then(|m| m.tumbo), PASOS.iter().position(|p| !(p.hecho)())) {
        (Some(i), _) => (b"cuidado", juntar(t, &[b"`", PASOS[i].nombre, b"` tumbo la maquina: quitado de save mode, a mano y con save"])),
        (None, Some(i)) => (b"siguiente", juntar(t, &[b"save mode -> ", PASOS[i].nombre, b": ", PASOS[i].que, armado])),
        (None, None) => (b"verificado", juntar(t, &[b"los ", paso_n(), b" pasos; `gpu` lo muestra todo", armado])),
    }
}

/// Cuantos pasos, en texto (hasta 99).
fn paso_n() -> &'static [u8] {
    const N: [u8; 2] = [b'0' + (PASOS.len() / 10) as u8, b'0' + (PASOS.len() % 10) as u8];
    if PASOS.len() < 10 {
        &N[1..]
    } else {
        &N
    }
}

/// **EL CONSEJERO** (24-09): lo que la caja recomienda AHORA, mirando la
/// maquina y no un texto fijo. Sale al arrancar y al acabar `save mode`; al
/// invocar la caja (Ctrl+Alt) sale solo su [`pista`], en la linea de estado:
/// agregarlo a la salida cada vez la mezclaba (el propietario, 24-09). Dos
/// lineas, alineadas: lo siguiente, y el modo.
pub(crate) fn consejero(g: &mut crate::scene::output::Output) {
    let modo = leer_modo();
    g.separar();
    g.with_ink(INK_GOOD);
    g.text(b"  CONSEJERO  ");
    g.with_ink(INK_PLAIN);
    let mut t = [0u8; 160];
    let (etiqueta, n) = pista(&mut t);
    g.text(etiqueta);
    g.text(b": ");
    g.text(&t[..n]);
    g.byte(b'\n');
    g.with_ink(INK_ECHO);
    g.text(b"              ");
    match &modo {
        Some(m) => {
            g.text(b"modo ARMADO (`save mode");
            if m.n > 0 {
                g.text(b" ");
                g.text(m.args());
            }
            g.text(b"`): se repite si la maquina cae; `save mode off` lo desarma\n");
        }
        None => g.text(b"`save mode` lo corre y lo deja ARMADO; `-paso` quita uno\n"),
    }
    g.with_ink(INK_PLAIN);
}

/// **Notas y consejos**: SOLO lo que importa ahora (24-09: salian los 32
/// consejos en cada arranque, y lo que habia que mirar se perdia entre
/// ellos). Cada paso que no salio, con que hacer; y el consejo del ULTIMO
/// que salio, que es lo que hay que mirar para seguir.
fn notas(dsk: &mut Desktop, salio: &[Salio; MAX_PASOS], armado: bool) {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_GOOD);
    g.text(b"  NOTAS Y CONSEJOS\n");
    g.with_ink(INK_PLAIN);
    let ultimo_bien = (0..PASOS.len()).rev().find(|&i| matches!(salio[i], Salio::Bien | Salio::YaEstaba));
    let mut dichas = 0;
    for (i, paso) in PASOS.iter().enumerate() {
        let texto: &[u8] = match salio[i] {
            Salio::Bien | Salio::YaEstaba if Some(i) == ultimo_bien => paso.consejo,
            Salio::Bien | Salio::YaEstaba | Salio::Quitado => continue,
            Salio::GspApagado if i > 0 && matches!(salio[i - 1], Salio::GspApagado) => continue,
            Salio::GspApagado => b"no se dio: el GSP ya se apago en orden en este arranque; arranca otra vez (sin cortar la corriente) y `save mode` lo da",
            Salio::Parado => b"no se dio: sin save de antes no se arriesga nada; mira `disco`",
            // Solo el primero de una cadena que no se dio: el resto es eco.
            Salio::FaltaOtro if i > 0 && matches!(salio[i - 1], Salio::FaltaOtro | Salio::No(_)) => continue,
            Salio::FaltaOtro => b"no se dio: el paso que pide no esta hecho (quita su `-`, o daselo a mano)",
            Salio::No(_) => b"NO salio: su fila en `gpu` dice por que; la foto de antes esta en el disco y `cabina fallos` lo ultimo",
        };
        g.text(b"    ");
        g.text(paso.nombre);
        g.text(b": ");
        g.text(texto);
        g.byte(b'\n');
        dichas += 1;
    }
    if dichas == 0 {
        g.text(b"    nada que mirar: ningun paso se dio\n");
    }
    g.with_ink(INK_ECHO);
    g.text(if armado {
        b"    al reiniciar la maquina APAGA todo esto, y el modo ARMADO lo vuelve a dar solo\n" as &[u8]
    } else {
        b"    al reiniciar todo esto vuelve a APAGADO, y el modo no esta armado: no se repite\n"
    });
    g.with_ink(INK_PLAIN);
}

/// **El globo al acabar `save mode`**: cuantos pasos salieron, en verde, o
/// cuantos NO, en rojo -- junto al puntero, sin tener que leer la salida.
fn avisar_fin(salio: &[Salio; MAX_PASOS]) {
    use crate::desktop::globo::{avisar, Tono};
    if !super::gsp::despierto() && super::gsp::caliente() {
        avisar(b"la 3060", b"viene CALIENTE (Windows o un reinicio): apaga del todo 15 s y vuelve", Tono::Mal);
        return;
    }
    let bien = salio[..PASOS.len()].iter().filter(|s| matches!(s, Salio::Bien | Salio::YaEstaba)).count();
    let no = salio[..PASOS.len()].iter().filter(|s| matches!(s, Salio::No(_))).count();
    let mut t = [0u8; 64];
    let mut n = 0;
    let mut poner = |s: &[u8]| {
        for &c in s {
            if n < t.len() {
                t[n] = c;
                n += 1;
            }
        }
    };
    let mut d = [0u8; 4];
    let cifra = |v: usize, d: &mut [u8; 4]| -> usize {
        let s = [b'0' + (v / 100 % 10) as u8, b'0' + (v / 10 % 10) as u8, b'0' + (v % 10) as u8];
        let k = if v >= 100 { 0 } else if v >= 10 { 1 } else { 2 };
        d[..3 - k].copy_from_slice(&s[k..]);
        3 - k
    };
    let k = cifra(bien, &mut d);
    poner(&d[..k]);
    poner(b" de ");
    let k = cifra(PASOS.len(), &mut d);
    poner(&d[..k]);
    if no == 0 {
        poner(b" pasos: la 3060 lista y verificada");
        avisar(b"save mode", &t[..n], Tono::Bien);
    } else {
        poner(b" pasos; ");
        let k = cifra(no, &mut d);
        poner(&d[..k]);
        poner(b" NO: mira `gpu`");
        avisar(b"save mode", &t[..n], Tono::Mal);
    }
}
