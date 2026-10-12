//! **`smp siempre`** (11-10): los doce hilos (seis nucleos x 2) en pie AL ARRANCAR, sin
//! teclear `smp all` cada vez. Lo pidio el propietario: *"el smp all y el
//! save intenta automatizar"*.
//!
//! [carril]  AMARILLO  despierta nucleos sin que nadie lo teclee en ESE
//!                     arranque: lo armo el propietario antes, y se dice
//! [consumo] NADA      una vez al arrancar, armado; desarmado, un fichero que
//!                     no esta
//!
//! Como `save siempre`: un fichero en el disco de datos (`informe/estado/smp.txt`)
//! dice si esta armado, y lo lee el escritorio al arrancar, DESPUES del save
//! del arranque (el que dice por que se reinicio la vez anterior).
//!
//! *** APAGADO POR DEFECTO, y no es prudencia de adorno: hoy (V9 y V10 de
//! `PLAN_LOS_DOCE_DIRECTORES`) `smp all` vive solo, pero una app que reparte
//! en Ring 3 (ESTUDIO, PROTON-X) con los obreros en pie REINICIA el PC. Armarlo
//! es decidir que cada arranque nace con ese riesgo; por eso al armarlo se
//! dice, y al arrancar armado tambien.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Donde se guarda si esta armado (8.3, el disco de datos).
const ARMADO_EN: &[u8] = b"informe/estado/smp.txt";

/// **`smp siempre` / `smp siempre off`**. `None` si `arg` no es suyo.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> Option<After> {
    let armar = match arg {
        b"siempre" => true,
        b"siempre off" | b"siempre no" => false,
        _ => return None,
    };
    let escrito = bmo::Archivo::create(ARMADO_EN).is_ok_and(|a| {
        a.write(if armar { b"smp siempre\n" as &[u8] } else { b"smp siempre off\n" });
        a.close()
    });
    let g = &mut dsk.out.grid;
    if armar {
        g.with_ink(INK_GOOD);
        g.text(b"  smp SIEMPRE: armado. Al arrancar, despues del save del arranque, se despierta a todos\n");
        g.with_ink(INK_ERR);
        g.text(b"  [!] hoy una app que reparte en Ring 3 (estudio, PROTON-X) con ellos en pie REINICIA el PC (V9, V10)\n");
    } else {
        g.with_ink(INK_ECHO);
        g.text(b"  smp siempre: desarmado. Al arrancar, solo el BSP; `smp all` cuando lo quieras\n");
    }
    if !escrito {
        g.with_ink(INK_ERR);
        g.text(b"  (no pude escribir informe/estado/smp.txt: no vale para el arranque siguiente)\n");
    }
    g.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, if armar { "smp siempre" } else { "smp a mano" }, INK_DIM);
    dsk.field.n = 0;
    Some(After::Settle)
}

/// **Al arrancar**, una vez: si esta armado, todos en pie, y se dice en la caja.
pub(crate) fn al_arrancar(dsk: &mut Desktop, _p: &bmo::Pantalla) {
    let mut b = [0u8; 32];
    let n = bmo::Archivo::leer_de(ARMADO_EN).map_or(0, |a| {
        let n = a.read(&mut b);
        a.close();
        n
    });
    if !b[..n].starts_with(b"smp siempre") || b[..n].starts_with(b"smp siempre off") {
        return;
    }
    let c = bmo::smp_pedir(u32::MAX);
    let g = &mut dsk.out.grid;
    g.with_ink(if c.vivos == c.esperados { INK_GOOD } else { INK_ERR });
    g.text(b"  smp siempre: ");
    g.dec(c.vivos as u64 + 1);
    g.text(b" de ");
    g.dec(c.esperados as u64 + 1);
    g.text(b" hilos en pie al arrancar");
    g.with_ink(INK_ECHO);
    g.text(b"   (`smp siempre off` lo desarma)\n");
    g.with_ink(INK_PLAIN);
    super::arranque::lineas_smp(g);
}
