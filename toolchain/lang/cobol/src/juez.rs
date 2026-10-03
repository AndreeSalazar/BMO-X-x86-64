//! **EL JUEZ** -- COBOL a nivel BANCO (2026-10-03, CM3 de
//! `docs/plan/PLAN_COBOL_MAESTRO.md`).
//!
//! El propietario: *"que COBOL BMO tenga un JUEZ ultra estricto, al nivel
//! BANCO, BIS, Suiza"*. La inspiracion es el espiritu de los principios del
//! Banco de Pagos Internacionales para los datos de riesgo (BCBS 239, Basilea):
//! un numero de dinero tiene que ser EXACTO, COMPLETO y TRAZABLE. Esto NO es
//! una certificacion del BIS ni lo pretende: son reglas de la casa, escritas
//! con ese rasero, que el compilador comprueba en cada programa.
//!
//! ** Lo que juzga es una sola cosa: **que ningun centimo se pierda en
//! silencio**. Cada regla cierra una puerta por donde COBOL, el del estandar,
//! deja pasar un numero que no es:
//!
//! ```text
//!    J1  DESBORDE     aritmetica sin ON SIZE ERROR: lo que no cabe se guarda
//!                     truncado por arriba y el programa sigue
//!    J2  REDONDEO     decimales que se pierden sin ROUNDED: el ultimo
//!                     digito es una decision legal, no un accidente
//!    J3  TRUNCA       un MOVE a un campo mas chico (enteros o decimales)
//!    J4  SIGNO        un valor con signo a un campo sin S: el negativo se
//!                     vuelve positivo sin avisar
//!    J5  FICHERO      un fichero sin FILE STATUS: un fallo de disco no se ve
//!    J6  DIVIDE       `DIVIDE a BY b` sin GIVING no es COBOL estandar (es
//!                     `DIVIDE a INTO b`): se rechaza hasta que haya INTO
//!    J7  SIN VALOR    un numero de WORKING-STORAGE sin VALUE empieza con lo
//!                     que haya en la memoria
//!    J8  BINARIO      un COMP binario para dinero: el decimal es COMP-3 o
//!                     DISPLAY (hoy el parser ya rechaza COMP y la coma
//!                     flotante: J8 es la guardia del dia en que COMP exista)
//!    J9  SIN DECLARAR un nombre que no esta en la DATA DIVISION
//!    J10 COMPARA      comparar con un literal de MAS decimales que el campo:
//!                     esa igualdad no puede ser verdad nunca
//! ```
//!
//! [!] Lo que NO juzga todavia, dicho: el `READ` de texto dentro de un campo
//! COMP-3 (extension de BMO, CM7), y los saltos `GO TO` que salen de un rango
//! `THRU`. Van al plan, no se fingen.

use crate::ast::{CobolProgram, CobolStatement, Condicion, DataItem};
use crate::pic::{PicField, Usage};

/// Las reglas, cada una con su numero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Regla {
    Desborde,
    Redondeo,
    Trunca,
    Signo,
    Fichero,
    Divide,
    SinValor,
    Binario,
    SinDeclarar,
    Compara,
}

impl Regla {
    pub fn codigo(self) -> &'static str {
        match self {
            Regla::Desborde => "J1 DESBORDE",
            Regla::Redondeo => "J2 REDONDEO",
            Regla::Trunca => "J3 TRUNCA",
            Regla::Signo => "J4 SIGNO",
            Regla::Fichero => "J5 FICHERO",
            Regla::Divide => "J6 DIVIDE",
            Regla::SinValor => "J7 SIN VALOR",
            Regla::Binario => "J8 BINARIO",
            Regla::SinDeclarar => "J9 SIN DECLARAR",
            Regla::Compara => "J10 COMPARA",
        }
    }
}

/// **Una falta**: que regla, donde (el parrafo, o el dato), y que.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Falta {
    pub regla: Regla,
    pub donde: String,
    pub que: String,
}

impl core::fmt::Display for Falta {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}  [{}]  {}", self.regla.codigo(), self.donde, self.que)
    }
}

/// Un operando, ya entendido: un dato (con su PIC) o un literal numerico.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Forma {
    enteros: u32,
    escala: u32,
    signo: bool,
}

impl Forma {
    fn de_pic(p: &PicField) -> Forma {
        Forma { enteros: p.integer_digits, escala: p.scale, signo: p.signed }
    }
}

/// Un literal numerico (`1250.00`, `-1`, `+0.5`), o `None`.
fn literal(s: &str) -> Option<Forma> {
    let t = s.trim();
    let (signo, c) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if c.is_empty() || !c.bytes().all(|b| b.is_ascii_digit() || b == b'.') || c.matches('.').count() > 1 {
        return None;
    }
    let (e, d) = c.split_once('.').unwrap_or((c, ""));
    let e = e.trim_start_matches('0');
    let d = d.trim_end_matches('0');
    Some(Forma { enteros: e.len() as u32, escala: d.len() as u32, signo })
}

/// Las constantes figurativas: valen en cualquier campo.
fn figurativa(s: &str) -> bool {
    matches!(s.trim().to_ascii_uppercase().as_str(), "ZERO" | "ZEROS" | "ZEROES" | "SPACE" | "SPACES" | "HIGH-VALUE" | "HIGH-VALUES" | "LOW-VALUE" | "LOW-VALUES")
}

/// Un nombre sin su subindice: `E(I)` -> `E`.
fn sin_subindice(s: &str) -> &str {
    s.trim().split('(').next().unwrap_or("").trim()
}

struct Juez<'a> {
    p: &'a CobolProgram,
    faltas: Vec<Falta>,
}

impl<'a> Juez<'a> {
    fn dato(&self, nombre: &str) -> Option<&'a DataItem> {
        let n = sin_subindice(nombre);
        self.p.data_items.iter().find(|d| d.name.eq_ignore_ascii_case(n))
    }

    fn falta(&mut self, regla: Regla, donde: &str, que: String) {
        self.faltas.push(Falta { regla, donde: donde.to_string(), que });
    }

    /// La forma de un operando. Un nombre que no esta es J9; un texto entre
    /// comillas o una figurativa, `None` (no son numeros).
    fn forma(&mut self, op: &str, donde: &str) -> Option<Forma> {
        let t = op.trim();
        if t.starts_with('"') || t.starts_with('\'') || figurativa(t) {
            return None;
        }
        if let Some(f) = literal(t) {
            return Some(f);
        }
        match self.dato(t) {
            Some(d) => d.pic_field.as_ref().filter(|p| p.numeric).map(Forma::de_pic),
            None => {
                self.falta(Regla::SinDeclarar, donde, format!("`{t}` no esta en la DATA DIVISION"));
                None
            }
        }
    }

    /// El destino de una aritmetica: tiene que ser un dato numerico.
    fn destino(&mut self, op: &str, donde: &str) -> Option<Forma> {
        match self.dato(op) {
            Some(d) => d.pic_field.as_ref().filter(|p| p.numeric).map(Forma::de_pic),
            None => {
                self.falta(Regla::SinDeclarar, donde, format!("el destino `{}` no esta en la DATA DIVISION", op.trim()));
                None
            }
        }
    }

    fn aritmetica(&mut self, verbo: &str, fuentes: &[Forma], destino: Option<Forma>, dst: &str, a: &crate::ast::Aritmetica, divide: bool, resta: bool, donde: &str) {
        let Some(d) = destino else { return };
        if a.si_desborda.is_none() {
            self.falta(Regla::Desborde, donde, format!("{verbo} sobre `{}` sin ON SIZE ERROR: lo que no cabe se guardaria truncado", dst.trim()));
        }
        let redondea = a.redondeo != crate::ast::Redondeo::Truncar;
        // Los decimales que produce la operacion, contra los del destino.
        let escala = match verbo {
            "MULTIPLY" => fuentes.iter().map(|f| f.escala).sum::<u32>() + d.escala,
            _ => fuentes.iter().map(|f| f.escala).max().unwrap_or(0),
        };
        if !redondea && (divide || escala > d.escala) {
            let por = if divide { "una division".to_string() } else { format!("{escala} decimales en {} que tiene {}", dst.trim(), d.escala) };
            self.falta(Regla::Redondeo, donde, format!("{verbo} con {por}, sin ROUNDED: el ultimo digito se decide, no se tira"));
        }
        if !d.signo && (resta || fuentes.iter().any(|f| f.signo)) {
            self.falta(Regla::Signo, donde, format!("{verbo} puede dejar un negativo en `{}`, que no tiene S", dst.trim()));
        }
    }

    fn sentencias(&mut self, ss: &[CobolStatement], donde: &str) {
        for s in ss {
            self.sentencia(s, donde);
        }
    }

    fn sentencia(&mut self, s: &CobolStatement, donde: &str) {
        use CobolStatement as S;
        match s {
            S::Move(src, dst) => {
                let fuente = self.forma(src, donde);
                let destino = self.dato(dst).and_then(|d| d.pic_field.as_ref()).filter(|p| p.numeric && d_no_editado(self.dato(dst))).map(Forma::de_pic);
                if self.dato(dst).is_none() {
                    self.falta(Regla::SinDeclarar, donde, format!("el destino `{}` no esta en la DATA DIVISION", dst.trim()));
                }
                if let (Some(f), Some(d)) = (fuente, destino) {
                    if f.enteros > d.enteros || f.escala > d.escala {
                        self.falta(
                            Regla::Trunca,
                            donde,
                            format!("MOVE {} TO {}: {}.{} digitos en un {}.{}, se cortaria", src.trim(), dst.trim(), f.enteros, f.escala, d.enteros, d.escala),
                        );
                    }
                    if f.signo && !d.signo {
                        self.falta(Regla::Signo, donde, format!("MOVE {} TO {}: con signo a un campo sin S", src.trim(), dst.trim()));
                    }
                }
            }
            S::Add(src, dst, a) => {
                let f: Vec<Forma> = self.forma(src, donde).into_iter().collect();
                let d = self.destino(dst, donde);
                self.aritmetica("ADD", &f, d, dst, a, false, false, donde);
                self.ramas(a, donde);
            }
            S::Subtract(src, dst, a) => {
                let f: Vec<Forma> = self.forma(src, donde).into_iter().collect();
                let d = self.destino(dst, donde);
                self.aritmetica("SUBTRACT", &f, d, dst, a, false, true, donde);
                self.ramas(a, donde);
            }
            S::Multiply(src, dst, a) => {
                let f: Vec<Forma> = self.forma(src, donde).into_iter().collect();
                let d = self.destino(dst, donde);
                self.aritmetica("MULTIPLY", &f, d, dst, a, false, false, donde);
                self.ramas(a, donde);
            }
            S::Divide(src, dst, a) => {
                self.falta(Regla::Divide, donde, format!("DIVIDE {} BY {} sin GIVING no es COBOL estandar: se escribe DIVIDE {} INTO {}", src.trim(), dst.trim(), src.trim(), dst.trim()));
                let f: Vec<Forma> = self.forma(src, donde).into_iter().collect();
                let d = self.destino(dst, donde);
                self.aritmetica("DIVIDE", &f, d, dst, a, true, false, donde);
                self.ramas(a, donde);
            }
            S::Compute(dst, expr, a) => {
                let mut fuentes = Vec::new();
                for t in tokens(expr) {
                    if !matches!(t.as_str(), "+" | "-" | "*" | "/" | "(" | ")") {
                        if let Some(f) = self.forma(&t, donde) {
                            fuentes.push(f);
                        }
                    }
                }
                let d = self.destino(dst, donde);
                let divide = expr.contains('/');
                let resta = tokens(expr).iter().any(|t| t == "-");
                // Un producto suma las escalas de sus factores: se mide la
                // peor (la suma de todas), que es lo que puede producir.
                let escala_producto = if expr.contains('*') { fuentes.iter().map(|f| f.escala).sum() } else { 0 };
                let mut f = fuentes.clone();
                if escala_producto > 0 {
                    f.push(Forma { enteros: 0, escala: escala_producto, signo: false });
                }
                self.aritmetica("COMPUTE", &f, d, dst, a, divide, resta, donde);
                self.ramas(a, donde);
            }
            S::If(c, si, no) => {
                self.condicion(c, donde);
                self.sentencias(si, donde);
                self.sentencias(no, donde);
            }
            S::Evaluate(ramas) => {
                for (c, cuerpo) in ramas {
                    if let Some(c) = c {
                        self.condicion(c, donde);
                    }
                    self.sentencias(cuerpo, donde);
                }
            }
            S::PerformTimes(_, cuerpo) => self.sentencias(cuerpo, donde),
            S::PerformUntil(c, cuerpo) => {
                self.condicion(c, donde);
                self.sentencias(cuerpo, donde);
            }
            S::PerformVarying { controles, cuerpo } => {
                for c in controles {
                    self.condicion(&c.hasta_que, donde);
                }
                self.sentencias(cuerpo, donde);
            }
            S::Read(_, fin, sigue) => {
                self.sentencias(fin, donde);
                self.sentencias(sigue, donde);
            }
            _ => {}
        }
    }

    fn ramas(&mut self, a: &crate::ast::Aritmetica, donde: &str) {
        if let Some(r) = &a.si_desborda {
            self.sentencias(r, donde);
        }
        if let Some(r) = &a.si_cabe {
            self.sentencias(r, donde);
        }
    }

    /// J10: una igualdad con un literal de mas decimales que el campo.
    fn condicion(&mut self, c: &Condicion, donde: &str) {
        use crate::ast::CobolCondition as C;
        match c {
            Condicion::Simple(C::Equal(a, b) | C::NotEqual(a, b)) => {
                for (campo, lit) in [(a, b), (b, a)] {
                    let fa = self.dato(campo).and_then(|d| d.pic_field.as_ref()).filter(|p| p.numeric).map(Forma::de_pic);
                    if let (Some(fa), Some(lb)) = (fa, literal(lit)) {
                        if lb.escala > fa.escala {
                            self.falta(Regla::Compara, donde, format!("{campo} comparado con {lit}: el campo tiene {} decimales y el literal {}", fa.escala, lb.escala));
                        }
                    }
                }
            }
            Condicion::Simple(_) => {}
            Condicion::Y(a, b) | Condicion::O(a, b) => {
                self.condicion(a, donde);
                self.condicion(b, donde);
            }
        }
    }

    fn datos(&mut self) {
        let registros: Vec<String> = self.p.files.iter().map(|f| f.record.to_ascii_uppercase()).collect();
        for d in &self.p.data_items {
            let Some(pf) = &d.pic_field else { continue };
            if d.level == 88 || !pf.numeric {
                continue;
            }
            // Lo de un FD lo llena el READ: no lleva VALUE.
            let raiz = raiz_de(self.p, d);
            let de_fichero = registros.iter().any(|r| r.eq_ignore_ascii_case(&raiz));
            if pf.usage == Usage::Comp && pf.scale > 0 {
                self.falta(Regla::Binario, &d.name, "dinero en COMP binario: el decimal va en COMP-3 o DISPLAY".into());
            }
            if !de_fichero && d.value.is_none() && d.edicion.is_none() {
                self.falta(Regla::SinValor, &d.name, "numero de WORKING-STORAGE sin VALUE: empezaria con lo que haya en la memoria".into());
            }
        }
        for f in &self.p.files {
            if f.estado.is_none() {
                self.falta(Regla::Fichero, &f.name, "fichero sin FILE STATUS: un fallo del disco no se veria".into());
            }
        }
    }
}

/// Un campo editado (`PIC $$$,$$9.99`) es para MOSTRAR, no para calcular: el
/// MOVE a el no se juzga como truncado (la mascara decide).
fn d_no_editado(d: Option<&DataItem>) -> bool {
    d.is_some_and(|d| d.edicion.is_none())
}

/// El `01` del que cuelga un dato.
fn raiz_de(p: &CobolProgram, d: &DataItem) -> String {
    let mut actual = d;
    for _ in 0..16 {
        match actual.padre.as_ref().and_then(|n| p.data_items.iter().find(|x| x.name.eq_ignore_ascii_case(n))) {
            Some(x) => actual = x,
            None => break,
        }
    }
    actual.name.clone()
}

/// Los trozos de una expresion de COMPUTE, con el guion pegado a un nombre
/// como parte del nombre (`CAB-SALDO`), igual que el emisor.
fn tokens(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in expr.chars() {
        let es_nombre = !cur.is_empty() && !cur.chars().all(|c| c.is_ascii_digit() || c == '.');
        if ch == '-' && es_nombre {
            cur.push(ch);
        } else if "+-*/()".contains(ch) {
            if !cur.trim().is_empty() {
                out.push(cur.trim().to_string());
            }
            cur.clear();
            out.push(ch.to_string());
        } else if ch.is_whitespace() {
            if !cur.trim().is_empty() {
                out.push(cur.trim().to_string());
            }
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// **Juzga un programa.** Ninguna falta = pasa el juez.
pub fn juzgar(p: &CobolProgram) -> Vec<Falta> {
    let mut j = Juez { p, faltas: Vec::new() };
    j.datos();
    let principal = format!("{} (principal)", p.program_id);
    j.sentencias(&p.statements, &principal);
    for par in &p.parrafos {
        j.sentencias(&par.statements, &par.name);
    }
    j.faltas
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn programa(datos: &str, cuerpo: &str) -> CobolProgram {
        let src = format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{datos}\nPROCEDURE DIVISION.\n{cuerpo}\nSTOP RUN.\n");
        crate::parse(&src).expect("el programa de prueba se analiza")
    }

    fn reglas(datos: &str, cuerpo: &str) -> Vec<Regla> {
        juzgar(&programa(datos, cuerpo)).into_iter().map(|f| f.regla).collect()
    }

    #[test]
    fn lo_bien_escrito_pasa() {
        let r = reglas(
            "01 SALDO PIC S9(7)V99 COMP-3 VALUE 0.\n01 CUOTA PIC S9(5)V99 VALUE 19.99.",
            "ADD CUOTA TO SALDO ON SIZE ERROR\nDISPLAY \"no cabe\"\nEND-ADD.\nMOVE CUOTA TO SALDO.",
        );
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn j1_aritmetica_sin_on_size_error() {
        assert_eq!(reglas("01 A PIC S9(3) VALUE 0.", "ADD 1 TO A."), [Regla::Desborde]);
    }

    #[test]
    fn j2_decimales_que_se_pierden_sin_rounded() {
        let r = reglas("01 A PIC S9(5) VALUE 0.\n01 B PIC S9(3)V99 VALUE 1.50.", "ADD B TO A ON SIZE ERROR\nDISPLAY \"x\"\nEND-ADD.");
        assert_eq!(r, [Regla::Redondeo]);
        let r = reglas("01 A PIC S9(5)V99 VALUE 0.\n01 B PIC S9(3)V99 VALUE 1.50.", "COMPUTE A = B / 3 ON SIZE ERROR\nDISPLAY \"x\"\nEND-COMPUTE.");
        assert_eq!(r, [Regla::Redondeo], "una division siempre decide su ultimo digito");
        let r = reglas("01 A PIC S9(5)V99 VALUE 0.\n01 B PIC S9(3)V99 VALUE 1.50.", "COMPUTE A ROUNDED = B / 3 ON SIZE ERROR\nDISPLAY \"x\"\nEND-COMPUTE.");
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn j3_el_move_que_corta() {
        // El caso que el ESPEJO encontro en `cuentas.cob`, ahora antes de compilar.
        assert_eq!(reglas("01 CORTO PIC 9(3) VALUE 0.", "MOVE 12345 TO CORTO."), [Regla::Trunca]);
        assert_eq!(reglas("01 A PIC 9(5) VALUE 0.", "MOVE 1.50 TO A."), [Regla::Trunca]);
        assert!(reglas("01 A PIC 9(5)V99 VALUE 0.", "MOVE 1.50 TO A.\nMOVE ZERO TO A.").is_empty());
    }

    #[test]
    fn j4_el_signo_que_se_pierde() {
        assert_eq!(reglas("01 A PIC 9(3) VALUE 0.", "MOVE -1 TO A."), [Regla::Signo]);
        let r = reglas("01 A PIC 9(3) VALUE 5.", "SUBTRACT 1 FROM A ON SIZE ERROR\nDISPLAY \"x\"\nEND-SUBTRACT.");
        assert_eq!(r, [Regla::Signo]);
    }

    #[test]
    fn j6_divide_by_sin_giving() {
        let r = reglas("01 A PIC S9(5)V99 VALUE 10.", "DIVIDE 4 BY A ROUNDED ON SIZE ERROR\nDISPLAY \"x\"\nEND-DIVIDE.");
        assert_eq!(r, [Regla::Divide]);
    }

    #[test]
    fn j7_un_numero_sin_valor() {
        assert_eq!(reglas("01 A PIC S9(3).", "DISPLAY A."), [Regla::SinValor]);
    }

    /// J8 es la guardia del DIA en que COMP exista: hoy el parser ya lo
    /// rechaza con su motivo, antes de que llegue al juez.
    #[test]
    fn j8_hoy_lo_para_el_parser() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC S9(3)V99 COMP VALUE 0.\nPROCEDURE DIVISION.\nSTOP RUN.\n";
        assert!(crate::parse(src).is_err());
    }

    #[test]
    fn j9_un_nombre_que_no_esta() {
        let r = reglas("01 A PIC S9(3) VALUE 0.", "ADD NADA TO A ON SIZE ERROR\nDISPLAY \"x\"\nEND-ADD.");
        assert_eq!(r, [Regla::SinDeclarar]);
    }

    #[test]
    fn j10_una_igualdad_imposible() {
        assert_eq!(reglas("01 A PIC S9(3)V99 VALUE 0.", "IF A = 1.005\nDISPLAY \"x\"\nEND-IF."), [Regla::Compara]);
    }

    #[test]
    fn j5_un_fichero_sin_file_status() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO \"d/f.txt\".\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC 9(5).\nPROCEDURE DIVISION.\nSTOP RUN.\n";
        let r: Vec<Regla> = juzgar(&crate::parse(src).unwrap()).into_iter().map(|f| f.regla).collect();
        assert_eq!(r, [Regla::Fichero]);
    }

    /// ** LA LIBRERIA DE BANK CAT Y SU MOTOR PASAN EL JUEZ: el primer COBOL
    /// de la casa a nivel banco.
    #[test]
    fn el_motor_de_bank_cat_pasa_el_juez() {
        let mut libreria = |n: &str| match n {
            "CABDATOS" => Some(include_str!("../copy/CABDATOS.cpy").to_string()),
            "CABLIBRO" => Some(include_str!("../copy/CABLIBRO.cpy").to_string()),
            _ => None,
        };
        let src = crate::copia::expandir(include_str!("../examples/11-bankcat/libro.cob"), &mut libreria).unwrap();
        let faltas = juzgar(&crate::parse(&src).unwrap());
        assert!(faltas.is_empty(), "{:#?}", faltas);
    }
}
