//! **EL JUEZ**: lo que imprimio BMO contra lo que imprimio GnuCOBOL, linea a
//! linea. Y no todas las diferencias son iguales:
//!
//! ```text
//!    IGUAL          byte a byte
//!    SOLO DISPLAY   difieren solo en COMO se muestra un numero sin mascara:
//!                   GnuCOBOL saca la forma del estandar (`00059.97`, con
//!                   ceros y su signo) y BMO la corta (`59.97`). Es la tarea
//!                   1.5 de PLAN_BANCA, y se cuenta aparte para que no tape
//!                   lo que de verdad esta mal
//!    DISTINTO       otra cosa: un VALOR distinto, una linea de mas o de menos
//! ```

/// Lo que paso al comparar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Comparacion {
    Igual,
    /// La primera linea que difiere, con las dos versiones.
    SoloDisplay(usize, String, String),
    Distinto(usize, String, String),
}

/// Un numero "crudo" del DISPLAY, en su forma minima: sin el `+`, sin ceros a
/// la izquierda y sin espacios. `None` si la linea no es un numero.
fn numero_minimo(l: &str) -> Option<String> {
    let t = l.trim();
    let (signo, cuerpo) = match t.as_bytes().first() {
        Some(b'+') => ("", &t[1..]),
        Some(b'-') => ("-", &t[1..]),
        _ => ("", t),
    };
    // El signo tambien puede ir DETRAS (`59.97-`).
    let (signo, cuerpo) = match cuerpo.strip_suffix('-') {
        Some(c) => ("-", c),
        None => (signo, cuerpo.strip_suffix('+').unwrap_or(cuerpo)),
    };
    if cuerpo.is_empty() || !cuerpo.bytes().all(|c| c.is_ascii_digit() || c == b'.') || cuerpo.matches('.').count() > 1 {
        return None;
    }
    let (ent, dec) = cuerpo.split_once('.').unwrap_or((cuerpo, ""));
    let ent = ent.trim_start_matches('0');
    let ent = if ent.is_empty() { "0" } else { ent };
    let cero = ent == "0" && dec.bytes().all(|c| c == b'0');
    let signo = if cero { "" } else { signo };
    Some(if dec.is_empty() { format!("{signo}{ent}") } else { format!("{signo}{ent}.{dec}") })
}

/// **Compara.** Las lineas se miran sin el `\r` final.
pub fn comparar(gnu: &str, bmo: &str) -> Comparacion {
    if gnu == bmo {
        return Comparacion::Igual;
    }
    let g: Vec<&str> = gnu.lines().map(|l| l.trim_end_matches('\r')).collect();
    let b: Vec<&str> = bmo.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut primera_display = None;
    for k in 0..g.len().max(b.len()) {
        let (x, y) = (g.get(k).copied().unwrap_or("<no hay linea>"), b.get(k).copied().unwrap_or("<no hay linea>"));
        if x == y {
            continue;
        }
        match (numero_minimo(x), numero_minimo(y)) {
            (Some(a), Some(c)) if a == c => {
                primera_display.get_or_insert((k + 1, x.to_string(), y.to_string()));
            }
            _ => return Comparacion::Distinto(k + 1, x.to_string(), y.to_string()),
        }
    }
    match primera_display {
        Some((k, x, y)) => Comparacion::SoloDisplay(k, x, y),
        None => Comparacion::Igual,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_numeros_crudos_se_reducen_a_su_forma_minima() {
        assert_eq!(numero_minimo("00059.97").as_deref(), Some("59.97"));
        assert_eq!(numero_minimo("+00059.97").as_deref(), Some("59.97"));
        assert_eq!(numero_minimo("-0001.50").as_deref(), Some("-1.50"));
        assert_eq!(numero_minimo("0001.50-").as_deref(), Some("-1.50"));
        assert_eq!(numero_minimo("000").as_deref(), Some("0"));
        assert_eq!(numero_minimo("-0.00").as_deref(), Some("0.00"));
        assert_eq!(numero_minimo("$1,500.00"), None, "lo editado no se toca");
        assert_eq!(numero_minimo("hola"), None);
        assert_eq!(numero_minimo("1.2.3"), None);
    }

    #[test]
    fn el_juez_separa_display_de_valor() {
        assert_eq!(comparar("a\n1\n", "a\n1\n"), Comparacion::Igual);
        assert_eq!(comparar("saldo:\n00059.97\n", "saldo:\n59.97\n"), Comparacion::SoloDisplay(2, "00059.97".into(), "59.97".into()));
        assert_eq!(comparar("00059.97\n", "59.98\n"), Comparacion::Distinto(1, "00059.97".into(), "59.98".into()));
        assert_eq!(comparar("a\nb\n", "a\n"), Comparacion::Distinto(2, "b".into(), "<no hay linea>".into()));
        // Un valor mal despues de un display distinto: gana el valor.
        assert_eq!(comparar("007\n8\n", "7\n9\n"), Comparacion::Distinto(2, "8".into(), "9".into()));
    }
}
