//! **`gpu g0`: las pruebas contra el 0x15** (17-10, GCB2c de
//! PLAN_GSP_CONTROL_BMO). El propietario elige que probar en el SIGUIENTE
//! arranque, sin recompilar: una palabra por experimento en
//! `informe/estado/g0.txt`, que el kernel lee al montar los datos.
//!
//! ```text
//!    gpu g0                    lo pedido, lo que corrio y que hace cada una
//!    gpu g0 riscv seguido      lo guarda; se aplica al reiniciar
//!    gpu g0 normal             el arranque de siempre
//! ```
//!
//! [consumo] NADA      corre cuando el propietario lo teclea
use bmo_nvidia_gsp_control::g0;
use bmo_userland as bmo;

use super::After;
use super::tabla::{campo, section};
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ERR, INK_GOOD, INK_PLAIN};

const RUTA: &[u8] = b"informe/estado/g0.txt";

/// Que hace cada palabra, en una linea.
const QUE: [(&[u8], &[u8]); 4] = [
    (b"riscv", b"E3: el GSP como OpenRM 570.144 -- nucleo RISC-V y buzones, sin arrancar el falcon vacio"),
    (b"seguido", b"E4: el booter ya cargado en el SEC2 antes de despertar al GSP, sin el hueco de 120 ms"),
    (b"reset", b"E5: el RESET de GSP y SEC2 como OpenRM, diez lecturas por flanco"),
    (b"ptimer", b"E6: el reloj de la 3060 (PTIMER) en hora antes de FWSEC, como OpenRM"),
];

/// Las palabras de un modo, separadas por espacios; `normal` si ninguna.
fn palabras(s: &mut Output, modo: u64) {
    let mut alguna = false;
    for (p, bit) in g0::PALABRAS {
        if modo & bit != 0 {
            if alguna {
                s.byte(b' ');
            }
            s.text(p);
            alguna = true;
        }
    }
    if !alguna {
        s.text(b"normal");
    }
}

/// **Las filas de G0**, para `gpu g0` y para el informe: lo que el kernel leyo
/// al arrancar y lo que de verdad corrio. Solo lee.
pub(crate) fn filas(s: &mut Output) {
    let v = bmo::info(bmo::INFO_GPU_G0);
    campo(s, b"g0 pedido");
    if v & g0::MIRADO == 0 {
        s.text(b"sin dato (kernel anterior a G0, o los datos sin montar)\n");
        return;
    }
    palabras(s, v);
    s.text(if v & g0::FICHERO != 0 { b"  (informe/estado/g0.txt)" as &[u8] } else { b"  (sin fichero: el de siempre)" });
    if g0::desconocidas(v) > 0 {
        s.text(b"; palabras desconocidas ");
        s.dec(g0::desconocidas(v));
    }
    s.byte(b'\n');
    campo(s, b"g0 corrio");
    let usado = v >> 8;
    if usado & 0xFF == 0 {
        s.text(b"nada todavia");
    } else {
        palabras(s, usado);
    }
    s.byte(b'\n');
}

/// `gpu g0 [palabras]`.
pub(crate) fn orden(dsk: &mut Desktop, args: &[u8]) -> After {
    let g = &mut dsk.out.grid;
    let args = trim(args);
    if args.is_empty() {
        section(g, b"G0 -- las pruebas contra el 0x15 (GCB2c)");
        filas(g);
        for (p, que) in QUE {
            campo(g, p);
            g.text(que);
            g.byte(b'\n');
        }
        g.text(b"  `gpu g0 <palabras>` las guarda para el PROXIMO arranque; `gpu g0 normal` vuelve al de siempre.\n");
        g.text(b"  Una prueba cada vez, y un `save` al final salga bien o mal (GCB2B_ENTRADAS.md, seccion 4).\n");
        dsk.field.n = 0;
        return After::Settle;
    }
    let pedido = if args.eq_ignore_ascii_case(b"off") { b"normal" as &[u8] } else { args };
    let modo = g0::leer(pedido);
    if g0::desconocidas(modo) > 0 {
        g.with_ink(INK_ERR);
        g.text(b"  NO: solo valen riscv, seguido, reset, ptimer o normal; no se guardo nada\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    let guardado = match bmo::Archivo::create(RUTA) {
        Ok(a) => {
            a.write(b"# G0 (GCB2c): lo que se prueba en el proximo arranque. Ver `gpu g0`.\n");
            a.write(pedido);
            a.write(b"\n");
            a.close()
        }
        Err(_) => false,
    };
    if guardado {
        g.with_ink(INK_GOOD);
        g.text(b"  g0.txt guardado: ");
        palabras(g, modo);
        g.text(b". Se aplica al REINICIAR; despues, `save` y `gpu g0` dicen que corrio.\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  NO se pudo escribir informe/estado/g0.txt\n");
    }
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}

fn trim(mut s: &[u8]) -> &[u8] {
    while let [b' ', resto @ ..] = s {
        s = resto;
    }
    while let [resto @ .., b' '] = s {
        s = resto;
    }
    s
}
