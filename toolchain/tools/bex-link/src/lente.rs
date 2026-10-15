//! **LA LENTE EN `bex-link`** (B4 de `docs/plan/EL_FOCO.md`, 14-10): lo que
//! LLVM escribio, leido entero y juzgado contra lo que decidio el propietario.
//!
//! > 14-10, el propietario: *"que ese LLVM de rust TENGA OBLIGACION que tengas
//! > control ... no quiero tener choque con LLVM luego que decida sino que yo
//! > decido por motivos"*.

use crate::{u32le, u64le, SeccionElf, SHT_SYMTAB};

/// Un simbolo del ELF: donde empieza, cuanto mide y como se llama.
pub struct Simbolo {
    pub addr: u64,
    #[allow(dead_code)] // el barrido corta en el siguiente simbolo, no en la medida
    pub size: u64,
    pub nombre: String,
}

/// Los simbolos de la `.symtab` con direccion (funciones y etiquetas de
/// `global_asm!`, que salen sin tipo y sin medida).
pub fn simbolos(b: &[u8], secciones: &[SeccionElf]) -> Vec<Simbolo> {
    let mut out = Vec::new();
    let Some(st) = secciones.iter().find(|s| s.tipo == SHT_SYMTAB) else { return out };
    let Some(strs) = secciones.get(st.link as usize) else { return out };
    let n = st.size as usize / 24;
    for i in 0..n {
        let e = st.offset as usize + i * 24;
        if e + 24 > b.len() {
            break;
        }
        let tipo = b[e + 4] & 0xF;
        // 0 NOTYPE (las de `global_asm!`), 2 FUNC. Ni secciones ni ficheros.
        if tipo != 0 && tipo != 2 {
            continue;
        }
        let addr = u64le(b, e + 8);
        if addr == 0 {
            continue;
        }
        let nom = strs.offset as usize + u32le(b, e) as usize;
        let fin = b[nom..].iter().position(|&c| c == 0).map_or(nom, |f| nom + f);
        out.push(Simbolo { addr, size: u64le(b, e + 16), nombre: String::from_utf8_lossy(&b[nom..fin]).into_owned() });
    }
    out
}

/// **El barrido de TODO `.text`**: de su principio a su final, en tramos que
/// empiezan en cada simbolo -- con medida o sin ella (las de `global_asm!`)
/// -- y acaban en el siguiente. Cada tramo se lee de seguido, relleno
/// incluido (`int3`, `nop`). No queda un byte del codigo sin leer: lo que la
/// lente no sabe se apunta y el tramo sigue en el siguiente simbolo.
pub struct Barrido {
    pub leidas: usize,
    /// Cada instruccion que pide una extension: `(extension, offset, si sin
    /// ella falla)`.
    pub piden: Vec<(&'static str, usize, bool)>,
    /// Lo que no sabe leer: `(offset, por que)`.
    pub no_sabe: Vec<(usize, bmo_lente_x86::NoSe)>,
    /// Los offsets donde empieza cada instruccion (para el oraculo).
    pub inicios: Vec<usize>,
}

pub fn barrer(code: &[u8], base: u64, simbolos: &[Simbolo]) -> Barrido {
    let mut cortes: Vec<usize> = simbolos
        .iter()
        .filter(|s| s.addr >= base && s.addr < base + code.len() as u64)
        .map(|s| (s.addr - base) as usize)
        .collect();
    cortes.push(0);
    cortes.push(code.len());
    cortes.sort_unstable();
    cortes.dedup();
    let mut b = Barrido { leidas: 0, piden: Vec::new(), no_sabe: Vec::new(), inicios: Vec::new() };
    for w in cortes.windows(2) {
        let (mut en, fin) = (w[0], w[1]);
        while en < fin {
            match bmo_lente_x86::lee(&code[en..fin.min(en + 15)]) {
                Ok(i) => {
                    b.leidas += 1;
                    b.inicios.push(en);
                    if !i.fila.pide.is_empty() {
                        b.piden.push((i.fila.pide, en, i.fila.sin_ella_falla()));
                    }
                    en += i.largo;
                }
                Err(e) => {
                    b.no_sabe.push((en, e));
                    break;
                }
            }
        }
    }
    b
}

/// El simbolo que contiene `off` (para decir los sitios con nombre).
pub fn donde(off: usize, base: u64, simbolos: &[Simbolo]) -> String {
    let a = base + off as u64;
    simbolos
        .iter()
        .filter(|s| s.addr <= a)
        .max_by_key(|s| s.addr)
        .map_or(format!("+0x{off:x}"), |s| format!("{}+0x{:x}", s.nombre, a - s.addr))
}

/// **El juicio**: lo que el barrido encontro, contra lo que el propietario
/// decidio (`mano`). Devuelve la mascara que el `.bex` tiene que DECLARAR (lo
/// que sin su extension da `#UD`) o la lista de motivos para PARAR el build.
///
/// ** Para por dos cosas, y las dos son la obligacion de LLVM:
///
/// ```text
///    una instruccion que la lente no sabe leer   codigo que no se puede leer
///                                                no se puede juzgar
///    una extension que el perfil no permite      el propietario decide, no
///                                                LLVM ni un `asm!` suelto
/// ```
///
/// Lo que se DEGRADA sin su extension (`tzcnt` es `rep bsf`) ni se declara
/// ni para nada: no pide la extension para correr.
pub fn juzgar(b: &Barrido, mano: u64, nombre_de: impl Fn(usize) -> String) -> Result<u64, Vec<String>> {
    use bmo_abi::bef::requisitos::instrucciones::{nombres, por_nombre};
    let mut motivos = Vec::new();
    for (en, e) in b.no_sabe.iter().take(10) {
        motivos.push(format!("{}: la lente no sabe leer esto ({e:?}) -- o se le ensena (`bmo-lente-x86/src/tabla.rs`), o no entra", nombre_de(*en)));
    }
    if b.no_sabe.len() > 10 {
        motivos.push(format!("... y {} sitios mas que la lente no sabe leer", b.no_sabe.len() - 10));
    }
    let mut usa = 0u64;
    let mut visto_fuera = 0u64;
    for &(ext, en, falla) in &b.piden {
        if !falla {
            continue;
        }
        let bit = por_nombre(ext).map_or(0, |e| e.bit);
        usa |= bit;
        if bit & !mano != 0 && visto_fuera & bit == 0 {
            visto_fuera |= bit;
            motivos.push(format!("{}: usa {ext}, y PERFIL/CPU.txt dice `mano: no` -- lo decide el propietario", nombre_de(en)));
        }
    }
    let _ = nombres;
    if motivos.is_empty() { Ok(usa) } else { Err(motivos) }
}

/// `bex-link --censo`: el barrido, y lo que la lente no sabe leer agrupado
/// por su forma, con cuantas y un ejemplo; y que extensiones pide el codigo.
pub fn censo(b: &[u8], elf: &crate::Elf) {
    let Some(t) = elf.secciones.iter().find(|s| s.name == ".text") else { return };
    let code = &b[t.offset as usize..(t.offset + t.size) as usize];
    let r = barrer(code, t.addr, &elf.simbolos);
    if std::env::var_os("BEX_LINK_DIRECCIONES").is_some() {
        for &o in &r.inicios {
            println!("@{:x}", t.addr as usize + o);
        }
    }
    let mut faltan: std::collections::BTreeMap<String, (usize, String)> = Default::default();
    for (en, e) in &r.no_sabe {
        let bytes: Vec<String> = code[*en..(*en + 8).min(code.len())].iter().map(|b| format!("{b:02x}")).collect();
        let f = faltan.entry(format!("{e:?}")).or_insert((0, format!("{}: {}", donde(*en, t.addr, &elf.simbolos), bytes.join(" "))));
        f.0 += 1;
    }
    let mut ext: std::collections::BTreeMap<&str, (usize, String)> = Default::default();
    for (n, en, _) in &r.piden {
        ext.entry(n).or_insert((0, donde(*en, t.addr, &elf.simbolos))).0 += 1;
    }
    println!("censo: {} B de .text, {} instrucciones leidas; {} formas que la lente no sabe; extensiones: {}", code.len(), r.leidas, faltan.len(),
        ext.iter().map(|(n, (c, d))| format!("{n} x{c} (p.ej. {d})")).collect::<Vec<_>>().join(", "));
    let mut v: Vec<_> = faltan.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (k, (n, ej)) in v {
        println!("  {n:>5}  {k}\n         {ej}");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_abi::bef::requisitos::instrucciones::por_nombre;

    fn bit(n: &str) -> u64 {
        por_nombre(n).unwrap().bit
    }

    /// `roundsd` (SSE4.1) y `rdrand`: se declaran si el propietario los
    /// permite; si no, el build para con el sitio. `tzcnt` ni lo uno ni lo otro.
    #[test]
    fn lo_que_no_decidio_el_propietario_para_el_build() {
        // 0: roundsd xmm0, xmm0, 2 / 6: tzcnt rax, rsi / 11: rdrand eax / 14: ret
        let code = [0x66, 0x0F, 0x3A, 0x0B, 0xC0, 0x02, 0xF3, 0x48, 0x0F, 0xBC, 0xC6, 0x0F, 0xC7, 0xF0, 0xC3];
        let simbolos = vec![Simbolo { addr: 0x1000, size: code.len() as u64, nombre: "f".into() }];
        let b = barrer(&code, 0x1000, &simbolos);
        assert_eq!(b.leidas, 4);
        let todo = bit("SSE4.1") | bit("RDRAND");
        assert_eq!(juzgar(&b, todo, |o| format!("+{o}")), Ok(todo), "tzcnt no se declara: se degrada");
        let e = juzgar(&b, bit("SSE4.1"), |o| format!("+{o}")).unwrap_err();
        assert_eq!(e, vec!["+11: usa RDRAND, y PERFIL/CPU.txt dice `mano: no` -- lo decide el propietario".to_string()]);
    }

    /// Lo que la lente no sabe leer tambien para: no se juzga a ciegas.
    #[test]
    fn lo_que_no_se_lee_para_el_build() {
        let code = [0x0F, 0x0D, 0x00, 0xC3];
        let b = barrer(&code, 0, &[]);
        let e = juzgar(&b, u64::MAX, |o| format!("+{o}")).unwrap_err();
        assert!(e[0].starts_with("+0: la lente no sabe leer esto"), "{e:?}");
    }
}
