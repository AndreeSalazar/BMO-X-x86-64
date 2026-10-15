//! **`cpu contadores`: LOS CONTADORES DEL ZEN 3, Y SI CUENTAN LO QUE DICE EL
//! PAPEL** (C4 de `docs/plan/EL_FOCO.md`, LB4, 15-10).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea; la calibracion,
//!                     en el kernel, UNA vez por arranque
//!
//! El kernel corre tres bucles que se saben de memoria y cuenta cada evento
//! (`cpu_vendor/ryzen_5_5600x/contadores.rs`); el juez es
//! `bmo_contadores::juzgar`. Aqui solo se pinta una fila por cuenta: el
//! evento, lo contado, lo esperado y el veredicto. Un evento `PorConfirmar`
//! se cree cuando su fila diga CUENTA en ESTE Ryzen.

use bmo_contadores as pc;
use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "calibrando los contadores del Zen 3", INK_DIM);
    let g = &mut dsk.out.grid;
    let estado = bmo::info(bmo::INFO_CPU_CONTADORES);
    if estado & 1 == 0 {
        g.with_ink(INK_ERR);
        g.text(b"  NO: los contadores no se tocan aqui (no es la CPU del perfil, o no tiene contadores extendidos)\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    let filas = (estado >> 8 & 0xFF) as usize;
    let mut buenas = 0;
    for k in 0..filas {
        let w = bmo::info(bmo::INFO_CPU_CONTADOR | (k as u64) << 8);
        let (prueba, e, contado, v) = pc::desempaquetar(w);
        let Some(ev) = pc::EVENTOS.get(e) else { continue };
        let n = pc::vueltas(prueba);
        let (ink, que): (_, &[u8]) = match v {
            1 => (INK_GOOD, b"CUENTA lo que dice el PPR"),
            2 => (INK_ERR, b"cuenta OTRA cosa: no se cree"),
            3 => (INK_ERR, b"no conto NADA"),
            _ => (INK_ERR, b"sin medir"),
        };
        buenas += (v == 1) as usize;
        g.text(b"  ");
        g.text(match prueba {
            pc::Prueba::Recta => b"recta       " as &[u8],
            pc::Prueba::Azar => b"azar        ",
            pc::Prueba::Persecucion => b"persecucion ",
        });
        g.text(ev.nombre.as_bytes());
        g.text(b": ");
        g.dec(contado);
        g.text(b" en ");
        g.dec(n);
        g.text(b" vueltas");
        if let Some((a, b)) = pc::esperado(prueba, e, n) {
            g.with_ink(INK_ECHO);
            g.text(b" (esperado ");
            g.dec(a);
            g.text(b"..");
            g.dec(b);
            g.text(b")");
        }
        g.with_ink(ink);
        g.text(b" -- ");
        g.text(que);
        if ev.confianza == pc::Confianza::PorConfirmar {
            g.with_ink(INK_ECHO);
            g.text(b" [por confirmar en el metal]");
        }
        g.with_ink(INK_PLAIN);
        g.byte(b'\n');
    }
    g.with_ink(if buenas == filas { INK_GOOD } else { INK_ERR });
    g.text(b"  ");
    g.dec(buenas as u64);
    g.text(b" de ");
    g.dec(filas as u64);
    g.text(b" filas cuentan lo que dice el papel (C4: apuntalo en la hoja 6b)\n");
    g.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, "contadores", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}
