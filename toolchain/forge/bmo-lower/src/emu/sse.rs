//! **SSE ESCALAR: `double` (`sd`) y `float` (`ss`).** El decodificador de la
//! coma flotante escalar del emulador.
//!
//! ## Por que es un fichero (L6a, L6b)
//!
//! ** Salio de `mod.rs` el 2026-09-23, cuando el emisor de SPIR-V trajo la
//! aritmetica SIMPLE (`addss`, `mulss`, `minss`, `ucomiss`, `cvtss2si`...): los
//! sombreadores son `float`, y calcular en doble y redondear daria otro numero
//! en `fma`, en las conversiones y en cada `min`/`max` -- la prueba diferencial
//! contra el oraculo dejaria de ser bit a bit. `mod.rs` estaba a 998 lineas de
//! codigo en la linea base de L6a, y lo que crece no es el despacho clasico:
//! es SSE, que ya tenia su bloque. Asi que el bloque ENTERO se muda aqui (lo
//! de `double` tal cual, sin tocar una linea) y `mod.rs` encoge.
//!
//! ## Las reglas de la casa para SSE
//!
//! - Solo la mitad baja de un `xmm` (ver `Machine::xmm`): todo es ESCALAR.
//! - Las `ss` operan sobre los 32 bits bajos y **dejan los de arriba como
//!   estaban**, que es lo que hace el silicio.
//! - *** `minss`/`maxss` son EXACTAMENTE `a < b ? a : b` y `a > b ? a : b`
//!   (destino a, fuente b): con un NaN o con `+0`/`-0` gana la FUENTE. Es la
//!   definicion de `bmo_spirv_front::math::min/max`, a proposito.
//! - `cvttss2si` NO satura: da el entero mas negativo si no cabe. Quien quiera
//!   saturar lo emite.

use super::*;

/// Los prefijos y bits de REX que el decodificador clasico ya leyo.
pub(super) struct Prefijos {
    pub f2: bool,
    pub f3: bool,
    pub op16: bool,
    pub wide: bool,
    pub rex_r: usize,
    pub rex_x: usize,
    pub rex_b: usize,
}

impl Machine {
    /// Una instruccion SSE escalar: `0F <second>` con sus prefijos.
    pub(super) fn sse(&mut self, second: u8, p: Prefijos) {
        let Prefijos { f2, f3, op16, wide, rex_r, rex_x, rex_b } = p;
        match second {
            // ---- SIMPLE (`ss`): el emisor de SPIR-V (2026-09-23) ----------
            0x51 | 0x58 | 0x59 | 0x5C | 0x5D | 0x5E | 0x5F if f3 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let b = f32::from_bits(self.leer_xmm32(src));
                let a = f32::from_bits(self.xmm[reg] as u32);
                let r = match second {
                    0x51 => sqrt_f32_ieee(b),
                    0x58 => a + b,
                    0x59 => a * b,
                    0x5C => a - b,
                    0x5E => a / b,
                    0x5D => {
                        if a < b {
                            a
                        } else {
                            b
                        }
                    }
                    _ => {
                        if a > b {
                            a
                        } else {
                            b
                        }
                    }
                };
                self.xmm[reg] = (self.xmm[reg] & !0xFFFF_FFFF) | r.to_bits() as u64;
            }
            // ucomiss / comiss -- comparar floats y dejar ZF/CF/PF (iguales
            // aqui: solo difieren en la excepcion de un NaN silencioso, que BMO
            // no atiende).
            0x2E | 0x2F if !op16 && !f2 && !f3 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let b = f32::from_bits(self.leer_xmm32(src));
                let a = f32::from_bits(self.xmm[reg] as u32);
                if a.is_nan() || b.is_nan() {
                    self.zf = true;
                    self.cf = true;
                    self.pf = true;
                } else {
                    self.zf = a == b;
                    self.cf = a < b;
                    self.pf = false;
                }
                self.sf = false;
                self.of = false;
            }
            // cvtsi2ss xmm, r/m32 (o r/m64 con REX.W) -- entero CON SIGNO a
            // float, redondeando al mas cercano.
            0x2A if f3 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = if wide { self.load(src, true) as i64 as f32 } else { self.load(src, false) as u32 as i32 as f32 };
                self.xmm[reg] = (self.xmm[reg] & !0xFFFF_FFFF) | v.to_bits() as u64;
            }
            // cvttss2si r32/r64, xmm -- float a entero TRUNCANDO. Si no cabe
            // (o es NaN) el entero mas negativo.
            0x2C if f3 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = f32::from_bits(self.leer_xmm32(src)) as f64;
                if wide {
                    let r = if v.is_nan() || v >= 9.223_372_036_854_776e18 || v < -9.223_372_036_854_776e18 {
                        i64::MIN
                    } else {
                        v as i64
                    };
                    self.regs[reg] = r as u64;
                } else {
                    let r = if v.is_nan() || v >= 2_147_483_648.0 || v < -2_147_483_648.0 { i32::MIN } else { v as i32 };
                    self.regs[reg] = r as u32 as u64;
                }
            }

            // ---- DOBLE (`sd`) y los movimientos: movido de `mod.rs` tal cual --
            // == SSE ESCALAR ======================================
            //
            // Las catorce que BMO C emite para `float` y `double`, y
            // ni una mas. Hasta hoy **ninguna se ejecutaba**: los 9
            // tests de coma flotante comparaban ventanas de bytes, que
            // es el metodo que la cabecera de este archivo declara
            // insuficiente. La ruta compilaba, daba verde, y ningun
            // CPU la habia corrido.
            //
            // El prefijo decide el ancho, que es como funciona SSE:
            // `F2` escalar doble, `F3` escalar simple, `66` entero
            // empaquetado o comparacion ordenada.

            // movsd/movss xmm, r/m -- CARGA
            0x10 if f2 || f3 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = match src {
                    Operand::Reg(r) => self.xmm[r],
                    Operand::Mem(a) => {
                        // Desde memoria, la mitad alta a cero (los 128).
                        self.xmm_alto[reg] = 0;
                        if f3 {
                            // `movss` carga 32 bits y **pone a cero el
                            // resto** cuando viene de memoria. Desde
                            // otro registro no lo haria; aqui solo se
                            // emite desde memoria.
                            (self.read_u64(a) & 0xFFFF_FFFF) as u32 as u64
                        } else {
                            self.read_u64(a)
                        }
                    }
                };
                self.xmm[reg] = v;
            }
            // movsd/movss r/m, xmm -- ALMACENA
            0x11 if f2 || f3 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.xmm[reg];
                match dst {
                    Operand::Reg(r) => self.xmm[r] = v,
                    // * El ancho importa: `movss` escribe CUATRO
                    // bytes. Escribir ocho pisaria el vecino, que es
                    // exactamente el bug que este emulador ya se comio
                    // una vez con `mov [mem], eax`.
                    Operand::Mem(a) => self.store(Operand::Mem(a), v, if f3 { 4 } else { 8 }),
                }
            }
            // ** sqrtsd / minsd / maxsd -- las que un motor grafico
            // pide y `+ - * /` no dan (2026-08-22).
            //
            // La raiz es UNARIA y las otras dos binarias, pero las tres
            // comparten la forma: destino izquierdo, fuente derecha. Se
            // modelan aqui y no en un bloque aparte porque separarlas
            // seria repetir el `modrm` y el orden de los operandos --
            // que es justo donde este emulador ya se equivoco una vez.
            //
            // *** `minsd`/`maxsd` NO son conmutativas ante un NaN: el
            // silicio devuelve el operando FUENTE si cualquiera de los
            // dos es NaN. Se modela asi a proposito, aunque sorprenda:
            // un emulador que "arregla" al procesador es un emulador que
            // aprueba programas que el metal suspende.
            0x51 | 0x5D | 0x5F if f2 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let b = f64::from_bits(self.leer_xmm(src));
                let a = f64::from_bits(self.xmm[reg]);
                let r = match second {
                    0x51 => b.sqrt(),
                    0x5D => {
                        if a.is_nan() || b.is_nan() || b < a {
                            b
                        } else {
                            a
                        }
                    }
                    _ => {
                        if a.is_nan() || b.is_nan() || b > a {
                            b
                        } else {
                            a
                        }
                    }
                };
                self.xmm[reg] = r.to_bits();
            }
            // addsd / mulsd / subsd / divsd
            0x58 | 0x59 | 0x5C | 0x5E if f2 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let b = f64::from_bits(self.leer_xmm(src));
                let a = f64::from_bits(self.xmm[reg]);
                // El orden NO es conmutativo en dos de las cuatro, y
                // ese fue el bug que el banco de pruebas ya cazo una
                // vez en los enteros: el destino es el operando
                // IZQUIERDO.
                let r = match second {
                    0x58 => a + b,
                    0x59 => a * b,
                    0x5C => a - b,
                    _ => a / b,
                };
                self.xmm[reg] = r.to_bits();
            }
            // cvtsd2ss (F2) / cvtss2sd (F3) -- cambiar de precision
            0x5A if f2 || f3 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.leer_xmm(src);
                self.xmm[reg] = if f2 {
                    // double -> float: **se pierde precision aqui**, y
                    // tiene que perderse. Guardar el double en un
                    // `float` y leerlo daria mas digitos de los que
                    // caben, y el test no veria lo que ve el silicio.
                    (f64::from_bits(v) as f32).to_bits() as u64
                } else {
                    (f32::from_bits(v as u32) as f64).to_bits()
                };
            }
            // comisd -- comparar y dejar el resultado en las BANDERAS
            0x2F if op16 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let b = f64::from_bits(self.leer_xmm(src));
                let a = f64::from_bits(self.xmm[reg]);
                // * `comisd` pone ZF/CF/PF, **no** SF ni OF, y por eso
                // los saltos que le siguen son los SIN SIGNO (`ja`,
                // `jb`), no `jg`/`jl`. Modelarlo con SF seria hacer
                // pasar codigo que en el silicio salta al reves.
                //
                // No-ordenado (algun NaN) pone las TRES a 1, `pf`
                // incluida.
                //
                // ** Esto decia "no pasa hoy" hasta que INTI empezo a
                // comparar flotantes. Ahora pasa, y `pf` es la unica
                // bandera que distingue un NaN de una igualdad: sin
                // ella, `a = b` con un NaN dentro contesta que si --
                // porque el no-ordenado enciende `zf` igual que la
                // igualdad de verdad.
                if a.is_nan() || b.is_nan() {
                    self.zf = true;
                    self.cf = true;
                    self.pf = true;
                } else {
                    self.zf = a == b;
                    self.cf = a < b;
                    self.pf = false;
                }
                self.sf = false;
                self.of = false;
            }
            // xorpd xmm, xmm -- el cero de la coma flotante
            0x57 if op16 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.leer_xmm(src);
                self.xmm[reg] ^= v;
                if let Operand::Reg(r) = src {
                    self.xmm_alto[reg] ^= self.xmm_alto[r];
                }
            }
            // movq xmm, r64 -- los BITS de un entero, tal cual
            //
            // * NO es una conversion: es como BMO C mete un literal
            // `double` en un registro SSE. El compilador pone los bits
            // del numero en `rax` con un `mov imm64` y los mueve aqui
            // sin tocarlos. Confundir esto con `cvtsi2sd` daria
            // `4614256656552045848.0` donde tiene que haber `3.14`.
            //
            // Tambien lo usa la NEGACION, que en coma flotante es un
            // `xor` con el bit de signo -- no una resta contra cero,
            // que daria `-0.0` mal para el cero.
            0x6E if op16 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.load(src, wide);
                self.xmm[reg] = if wide { v } else { v & 0xFFFF_FFFF };
                // `movd`/`movq` hacia un `xmm` ponen a cero hasta el bit 127.
                self.xmm_alto[reg] = 0;
            }
            // movq r64, xmm / movd r32, xmm -- el camino de VUELTA
            //
            // * La hermana de `0x6E`, y la que faltaba: aquella mete
            // bits en un registro SSE, esta los saca. Es como BMO C
            // pasa un `double` a una funcion -- los argumentos van por
            // la PILA, asi que el valor tiene que bajar de `xmm0` a un
            // registro entero para poder empujarlo.
            //
            // Ojo al reparto de campos: aqui el operando de ModRM que
            // manda es el `reg`, y **es el XMM**; el destino entero es
            // el `r/m`. Al reves que en casi todo lo demas, y por eso
            // se escribe explicito.
            0x7E if op16 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.xmm[reg];
                // Sin REX.W son cuatro bytes: un `float`, no un
                // `double`. Llevarse los ocho seria arrastrar la mitad
                // alta de la mantisa a un registro que declara 32 bits.
                let bytes = if wide { 8 } else { 4 };
                self.store(dst, if wide { v } else { v & 0xFFFF_FFFF }, bytes);
            }
            // cvtsi2sd xmm, r64 -- entero con signo a double
            0x2A if f2 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                // CON SIGNO: `-1` tiene que dar `-1.0` y no
                // 18446744073709551615.0.
                let v = self.load(src, true) as i64;
                self.xmm[reg] = (v as f64).to_bits();
            }
            // cvttsd2si r64, xmm -- double a entero, TRUNCANDO
            0x2C if f2 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = f64::from_bits(self.leer_xmm(src));
                // `cvtt` trunca hacia cero; `cvt` (0x2D) redondearia.
                // BMO solo emite el que trunca, que es lo que manda C
                // para un cast a entero: `(int)2.7` son 2.
                //
                // ** Y LO QUE PASA CUANDO NO CABE, que estaba mal.
                //
                // Esto escribia `v as i64` a secas, que en Rust
                // **satura**: 1e30 daba el entero mas grande y un NaN
                // daba cero. El silicio no hace ninguna de las dos:
                // devuelve el entero mas NEGATIVO como centinela, para
                // los dos casos y sin levantar nada.
                //
                // La diferencia no es academica. Es la unica signal que
                // el procesador da de que la conversion no cabia, asi
                // que **es la que la Regla 12 de INTI tiene que mirar**.
                // Con la version que satura, un programa que comprueba
                // el centinela pasaba aqui y atrapaba en metal -- o al
                // reves, que es peor.
                //
                // Es exactamente la clase de fallo que este emulador
                // existe para no tener: uno donde el banco dice que si
                // y el Ryzen dice que no.
                let r = if v.is_nan() || v >= 9223372036854775808.0 || v < -9223372036854775808.0
                {
                    i64::MIN
                } else {
                    v as i64
                };
                self.write_reg(reg, r as u64, true);
            }
            // ---- EMPAQUETADO `ps`: cuatro `flotante32` (2026-09-26) ------
            //
            // Sin prefijo. Solo lo que emiten las filas `sse_*4f` de
            // `intrinsics.toml` (INTI: `suma_de_cuatro32`...), y en su forma.
            // movups xmm, xmm/m128
            0x10 if !op16 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.leer128(src);
                self.escribir128(reg, v);
            }
            // movups m128/xmm, xmm
            0x11 if !op16 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.leer128(Operand::Reg(reg));
                match dst {
                    Operand::Reg(r) => self.escribir128(r, v),
                    Operand::Mem(a) => {
                        for (k, x) in v.iter().enumerate() {
                            self.store(Operand::Mem(a + 4 * k as u64), *x as u64, 4);
                        }
                    }
                }
            }
            // addps / mulps / subps: carril a carril, cada uno redondeado a
            // `f32` como el silicio (la cuenta de Rust en `f32` es IEEE).
            0x58 | 0x59 | 0x5C if !op16 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.leer128(Operand::Reg(reg));
                let b = self.leer128(src);
                let mut r = [0u32; 4];
                for k in 0..4 {
                    let (x, y) = (f32::from_bits(a[k]), f32::from_bits(b[k]));
                    r[k] = match second {
                        0x58 => x + y,
                        0x59 => x * y,
                        _ => x - y,
                    }
                    .to_bits();
                }
                self.escribir128(reg, r);
            }
            // shufps xmm, xmm/m128, imm8: los dos de abajo salen del destino,
            // los dos de arriba de la fuente; cada uno lo elige un par de bits.
            0xC6 if !op16 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let imm = self.fetch_u8();
                let a = self.leer128(Operand::Reg(reg));
                let b = self.leer128(src);
                let sel = |k: u32| (imm >> (2 * k) & 3) as usize;
                self.escribir128(reg, [a[sel(0)], a[sel(1)], b[sel(2)], b[sel(3)]]);
            }
            other => panic!("opcode 0F {other:#04X} no emitido por BMO"),
        }
    }

    /// Los cuatro carriles de 32 bits de un `xmm` entero, o 16 bytes de
    /// memoria.
    fn leer128(&self, op: Operand) -> [u32; 4] {
        let (lo, hi) = match op {
            Operand::Reg(r) => (self.xmm[r], self.xmm_alto[r]),
            Operand::Mem(a) => (self.read_u64(a), self.read_u64(a + 8)),
        };
        [lo as u32, (lo >> 32) as u32, hi as u32, (hi >> 32) as u32]
    }

    fn escribir128(&mut self, reg: usize, v: [u32; 4]) {
        self.xmm[reg] = v[0] as u64 | (v[1] as u64) << 32;
        self.xmm_alto[reg] = v[2] as u64 | (v[3] as u64) << 32;
    }

    /// Los 32 bits bajos de un operando `ss`: de un registro, o CUATRO bytes de
    /// memoria (leer ocho tocaria al vecino, que puede no existir).
    fn leer_xmm32(&self, op: Operand) -> u32 {
        match op {
            Operand::Reg(r) => self.xmm[r] as u32,
            Operand::Mem(a) => (self.read_u64(a) & 0xFFFF_FFFF) as u32,
        }
    }
}

/// La raiz IEEE de un `f32`: la de `f64` es exacta y, para una raiz, redondear
/// de doble a simple no puede fallar (53 >= 2*24 + 2). Es lo que da `sqrtss`.
fn sqrt_f32_ieee(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}
