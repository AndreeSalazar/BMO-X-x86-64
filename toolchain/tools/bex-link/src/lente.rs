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
