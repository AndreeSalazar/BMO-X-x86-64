//! **`metiche`: LO QUE EL HARDWARE APUNTO SOLO** (26-09) -- el kernel le
//! pregunta a cada funcion PCI sus bits de error (Status, Device Status de
//! PCIe y AER) y aqui se dicen. Solo lectura: nadie borra lo que confeso.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea y en el informe
//!
//! Lo que se lee donde: `Ultra_kernel_x86-64/kernel/src/ring0/dev/metiche.rs`.

use bmo_userland as bmo;

use super::tabla::{campo, section};
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Los nombres de los bits, para que un numero diga algo.
fn bits(s: &mut Output, v: u64, nombres: &[(u32, &[u8])]) {
    for &(b, n) in nombres {
        if v >> b & 1 != 0 {
            s.byte(b' ');
            s.text(n);
        }
    }
}

const STATUS: &[(u32, &[u8])] = &[
    (8, b"paridad-de-datos"),
    (11, b"aborto-dado"),
    (12, b"aborto-recibido"),
    (13, b"maestro-abortado"),
    (14, b"error-del-sistema"),
    (15, b"paridad"),
];
const DEVSTA: &[(u32, &[u8])] = &[(0, b"CORREGIBLE"), (1, b"NO-FATAL"), (2, b"FATAL"), (3, b"peticion-no-soportada")];
/// AER, errores CORREGIBLES (PCIe 7.8.4.5).
const AER_COR: &[(u32, &[u8])] = &[
    (0, b"error-del-receptor"),
    (6, b"TLP-malo"),
    (7, b"DLLP-malo"),
    (8, b"vuelta-de-reintentos"),
    (12, b"plazo-de-reintento"),
    (13, b"aviso-no-fatal"),
    (14, b"interno-corregido"),
    (15, b"cabecera-desbordada"),
];
/// AER, errores NO corregibles (PCIe 7.8.4.2).
const AER_UNC: &[(u32, &[u8])] = &[
    (4, b"protocolo-del-enlace"),
    (5, b"caida-sorpresa"),
    (12, b"TLP-envenenado"),
    (13, b"control-de-flujo"),
    (14, b"PLAZO-DE-RESPUESTA"),
    (15, b"aborto-del-que-responde"),
    (16, b"respuesta-inesperada"),
    (17, b"desborde-del-receptor"),
    (18, b"TLP-deforme"),
    (19, b"ECRC"),
    (20, b"peticion-no-soportada"),
    (21, b"violacion-ACS"),
];

/// **La seccion `metiche`**: pregunta OTRA VEZ (asi se ve lo nuevo de esta
/// sesion contra lo que habia al arrancar) y dice quien confeso.
pub(crate) fn report_metiche(s: &mut Output) {
    section(s, b"metiche -- lo que el hardware apunto solo, preguntado a todos");
    let r = bmo::info(bmo::INFO_METICHE);
    let al_arrancar = bmo::info(bmo::INFO_METICHE | 1 << 8);
    let (funciones, aer, ahora) = (r & 0xFFFF, r >> 16 & 0xFFFF, r >> 32 & 0xFFFF);
    campo(s, b"bus");
    s.with_ink(if ahora == 0 { INK_GOOD } else { INK_ERR });
    s.dec(funciones);
    s.text(b" funciones preguntadas, ");
    s.dec(aer);
    s.text(b" con AER; confiesan errores ");
    s.dec(ahora);
    s.text(b" ahora, ");
    s.dec(al_arrancar);
    s.text(b" al arrancar");
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"metiche funciones", funciones, b"");
    super::datos::anotar(b"metiche con errores", ahora, b"");
    super::datos::anotar(b"metiche con errores al arrancar", al_arrancar, b"");
    let nuevos = bmo::info(bmo::INFO_METICHE | 34 << 8);
    super::datos::anotar(b"metiche nuevos en la sesion", nuevos.count_ones() as u64, b"");
    for k in 0..ahora.min(16) {
        let q = bmo::info(bmo::INFO_METICHE | (2 + 2 * k) << 8);
        let a = bmo::info(bmo::INFO_METICHE | (3 + 2 * k) << 8);
        campo(s, b"confiesa");
        s.with_ink(INK_ECHO);
        let bdf = q & 0xFFFF;
        s.hex(bdf >> 8, 2);
        s.byte(b':');
        s.hex(bdf >> 3 & 0x1F, 2);
        s.byte(b'.');
        s.dec(bdf & 7);
        s.text(b" vendor ");
        s.hex(q >> 48, 4);
        s.text(b":");
        bits(s, q >> 16 & 0xFFFF, STATUS);
        bits(s, q >> 32 & 0xFFFF, DEVSTA);
        if a != 0 {
            s.text(b"; AER");
            if a & 0xFFFF_FFFF != 0 {
                s.text(b" NO corregible:");
                bits(s, a & 0xFFFF_FFFF, AER_UNC);
            }
            if a >> 32 != 0 {
                s.text(b" corregible:");
                bits(s, a >> 32, AER_COR);
            }
        }
        s.text(if nuevos >> k & 1 != 0 { b"  <- NUEVO en esta sesion" as &[u8] } else { b"  (ya estaba al arrancar)" });
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
}

/// `metiche`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "metiche: preguntando a todo el hardware", INK_DIM);
    report_metiche(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}
