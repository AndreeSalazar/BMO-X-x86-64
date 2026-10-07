//! **LA ALARMA MAESTRA** -- el panel de precaucion y aviso de F9 (HM5b de
//! `docs/plan/PLAN_EL_HUD.md`, 07-10), delante de la autopsia.
//!
//! [consumo] NADA      corre cuando se pulsa F9 o se teclea `fallo` (L6h)
//!
//! El dia malo, como en una cabina: arriba la ALARMA MAESTRA (NOMINAL, o
//! ENCENDIDA con cuantas luces), y debajo una luz por sistema, cada una con lo
//! que el kernel YA sabe de el. La consola colorea por renglon, asi que cada
//! luz es un renglon con su color:
//!
//! ```text
//!    ALARMA MAESTRA  --  NOMINAL
//!    [  OK  ]  RING 3     ningun fallo desde el arranque
//!    [ AVISO]  CABINA     12 eventos se cayeron del anillo
//!    [FALLO ]  DISCO      no contesta
//!    [NO HAY]  LA 3060    dormida: la despierta `save mode`
//! ```
//!
//! ** Solo preguntas sin efecto (`bmo::info` y la caja negra): el dia malo no
//! es el dia de reclamar un aparato para preguntarle -- `audio_tubo` reclama
//! el de sonido, y por eso aqui no se usa.

use bmo_userland as bmo;

use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

/// Como esta una luz.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Luz {
    Ok,
    /// Algo que mirar, pero el sistema anda.
    Aviso,
    Fallo,
    /// No hay ese aparato: no es un fallo, y no enciende la alarma.
    NoHay,
}

/// Una luz: el sistema, como esta y el porque (con un numero si lo hay).
struct Renglon {
    nombre: &'static [u8],
    luz: Luz,
    dice: &'static [u8],
    numero: Option<u64>,
    cola: &'static [u8],
}

const fn r(nombre: &'static [u8], luz: Luz, dice: &'static [u8]) -> Renglon {
    Renglon { nombre, luz, dice, numero: None, cola: b"" }
}

/// **Las luces**: cada una de lo que el kernel ya contesta.
fn luces() -> [Renglon; 10] {
    let fallos = bmo::autopsia_total();
    let ring3 = if fallos == 0 {
        r(b"RING 3", Luz::Ok, b"ningun fallo desde el arranque")
    } else {
        Renglon { nombre: b"RING 3", luz: Luz::Fallo, dice: b"", numero: Some(fallos), cola: b" fallo(s) desde el arranque: la autopsia, abajo" }
    };
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    let memoria = if total == 0 {
        r(b"MEMORIA", Luz::Fallo, b"el kernel no da la cuenta")
    } else if libre < total / 20 {
        Renglon { nombre: b"MEMORIA", luz: Luz::Aviso, dice: b"queda menos del 5 %: ", numero: Some(libre / (1024 * 1024)), cola: b" MiB libres" }
    } else {
        Renglon { nombre: b"MEMORIA", luz: Luz::Ok, dice: b"", numero: Some(libre / (1024 * 1024)), cola: b" MiB libres" }
    };
    let perdidos = bmo::cabina_perdidos();
    let cabina = if perdidos == 0 {
        r(b"CABINA", Luz::Ok, b"el anillo no ha perdido nada")
    } else {
        Renglon { nombre: b"CABINA", luz: Luz::Aviso, dice: b"", numero: Some(perdidos), cola: b" eventos se cayeron del anillo (`cabina radar`)" }
    };
    let huecos = bmo::info(bmo::INFO_AUDIO_HUECOS);
    let sonido = if bmo::info(bmo::INFO_AUDIO_APARATO) == 0 {
        r(b"SONIDO", Luz::NoHay, b"no hay aparato de sonido")
    } else if huecos > 0 {
        Renglon { nombre: b"SONIDO", luz: Luz::Aviso, dice: b"", numero: Some(huecos), cola: b" huecos: el que produce llego tarde (`audio`)" }
    } else {
        r(b"SONIDO", Luz::Ok, b"sin huecos")
    };
    let caida = bmo::info(bmo::INFO_CAIDA_RECUPERADO);
    let caja = if caida == 0 {
        r(b"CAJA NEGRA", Luz::Ok, b"el arranque anterior no dejo rastro")
    } else {
        Renglon { nombre: b"CAJA NEGRA", luz: Luz::Aviso, dice: b"el arranque anterior se cayo: ", numero: Some(caida), cola: b" bytes en CAIDA.TXT" }
    };
    let si = |c: bool, nombre: &'static [u8], bien: &'static [u8], mal: &'static [u8], falta: Luz| if c { r(nombre, Luz::Ok, bien) } else { r(nombre, falta, mal) };
    [
        ring3,
        memoria,
        si(bmo::info(bmo::INFO_TSC_HZ) != 0, b"RELOJ", b"la frecuencia esta medida", b"sin frecuencia medida: las esperas no saben cuanto duran", Luz::Fallo),
        si(bmo::info(bmo::INFO_DISCO_LISTO) != 0, b"DISCO", b"listo", b"no contesta", Luz::Fallo),
        si(bmo::info(bmo::INFO_DATOS_MONTADO) != 0, b"ESTRATOS", b"volumen de datos montado", b"sin volumen de datos", Luz::Fallo),
        si(bmo::info(bmo::INFO_NET_PRESENTE) != 0, b"RED", b"tarjeta presente", b"sin tarjeta de red", Luz::NoHay),
        si(bmo::info(bmo::INFO_IOMMU_GPU) & bmo::IOMMU_GPU_TRADUCIDA != 0, b"LA 3060", b"traducida y lista", b"dormida: la despierta `save mode`", Luz::NoHay),
        sonido,
        cabina,
        caja,
    ]
}

/// **Escribe el panel** en la consola: la alarma y sus luces.
pub(crate) fn report_alarma(s: &mut Output) {
    let ls = luces();
    let encendidas = ls.iter().filter(|l| matches!(l.luz, Luz::Fallo | Luz::Aviso)).count();
    let fallos = ls.iter().filter(|l| l.luz == Luz::Fallo).count();
    super::tabla::section(s, b"alarma maestra");
    if encendidas == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"    ALARMA MAESTRA  --  NOMINAL: todas las luces apagadas\n");
    } else {
        s.with_ink(if fallos > 0 { INK_ERR } else { INK_ECHO });
        s.text(b"    ALARMA MAESTRA  --  ENCENDIDA: ");
        s.dec(encendidas as u64);
        s.text(if encendidas == 1 { b" luz\n" } else { b" luces\n" });
    }
    for l in &ls {
        let (marca, tinta): (&[u8], u8) = match l.luz {
            Luz::Ok => (b"[  OK  ]", INK_GOOD),
            Luz::Aviso => (b"[ AVISO]", INK_ECHO),
            Luz::Fallo => (b"[FALLO ]", INK_ERR),
            Luz::NoHay => (b"[NO HAY]", INK_PLAIN),
        };
        s.with_ink(tinta);
        s.text(b"    ");
        s.text(marca);
        s.text(b"  ");
        s.text(l.nombre);
        for _ in l.nombre.len()..12 {
            s.byte(b' ');
        }
        s.text(l.dice);
        if let Some(n) = l.numero {
            s.dec(n);
        }
        s.text(l.cola);
        s.byte(b'\n');
    }
    s.with_ink(INK_PLAIN);
}
