//! **El grupo F7**: `test` con inmediato, `not`, `neg`, `mul`, `div` e
//! `idiv`, los seis que comparten opcode y se distinguen por `/ext`.
//!
//! ** Salio de `mod.rs` el 2026-09-16 por L6a: el despachador paso de las
//! 1.000 lineas de codigo al entrar `mul` (la quinta familia del fallo del
//! signo de INTI) y el DIRECTOR de mentira. Es un brazo del `match` movido
//! como texto, con las mismas variables: `rex_x`, `rex_b`, `wide` y `ancho`
//! llegan como llegaban.

use super::{Machine, RAX, RDX};

impl Machine {
    pub(super) fn grupo_f7(&mut self, rex_x: usize, rex_b: usize, wide: bool, ancho: usize) {
        let (ext, src) = self.modrm(0, rex_x, rex_b);
        let v = self.load(src, wide);
        match ext & 7 {
            // `test r/m, imm32` (LB4, 08-10): la salida del MXCSR del x86-64
            // de una gpu fn (`nativo` de PROTON-X) mira si el control de
            // quien llamo es otro (`test ecx, MXCSR_CONTROL`). El inmediato
            // va DETRAS del ModRM, y como el `and`, solo deja banderas.
            0 => {
                let imm = self.fetch_u32() as i32 as i64 as u64;
                self.flags_logic(v & imm);
            }
            // `~x`. Faltaba, y el hueco era invisible: el codegen de C
            // lo emitia BIEN desde siempre --un `.bef` con `~0` se
            // escribe sin quejarse-- pero **ninguna matriz lo podia
            // ejecutar**, asi que ni C ni COBOL tenian una fila con
            // `~`. Lo destapo C++ al escribir la suya desde cero.
            //
            // * A diferencia de `neg`, `not` **no toca las banderas**
            // en x86-64. Llamar a `flags_logic` aqui seria un no-op
            // silencioso el 99% de las veces y una mentira el 1%: un
            // `~x` seguido de un salto condicional decidiria por el
            // resultado del `not` en vez de por la comparacion de
            // antes, que es lo que el silicio conserva.
            2 => {
                let r = !v;
                self.store(src, r, ancho);
            }
            3 => {
                let r = (self.load(src, wide) as i64).wrapping_neg() as u64;
                self.flags_logic(r);
                self.store(src, r, ancho);
            }
            // mul SIN signo: `rdx:rax = rax * operando`. CF y OF se
            // encienden solo si la mitad alta no es cero -- que es lo
            // que el `jc` de la Regla 1 sin signo lee. Como el silicio,
            // no toca `zf` ni `sf`. (2026-09-16: la quinta familia del
            // fallo del signo; ver `operaciones.rs` de INTI)
            4 => {
                let a = self.regs[RAX];
                let (lo, hi) = if wide {
                    let r = (a as u128) * (v as u128);
                    (r as u64, (r >> 64) as u64)
                } else {
                    let r = (a as u32 as u64) * (v as u32 as u64);
                    (r & 0xFFFF_FFFF, r >> 32)
                };
                self.regs[RAX] = lo;
                self.regs[RDX] = hi;
                self.cf = hi != 0;
                self.of = hi != 0;
            }
            // div SIN signo e idiv CON signo: el dividendo es rdx:rax en
            // 64 bits y edx:eax en 32; cociente a rax (eax), resto a rdx
            // (edx). Un divisor 0, o un cociente que no cabe, es #DE en el
            // silicio: aqui revienta con su nombre.
            //
            // ** Hasta el 08-10 esto miraba rax solo, porque los emisores
            // ponian rdx = 0 (o `cqo`) en 64 bits. En 32 bits, rax con su
            // mitad alta a cero es un numero POSITIVO: `cdq; idiv ecx` de -1
            // entre 2 daba 0x7FFFFFFF (el silicio: 0). Lo destapo la CPU como
            // libreria de la GPU (LB4): los enteros del Programa son de 32.
            6 | 7 => {
                assert_ne!(v, 0, "#DE: division por cero en el codigo emitido");
                let (alto, bajo) = (self.regs[RDX], self.regs[RAX]);
                let (q, r) = match (ext & 7, wide) {
                    (6, true) => {
                        let n = (alto as u128) << 64 | bajo as u128;
                        let q = n / v as u128;
                        assert!(q <= u64::MAX as u128, "#DE: el cociente de div no cabe en 64 bits");
                        (q as u64, (n % v as u128) as u64)
                    }
                    (6, false) => {
                        let n = (alto as u32 as u64) << 32 | bajo as u32 as u64;
                        let q = n / v as u32 as u64;
                        assert!(q <= u32::MAX as u64, "#DE: el cociente de div no cabe en 32 bits");
                        (q, n % v as u32 as u64)
                    }
                    (_, true) => {
                        let n = ((alto as u128) << 64 | bajo as u128) as i128;
                        let q = n / v as i64 as i128;
                        assert!(i64::try_from(q).is_ok(), "#DE: el cociente de idiv no cabe en 64 bits");
                        (q as i64 as u64, (n % v as i64 as i128) as i64 as u64)
                    }
                    (_, false) => {
                        let n = ((alto as u32 as u64) << 32 | bajo as u32 as u64) as i64;
                        let q = n / v as u32 as i32 as i64;
                        assert!(i32::try_from(q).is_ok(), "#DE: el cociente de idiv no cabe en 32 bits");
                        (q as i32 as u32 as u64, (n % v as u32 as i32 as i64) as i32 as u32 as u64)
                    }
                };
                self.regs[RAX] = q;
                self.regs[RDX] = r;
            }
            other => panic!("grupo F7 /{other} no emitido por BMO"),
        }
    }
}
