//! **EL TANQUE** -- el instrumento de BANK CAT con el escritorio de mision (HM6
//! de `docs/plan/PLAN_EL_HUD.md`, 07-10): *"el combustible: el saldo es lo
//! que queda en el tanque"*.
//!
//! [consumo] NADA      se pinta con la ventana (L6h)
//!
//! ```text
//!    COMBUSTIBLE                                   F5  COBOL
//!    EN EL TANQUE
//!    1.240,03 CAB
//!    [##################.......]    76 % del tope
//!    REPOSTADO  + 50,00         CONSUMIDO  - 59,97
//! ```
//!
//! ** El TOPE del tanque es el saldo mas alto de esta sesion (el de antes del
//! primer asiento cuenta): un banco no tiene capacidad, y fingir una seria
//! inventar un numero. Asi la barra dice lo unico verdadero -- cuanto queda de
//! lo que hubo. Y como todo en BANK CAT, ni un centimo se calcula aqui: cada
//! cifra es una que el motor COBOL contesto.

use crate::canvas::Canvas;
use crate::mision::{barra, lectura, marco, miles, CUIDADO, GO, NOGO, TENUE, TINTA};
use crate::pintar::Vista;
use crate::piezas::{texto, Estilo};
use bmo_bankcat::{formato, Centimos};

/// Lo que mide el instrumento.
pub const ALTO: i32 = 196;

/// El saldo mas alto de la sesion: el de ahora, el de cada asiento, y el de
/// antes del primero.
fn tope(v: &Vista) -> Centimos {
    let mut t = v.saldo.unwrap_or(0);
    for a in v.libro {
        t = t.max(a.saldo).max(a.saldo - a.cambio);
    }
    t
}

/// **Pinta el tanque** en `(x, y)`, `w` de ancho.
pub fn pintar(cv: &mut Canvas, v: &Vista, x: i32, y: i32, w: i32) {
    marco(cv, x, y, w, ALTO, b"COMBUSTIBLE", b"F5  COBOL");
    let mut d = [0u8; 32];
    let Some(saldo) = v.saldo else {
        lectura(cv, x + 16, y + 34, b"EN EL TANQUE", b"--", b"el motor aun no contesto", TENUE);
        return;
    };
    let n = formato(saldo, &mut d).unwrap_or(0);
    let t = tope(v);
    let pct = if t > 0 { (saldo.max(0) as u64 * 100 / t as u64) as u32 } else { 0 };
    let tinta = if saldo <= 0 {
        NOGO
    } else if pct >= 50 {
        GO
    } else if pct >= 20 {
        CUIDADO
    } else {
        NOGO
    };
    lectura(cv, x + 16, y + 34, b"EN EL TANQUE", &d[..n], b"CAB", tinta);
    barra(cv, x + 16, y + 94, w - 32, 12, saldo.max(0) as u64, t.max(0) as u64, tinta);
    let chica = Estilo::normal(11);
    if saldo <= 0 {
        texto(cv, x + 16, y + 112, 16, b"TANQUE VACIO: el gato no fia", NOGO, chica);
    } else {
        let mut b = [0u8; 24];
        let k = miles(pct as u64, &mut b);
        let ancho = texto(cv, x + 16, y + 112, 16, &b[..k], TINTA, chica);
        texto(cv, x + 16 + ancho, y + 112, 16, b" % del tope de la sesion", TENUE, chica);
    }
    let entro: Centimos = v.libro.iter().filter(|a| a.cambio > 0).map(|a| a.cambio).sum();
    let salio: Centimos = v.libro.iter().filter(|a| a.cambio < 0).map(|a| -a.cambio).sum();
    let rotulo = Estilo::normal(10).espaciado(140).mayusculas();
    let cifra = Estilo::media(14);
    for (k, (que, signo, val, c)) in [(&b"REPOSTADO"[..], b'+', entro, GO), (b"CONSUMIDO", b'-', salio, if salio > 0 { CUIDADO } else { TINTA })].into_iter().enumerate() {
        let cx = x + 16 + k as i32 * ((w - 32) / 2);
        texto(cv, cx, y + 150, 14, que, TENUE, rotulo);
        d[0] = signo;
        d[1] = b' ';
        let mut e = [0u8; 32];
        let m = formato(val, &mut e).unwrap_or(0).min(30);
        d[2..2 + m].copy_from_slice(&e[..m]);
        texto(cv, cx, y + 166, 18, &d[..2 + m], c, cifra);
    }
}
