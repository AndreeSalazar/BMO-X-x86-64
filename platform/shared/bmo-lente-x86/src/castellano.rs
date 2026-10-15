//! **EN CASTELLANO** -- lo que hace una instruccion, dicho para una persona
//! (15-10).
//!
//! [carril]  VERDE     escribe en un bufer fijo; sin memoria dinamica
//! [consumo] NADA      solo cuando alguien lo pide (`dentro`, una autopsia)
//!
//! > El propietario: *"que mi kernel entienda binarios puros del 0 y 1 [...]
//! > sepa en castellano que significan y lo traduzca: mi lente automatico de
//! > ver binarios a [castellano]"*.
//!
//! Tres vistas de la MISMA instruccion, cada una sobre la anterior:
//!
//! ```text
//!    los bytes      48 63 00
//!    Intel          movsxd rax, dword [rax]          (texto.rs)
//!    castellano     lee 4 bytes con signo de la memoria en [rax] y los
//!                   deja en rax                       (esto)
//! ```
//!
//! Se construye SOBRE el texto Intel (el nombre y sus operandos ya dichos),
//! asi que lo que la lente sabe leer lo sabe decir: una instruccion sin frase
//! propia sale con su nombre y "(sin frase todavia)", nunca inventada.

use core::fmt::Write;

use crate::Instr;

/// La frase, en un bufer fijo (cabe la mas larga).
pub struct Frase {
    buf: [u8; 224],
    n: usize,
}

impl Frase {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.n]).unwrap_or("?")
    }
}

impl Write for Frase {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let cabe = b.len().min(self.buf.len() - self.n);
        self.buf[self.n..self.n + cabe].copy_from_slice(&b[..cabe]);
        self.n += cabe;
        Ok(())
    }
}

/// Un operando del texto Intel, dicho: la memoria con su medida, un numero,
/// o el registro por su nombre.
fn operando(f: &mut Frase, op: &str) {
    let op = op.trim();
    if op.is_empty() {
        return;
    }
    if let Some(i) = op.find('[') {
        let medida = op[..i].trim();
        let bytes = match medida {
            "byte" => "1 byte",
            "word" => "2 bytes",
            "dword" => "4 bytes",
            "qword" => "8 bytes",
            _ => "",
        };
        let dir = &op[i..];
        let _ = if bytes.is_empty() { write!(f, "la memoria en {dir}") } else { write!(f, "{bytes} de la memoria en {dir}") };
    } else if op.starts_with("0x") || op.starts_with("-0x") || op.bytes().all(|b| b.is_ascii_digit()) {
        let _ = write!(f, "el numero {op}");
    } else {
        let _ = f.write_str(op);
    }
}

/// Adonde va un salto o una llamada: una direccion se dice tal cual, y un
/// registro o la memoria como operando ("la direccion que hay en ...").
fn destino(f: &mut Frase, op: &str) {
    let op = op.trim();
    if op.starts_with("0x") {
        let _ = f.write_str(op);
    } else {
        let _ = f.write_str("la direccion que hay en ");
        operando(f, op);
    }
}

/// La condicion de un `j`, `cmov` o `set`, segun la ultima comparacion.
fn condicion(cc: &str) -> Option<&'static str> {
    Some(match cc {
        "e" | "z" => "salio igual (o cero)",
        "ne" | "nz" => "salio distinto (o no cero)",
        "l" => "salio menor (con signo)",
        "le" => "salio menor o igual (con signo)",
        "g" => "salio mayor (con signo)",
        "ge" => "salio mayor o igual (con signo)",
        "b" => "salio menor (sin signo)",
        "be" => "salio menor o igual (sin signo)",
        "a" => "salio mayor (sin signo)",
        "ae" => "salio mayor o igual (sin signo)",
        "s" => "salio negativo",
        "ns" => "no salio negativo",
        "o" => "desbordo",
        "no" => "no desbordo",
        "p" => "dejo paridad par",
        "np" => "dejo paridad impar",
        _ => return None,
    })
}

/// **La instruccion `i` (en `en`), en castellano.**
pub fn explica(i: &Instr, en: u64) -> Frase {
    let intel = i.texto(en);
    let mut f = Frase { buf: [0; 224], n: 0 };
    let s = intel.as_str();
    // `rep ` delante: se dice al final.
    let (rep, s) = match s.strip_prefix("rep ") {
        Some(r) => (true, r),
        None => (false, s),
    };
    let (nombre, resto) = match s.find(' ') {
        Some(k) => (&s[..k], s[k + 1..].trim()),
        None => (s, ""),
    };
    let mut ops = resto.split(", ");
    let a = ops.next().unwrap_or("");
    let b = ops.next().unwrap_or("");
    let c = ops.next().unwrap_or("");
    let dos = |f: &mut Frase, antes: &str, x: &str, medio: &str, y: &str, despues: &str| {
        let _ = f.write_str(antes);
        operando(f, x);
        let _ = f.write_str(medio);
        operando(f, y);
        let _ = f.write_str(despues);
    };
    match nombre {
        // Con `rep` delante son un bucle entero: se dice el bucle, no el paso.
        "movsb" | "movsq" | "movsd" | "movsw" if resto.is_empty() || resto.contains("[rdi]") => {
            let cuanto = match nombre {
                "movsb" => "1 byte",
                "movsw" => "2 bytes",
                "movsd" => "4 bytes",
                _ => "8 bytes",
            };
            let _ = if rep {
                write!(f, "copia rcx veces {cuanto} de la memoria en [rsi] a la de [rdi] (memcpy)")
            } else {
                write!(f, "copia {cuanto} de [rsi] a [rdi] y avanza los dos")
            };
            return f;
        }
        "mov" | "movaps" | "movups" | "movapd" | "movupd" | "movdqa" | "movdqu" | "movss" | "movsd" | "movq" | "movd" => dos(&mut f, "copia ", b, " en ", a, ""),
        "movzx" => dos(&mut f, "lee ", b, " sin signo (rellena con ceros) y lo deja en ", a, ""),
        "movsx" | "movsxd" => dos(&mut f, "lee ", b, " con signo y lo deja en ", a, ""),
        "lea" => {
            let dir = b.find('[').map_or(b, |k| &b[k..]);
            let _ = write!(f, "calcula la direccion {dir} (sin leer la memoria) y la deja en {a}");
        }
        "add" => dos(&mut f, "suma ", b, " a ", a, ""),
        "sub" => dos(&mut f, "resta ", b, " de ", a, ""),
        "imul" if !c.is_empty() => {
            let _ = write!(f, "multiplica ");
            operando(&mut f, b);
            let _ = write!(f, " por ");
            operando(&mut f, c);
            let _ = write!(f, " (con signo) y lo deja en {a}");
        }
        "imul" if !b.is_empty() => dos(&mut f, "multiplica ", a, " por ", b, " (con signo)"),
        "imul" | "mul" => {
            let _ = write!(f, "multiplica rax por ");
            operando(&mut f, a);
            let _ = f.write_str(if nombre == "mul" { " (sin signo); el resultado en rdx:rax" } else { " (con signo); el resultado en rdx:rax" });
        }
        "div" | "idiv" => {
            let _ = f.write_str("divide rdx:rax entre ");
            operando(&mut f, a);
            let _ = f.write_str(if nombre == "div" { " (sin signo): cociente en rax, resto en rdx" } else { " (con signo): cociente en rax, resto en rdx" });
        }
        "inc" => dos(&mut f, "le suma 1 a ", a, "", "", ""),
        "dec" => dos(&mut f, "le resta 1 a ", a, "", "", ""),
        "neg" => dos(&mut f, "le cambia el signo a ", a, "", "", ""),
        "not" => dos(&mut f, "invierte cada bit de ", a, "", "", ""),
        "xor" | "pxor" | "xorps" | "xorpd" if a == b => {
            let _ = write!(f, "pone {a} a cero");
        }
        "and" | "andps" | "andpd" | "pand" => dos(&mut f, "deja en ", a, " solo los bits que tambien estan en ", b, ""),
        "or" | "orps" | "orpd" | "por" => dos(&mut f, "junta en ", a, " los bits de ", b, ""),
        "xor" | "xorps" | "xorpd" | "pxor" => dos(&mut f, "invierte en ", a, " los bits que estan en ", b, ""),
        "cmp" => dos(&mut f, "compara ", a, " con ", b, " (para el salto o la eleccion que venga)"),
        "test" if a == b => {
            let _ = write!(f, "mira si {a} es cero o negativo (para lo que venga)");
        }
        "test" => dos(&mut f, "mira que bits tienen en comun ", a, " y ", b, " (para lo que venga)"),
        "shl" | "sal" => dos(&mut f, "corre a la izquierda los bits de ", a, " tantos como ", b, " (multiplica por 2 cada uno)"),
        "shr" => dos(&mut f, "corre a la derecha los bits de ", a, " tantos como ", b, " (divide por 2 cada uno, sin signo)"),
        "sar" => dos(&mut f, "corre a la derecha los bits de ", a, " tantos como ", b, " (divide por 2 cada uno, con signo)"),
        "rol" | "ror" => dos(&mut f, "gira los bits de ", a, " tantos como ", b, ""),
        "push" => dos(&mut f, "guarda ", a, " en la pila", "", ""),
        "pop" => {
            let _ = write!(f, "saca lo de arriba de la pila y lo deja en {a}");
        }
        "call" => {
            let _ = f.write_str("llama a la funcion en ");
            destino(&mut f, a);
            let _ = f.write_str(" (y vuelve aqui al acabar)");
        }
        "ret" => {
            let _ = f.write_str("vuelve a quien llamo a esta funcion");
        }
        "jmp" => {
            let _ = f.write_str("salta a ");
            destino(&mut f, a);
        }
        "leave" => {
            let _ = f.write_str("deshace el marco de la funcion (rsp vuelve a rbp y rbp a su valor de antes)");
        }
        "cqo" | "cdq" | "cdqe" | "cwde" | "cbw" => {
            let _ = f.write_str("extiende el signo de rax (para dividir con signo)");
        }
        "nop" => {
            let _ = f.write_str("no hace nada (relleno para alinear)");
        }
        "syscall" => {
            let _ = f.write_str("LLAMA AL KERNEL: le pide algo a BMO-X (la puerta)");
        }
        "ud2" => {
            let _ = f.write_str("para el programa A PROPOSITO: es una trampa (una regla que no se cumplio)");
        }
        "int3" => {
            let _ = f.write_str("para el programa (punto de ruptura)");
        }
        "hlt" => {
            let _ = f.write_str("detiene el procesador hasta la proxima interrupcion");
        }
        "rdtsc" => {
            let _ = f.write_str("lee el reloj de ciclos del procesador en edx:eax");
        }
        "cpuid" => {
            let _ = f.write_str("le pregunta al procesador quien es y que sabe hacer");
        }
        "xchg" => dos(&mut f, "intercambia ", a, " y ", b, ""),
        "adc" => dos(&mut f, "suma ", b, " y el acarreo a ", a, ""),
        "sbb" => dos(&mut f, "resta ", b, " y el acarreo de ", a, ""),
        "rcl" | "rcr" => dos(&mut f, "gira los bits de ", a, " con el acarreo, tantos como ", b, ""),
        "cld" => {
            let _ = f.write_str("las copias de memoria que vengan van hacia adelante");
        }
        "pause" => {
            let _ = f.write_str("espera un instante (dentro de un bucle que espera a algo)");
        }
        "stosb" | "stosq" | "stosd" | "stosw" => {
            let _ = if rep { f.write_str("rellena rcx veces la memoria en [rdi] con rax (memset)") } else { f.write_str("escribe rax en [rdi] y avanza") };
            return f;
        }
        n if n.starts_with('f') && n.len() > 2 => {
            let _ = write!(f, "cuenta en coma flotante con el coprocesador x87 ({n} {resto})");
        }
        "bsf" | "tzcnt" => dos(&mut f, "busca el primer bit a 1 (desde abajo) de ", b, " y deja su posicion en ", a, ""),
        "bsr" | "lzcnt" => dos(&mut f, "busca el ultimo bit a 1 (desde arriba) de ", b, " y deja su posicion en ", a, ""),
        "popcnt" => dos(&mut f, "cuenta los bits a 1 de ", b, " y deja la cuenta en ", a, ""),
        "bswap" => dos(&mut f, "da la vuelta al orden de los bytes de ", a, "", "", ""),
        "addss" | "addsd" | "addps" | "addpd" => dos(&mut f, "suma (coma flotante) ", b, " a ", a, ""),
        "subss" | "subsd" | "subps" | "subpd" => dos(&mut f, "resta (coma flotante) ", b, " de ", a, ""),
        "mulss" | "mulsd" | "mulps" | "mulpd" => dos(&mut f, "multiplica (coma flotante) ", a, " por ", b, ""),
        "divss" | "divsd" | "divps" | "divpd" => dos(&mut f, "divide (coma flotante) ", a, " entre ", b, ""),
        "sqrtss" | "sqrtsd" | "sqrtps" => dos(&mut f, "saca la raiz cuadrada de ", b, " y la deja en ", a, ""),
        "minss" | "minsd" | "minps" => dos(&mut f, "deja en ", a, " el menor entre el y ", b, ""),
        "maxss" | "maxsd" | "maxps" => dos(&mut f, "deja en ", a, " el mayor entre el y ", b, ""),
        "ucomiss" | "ucomisd" | "comiss" | "comisd" => dos(&mut f, "compara (coma flotante) ", a, " con ", b, " (para lo que venga)"),
        n if n.starts_with("cvt") => dos(&mut f, "convierte ", b, " a otro tipo de numero y lo deja en ", a, ""),
        n if n.starts_with("cmov") => match condicion(&n[4..]) {
            Some(cc) => {
                dos(&mut f, "copia ", b, " en ", a, "");
                let _ = write!(f, " solo si la comparacion anterior {cc}");
            }
            None => {
                let _ = write!(f, "{n}: (sin frase todavia) {resto}");
            }
        },
        n if n.starts_with("set") => match condicion(&n[3..]) {
            Some(cc) => {
                let _ = write!(f, "pone {a} a 1 si la comparacion anterior {cc}, si no a 0");
            }
            None => {
                let _ = write!(f, "{n}: (sin frase todavia) {resto}");
            }
        },
        n if n.starts_with('j') => match condicion(&n[1..]) {
            Some(cc) => {
                let _ = f.write_str("salta a ");
                destino(&mut f, a);
                let _ = write!(f, " si la comparacion anterior {cc}");
            }
            None => {
                let _ = write!(f, "{n}: (sin frase todavia) {resto}");
            }
        },
        _ => {
            let _ = write!(f, "{nombre}: (sin frase todavia) {resto}");
        }
    }
    if rep {
        let _ = f.write_str(" (rep: con la regla de la CPU que la repite o la cambia)");
    }
    f
}
