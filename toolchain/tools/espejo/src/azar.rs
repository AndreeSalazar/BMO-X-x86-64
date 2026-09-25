//! **PROGRAMAS AL AZAR**, sin comportamiento indefinido, para cazar lo que
//! ningun caso escrito a mano pregunta. Es la idea de Csmith (2011), en chico:
//! un programa que nadie escribio, compilado por los dos lados, y la salida
//! comparada linea a linea.
//!
//! # Las reglas que lo hacen JUSTO
//!
//! Un programa con comportamiento indefinido puede dar cualquier cosa, y
//! entonces que BMO "falle" no dice nada. Asi que aqui no hay:
//!
//! ```text
//!    desbordamiento con signo   toda la aritmetica es SIN signo, y cada
//!                               operando lleva su cast: dos `unsigned short`
//!                               se promoverian a `int` y su producto si
//!                               desbordaria
//!    desplazar demasiado        `x << (y & 31)` (o `& 63` en 64 bits)
//!    dividir por cero           `x / (y | 1)`
//!    salirse de un array        `a[(i) & 7]` sobre arrays de 8
//!    recursion sin fin          una funcion solo llama a las de ANTES
//!    orden sin fijar            una funcion NO escribe globales: solo sus
//!                               locales. Si escribiera `g0` y la llamada
//!                               estuviera en `g0 + f()`, el orden de la
//!                               lectura y la escritura no lo fija el
//!                               estandar (GCC y Clang pueden coincidir y aun
//!                               asi no ser la unica respuesta correcta)
//!    variables sin iniciar      todas nacen con valor
//! ```
//!
//! Lo que SI hay y es "de la implementacion" (convertir un sin signo grande a
//! `int`, desplazar a la derecha un negativo): en x86-64 GCC y Clang hacen lo
//! mismo, y si algun dia no, el oraculo aparta el caso.
//!
//! La misma semilla da el mismo programa: un fallo se reproduce con su numero.

/// xorshift64*: chico, rapido y el mismo en cualquier maquina.
pub struct Azar(u64);

impl Azar {
    pub fn nuevo(semilla: u64) -> Self {
        Azar(semilla.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn hasta(&mut self, n: u64) -> u64 {
        self.u64() % n.max(1)
    }
    fn si(&mut self, de_cien: u64) -> bool {
        self.hasta(100) < de_cien
    }
}

/// Los tipos que usa el generador. Todos sin signo, salvo `I32`, que solo se
/// escribe por conversion y solo se compara o se imprime.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tipo {
    U8,
    U16,
    U32,
    U64,
    I32,
}

impl Tipo {
    fn c(self) -> &'static str {
        match self {
            Tipo::U8 => "unsigned char",
            Tipo::U16 => "unsigned short",
            Tipo::U32 => "unsigned int",
            Tipo::U64 => "unsigned long long",
            Tipo::I32 => "int",
        }
    }
    /// El tipo en el que se hace la cuenta.
    fn cuenta(self) -> Tipo {
        if self == Tipo::U64 { Tipo::U64 } else { Tipo::U32 }
    }
}

struct Var {
    nombre: String,
    tipo: Tipo,
}

struct Gen {
    r: Azar,
    globales: Vec<Var>,
    /// Funciones ya escritas: (nombre). Todas `unsigned int f(unsigned int, unsigned long long)`.
    funciones: Vec<String>,
    /// Variables de bucle vivas (nombre).
    bucles: Vec<String>,
    /// Parametros de la funcion que se esta escribiendo.
    params: Vec<Var>,
    /// Sus variables locales: lo UNICO que una funcion puede escribir.
    locales: Vec<Var>,
    prof: u32,
}

/// **Un programa de C** (que tambien es C++ valido) para la semilla `semilla`.
pub fn programa(semilla: u64) -> String {
    let mut g = Gen { r: Azar::nuevo(semilla), globales: Vec::new(), funciones: Vec::new(), bucles: Vec::new(), params: Vec::new(), locales: Vec::new(), prof: 0 };
    let mut s = String::new();
    s.push_str(&format!("/* espejo azar, semilla {semilla} */\n#include <stdio.h>\n\n"));
    // Las globales, con su valor de nacimiento.
    let n = 6 + g.r.hasta(6) as usize;
    for i in 0..n {
        let tipo = [Tipo::U8, Tipo::U16, Tipo::U32, Tipo::U32, Tipo::U64, Tipo::U64, Tipo::I32][g.r.hasta(7) as usize];
        let v = Var { nombre: format!("g{i}"), tipo };
        let valor = g.constante(tipo);
        s.push_str(&format!("static {} {} = {};\n", tipo.c(), v.nombre, valor));
        g.globales.push(v);
    }
    s.push_str("static unsigned int arr[8] = {");
    for i in 0..8 {
        s.push_str(&format!("{}{}u", if i > 0 { ", " } else { "" }, g.r.hasta(1 << 20)));
    }
    s.push_str("};\n");
    s.push_str("struct S { unsigned int a; unsigned short b; unsigned long long c; };\n");
    s.push_str(&format!("static struct S st = {{ {}u, {}, {}ull }};\n\n", g.r.hasta(1 << 30), g.r.hasta(65536), g.r.u64() >> 1));

    // Funciones: cada una solo llama a las de antes.
    let nf = 1 + g.r.hasta(3) as usize;
    for i in 0..nf {
        let nombre = format!("f{i}");
        g.params = vec![Var { nombre: "pa".into(), tipo: Tipo::U32 }, Var { nombre: "pb".into(), tipo: Tipo::U64 }];
        s.push_str(&format!("static unsigned int {nombre}(unsigned int pa, unsigned long long pb) {{\n"));
        s.push_str("    unsigned int l0 = pa;\n    unsigned long long l1 = pb;\n");
        g.locales = vec![Var { nombre: "l0".into(), tipo: Tipo::U32 }, Var { nombre: "l1".into(), tipo: Tipo::U64 }];
        let k = 2 + g.r.hasta(4);
        for _ in 0..k {
            g.sentencia(&mut s, 1);
        }
        let e = g.expr(Tipo::U32, 2);
        s.push_str(&format!("    return (unsigned int)({e} + (unsigned int)l0 + (unsigned int)l1);\n}}\n\n"));
        g.params.clear();
        g.locales.clear();
        g.funciones.push(nombre);
    }

    s.push_str("int main(void) {\n");
    let k = 8 + g.r.hasta(12);
    for _ in 0..k {
        g.sentencia(&mut s, 1);
    }
    // Lo que se compara: cada global, el array, el struct, y una suma.
    s.push_str("    unsigned long long suma = 0;\n");
    for v in &g.globales {
        let f = match v.tipo {
            Tipo::U64 => "%llu",
            Tipo::I32 => "%d",
            _ => "%u",
        };
        let arg = match v.tipo {
            Tipo::U8 | Tipo::U16 => format!("(unsigned int){}", v.nombre),
            _ => v.nombre.clone(),
        };
        s.push_str(&format!("    printf(\"{} {}\\n\", {});\n", v.nombre, f, arg));
        s.push_str(&format!("    suma = suma * 31ull + (unsigned long long){};\n", v.nombre));
    }
    s.push_str("    for (unsigned int i = 0; i < 8; i++) { printf(\"arr %u\\n\", arr[i]); suma = suma * 31ull + arr[i]; }\n");
    s.push_str("    printf(\"st %u %u %llu\\n\", st.a, (unsigned int)st.b, st.c);\n");
    s.push_str("    printf(\"suma %llu\\n\", suma);\n");
    s.push_str("    return (int)(suma & 0x7Full);\n}\n");
    s
}

impl Gen {
    fn constante(&mut self, t: Tipo) -> String {
        match t {
            Tipo::U8 => format!("{}", self.r.hasta(256)),
            Tipo::U16 => format!("{}", self.r.hasta(65536)),
            Tipo::U32 => format!("{}u", self.r.u64() as u32),
            Tipo::U64 => format!("{}ull", self.r.u64()),
            Tipo::I32 => format!("{}", (self.r.u64() as i32) / 2),
        }
    }

    /// Una variable que se puede LEER, del tipo que sea.
    fn leer(&mut self) -> (String, Tipo) {
        let mut opciones: Vec<(String, Tipo)> = self.globales.iter().map(|v| (v.nombre.clone(), v.tipo)).collect();
        for v in self.params.iter().chain(self.locales.iter()) {
            opciones.push((v.nombre.clone(), v.tipo));
        }
        for b in &self.bucles {
            opciones.push((b.clone(), Tipo::U32));
        }
        opciones.push(("st.a".into(), Tipo::U32));
        opciones.push(("st.b".into(), Tipo::U16));
        opciones.push(("st.c".into(), Tipo::U64));
        let i = self.r.hasta(opciones.len() as u64) as usize;
        opciones.swap_remove(i)
    }

    /// Una expresion cuyo valor es del tipo `t` (y se escribe con su cast).
    fn expr(&mut self, t: Tipo, prof: u32) -> String {
        let c = t.cuenta().c();
        if prof == 0 || self.r.si(25) {
            return if self.r.si(35) {
                format!("(({c}){})", self.constante(t.cuenta()))
            } else {
                let (v, _) = self.leer();
                format!("(({c}){v})")
            };
        }
        let a = self.expr(t, prof - 1);
        let b = self.expr(t, prof - 1);
        let bits = if t.cuenta() == Tipo::U64 { 63 } else { 31 };
        match self.r.hasta(16) {
            0 => format!("({a} + {b})"),
            1 => format!("({a} - {b})"),
            2 => format!("({a} * {b})"),
            3 => format!("({a} & {b})"),
            4 => format!("({a} | {b})"),
            5 => format!("({a} ^ {b})"),
            6 => format!("({a} << ({b} & {bits}))"),
            7 => format!("({a} >> ({b} & {bits}))"),
            8 => format!("({a} / ({b} | 1))"),
            9 => format!("({a} % ({b} | 1))"),
            10 => format!("(({c})({a} < {b}))"),
            11 => format!("(({c})({a} == {b}))"),
            12 => {
                let x = self.expr(t, prof - 1);
                format!("({a} > {b} ? {x} : {b})")
            }
            13 => format!("(({c})(~{a}))"),
            14 => format!("(({c})arr[({a}) & 7])"),
            _ => {
                if let Some(f) = self.funciones.last().cloned() {
                    let i = self.r.hasta(self.funciones.len() as u64) as usize;
                    let f = if self.r.si(50) { f } else { self.funciones[i].clone() };
                    let x = self.expr(Tipo::U64, prof - 1);
                    format!("(({c}){f}((unsigned int){a}, {x}))")
                } else {
                    // Con signo, solo para COMPARAR: sin aritmetica, sin UB.
                    format!("(({c})((int){a} < (int){b}))")
                }
            }
        }
    }

    fn sangria(s: &mut String, n: u32) {
        for _ in 0..n {
            s.push_str("    ");
        }
    }

    fn sentencia(&mut self, s: &mut String, nivel: u32) {
        Self::sangria(s, nivel);
        let hondo = self.prof >= 2;
        let en_funcion = !self.locales.is_empty();
        let mut que = self.r.hasta(if hondo { 4 } else { 8 });
        // Dentro de una funcion solo se escriben SUS locales: las escrituras a
        // globales, al array, al struct y por puntero se vuelven una local.
        if en_funcion && matches!(que, 2 | 3 | 7) {
            que = 0;
        }
        match que {
            0 | 1 if en_funcion => {
                let i = self.r.hasta(self.locales.len() as u64) as usize;
                let (nombre, tipo) = (self.locales[i].nombre.clone(), self.locales[i].tipo);
                let e = self.expr(tipo.cuenta(), 3);
                s.push_str(&format!("{nombre} = ({})({e});\n", tipo.c()));
            }
            0 | 1 => {
                let i = self.r.hasta(self.globales.len() as u64) as usize;
                let (nombre, tipo) = (self.globales[i].nombre.clone(), self.globales[i].tipo);
                let e = self.expr(tipo.cuenta(), 3);
                s.push_str(&format!("{nombre} = ({})({e});\n", tipo.c()));
            }
            2 => {
                let e = self.expr(Tipo::U32, 2);
                let d = self.expr(Tipo::U32, 2);
                s.push_str(&format!("arr[({e}) & 7] = {d};\n"));
            }
            3 => {
                let campo = ["a", "b", "c"][self.r.hasta(3) as usize];
                let t = match campo { "a" => Tipo::U32, "b" => Tipo::U16, _ => Tipo::U64 };
                let e = self.expr(t.cuenta(), 3);
                s.push_str(&format!("st.{campo} = ({})({e});\n", t.c()));
            }
            4 => {
                self.prof += 1;
                let c = self.expr(Tipo::U32, 2);
                s.push_str(&format!("if ({c} & 1u) {{\n"));
                self.sentencia(s, nivel + 1);
                Self::sangria(s, nivel);
                s.push_str("} else {\n");
                self.sentencia(s, nivel + 1);
                Self::sangria(s, nivel);
                s.push_str("}\n");
                self.prof -= 1;
            }
            5 => {
                self.prof += 1;
                let v = format!("i{}", self.bucles.len());
                let k = 1 + self.r.hasta(6);
                s.push_str(&format!("for (unsigned int {v} = 0; {v} < {k}u; {v}++) {{\n"));
                self.bucles.push(v);
                self.sentencia(s, nivel + 1);
                self.sentencia(s, nivel + 1);
                self.bucles.pop();
                Self::sangria(s, nivel);
                s.push_str("}\n");
                self.prof -= 1;
            }
            6 => {
                self.prof += 1;
                let c = self.expr(Tipo::U32, 2);
                s.push_str(&format!("switch (({c}) & 3u) {{\n"));
                for caso in 0..3 {
                    Self::sangria(s, nivel);
                    s.push_str(&format!("case {caso}:\n"));
                    self.sentencia(s, nivel + 1);
                    // El caso 1 CAE al 2 a proposito: el fallthrough tambien se prueba.
                    if caso != 1 {
                        Self::sangria(s, nivel + 1);
                        s.push_str("break;\n");
                    }
                }
                Self::sangria(s, nivel);
                s.push_str("default:\n");
                self.sentencia(s, nivel + 1);
                Self::sangria(s, nivel);
                s.push_str("}\n");
                self.prof -= 1;
            }
            _ => {
                // Por puntero: lo que se escribe por `*p` tiene que verse en la global.
                let candidatos: Vec<usize> =
                    (0..self.globales.len()).filter(|&i| self.globales[i].tipo == Tipo::U32).collect();
                if let Some(&i) = candidatos.get(self.r.hasta(candidatos.len().max(1) as u64) as usize) {
                    let nombre = self.globales[i].nombre.clone();
                    let e = self.expr(Tipo::U32, 2);
                    s.push_str(&format!("{{ unsigned int *p = &{nombre}; *p = *p + {e}; }}\n"));
                } else {
                    let e = self.expr(Tipo::U32, 2);
                    s.push_str(&format!("st.a = st.a ^ {e};\n"));
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_misma_semilla_da_el_mismo_programa() {
        assert_eq!(programa(42), programa(42));
        assert_ne!(programa(42), programa(43));
    }
}
