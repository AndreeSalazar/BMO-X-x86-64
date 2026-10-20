//! # traza-gpu -- la receta de NVIDIA, leida de tu propia 3060
//!
//! GL2 de `docs/plan/PLAN_GPU_COMO_LINUX.md` (20-10). El propietario arranco
//! su RTX 3060 en ULTRA LinuX con `nvidia-open` 615.78.08 y su firmware GSP
//! de la misma version, y alli arranca "normal". BMO-X, con el GSP-RM
//! 570.144, a veces no: el booter da 0x15 (G0). Lo que se trae de Linux es
//! un DATO, no codigo: la traza `mmiotrace` del kernel -- cada lectura y
//! escritura que el driver de NVIDIA hizo en la tarjeta --, grabada en la
//! misma maquina.
//!
//! ```text
//!    leer      las lineas de mmiotrace (MAP, R, W, MARK, PCIDEV) a accesos
//!    bar0      donde vive la BAR0 de la NVIDIA (PCIDEV 10de, o el MAP de 16 MiB)
//!    nombrar   el desplazamiento dentro de la BAR0 con los nombres de BMO-X
//!              (arranque/falcon.rs: GSP 0x110000, SEC2 0x840000, CPUCTL,
//!              MAILBOX0/1, BCR_CTRL, RISCV_CPUCTL...)
//!    cortar    hasta la escritura que arranca el SEC2 (CPUCTL con STARTCPU):
//!              ahi suelta NVIDIA al booter
//!    comparar  dos recetas, acceso a acceso: la primera diferencia
//! ```
//!
//! El formato es el de `Documentation/trace/mmiotrace.rst` del kernel:
//!
//! ```text
//!    MAP    tiempo id fisica virtual medida pc pid
//!    R|W    ancho tiempo id fisica valor pc pid
//!    MARK   tiempo texto
//!    PCIDEV bus_devfn vendedor_dispositivo irq recurso0 recurso1 ...
//! ```

/// Una lectura o una escritura de la tarjeta, ya dentro de la BAR0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acceso {
    pub escribe: bool,
    pub ancho: u8,
    /// El tiempo de la traza, en microsegundos.
    pub us: u64,
    /// El desplazamiento dentro de la BAR0.
    pub reg: u32,
    pub valor: u32,
}

/// Una linea de la traza, leida.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Linea {
    Map { fisica: u64, medida: u64 },
    Acceso { escribe: bool, ancho: u8, us: u64, fisica: u64, valor: u32 },
    Marca { us: u64, texto: String },
    Pcidev { vendedor: u16, dispositivo: u16, recursos: Vec<u64> },
}

fn hex(s: &str) -> Option<u64> {
    u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

/// `0.123456` segundos a microsegundos, sin coma flotante.
fn microsegundos(s: &str) -> Option<u64> {
    let (ent, frac) = s.split_once('.').unwrap_or((s, ""));
    let mut f = frac.to_string();
    f.truncate(6);
    while f.len() < 6 {
        f.push('0');
    }
    Some(ent.parse::<u64>().ok()? * 1_000_000 + f.parse::<u64>().ok()?)
}

/// **Una linea de mmiotrace**, o `None` si no es de las que importan.
pub fn leer_linea(l: &str) -> Option<Linea> {
    let c: Vec<&str> = l.split_whitespace().collect();
    match *c.first()? {
        "MAP" if c.len() >= 6 => Some(Linea::Map { fisica: hex(c[3])?, medida: hex(c[5])? }),
        "R" | "W" if c.len() >= 6 => Some(Linea::Acceso { escribe: c[0] == "W", ancho: c[1].parse().ok()?, us: microsegundos(c[2])?, fisica: hex(c[4])?, valor: hex(c[5])? as u32 }),
        "MARK" if c.len() >= 2 => Some(Linea::Marca { us: microsegundos(c[1])?, texto: c[2..].join(" ") }),
        "PCIDEV" if c.len() >= 5 => {
            let vd = hex(c[2])?;
            Some(Linea::Pcidev { vendedor: (vd >> 16) as u16, dispositivo: vd as u16, recursos: c[4..].iter().filter_map(|x| hex(x)).collect() })
        }
        _ => None,
    }
}

/// La BAR0 de NVIDIA: GA106 la tiene de 16 MiB.
pub const BAR0_MEDIDA: u64 = 0x100_0000;
/// El vendedor PCI de NVIDIA.
pub const NVIDIA: u16 = 0x10DE;

/// **Donde empieza la BAR0** en esta traza: el primer recurso de un PCIDEV
/// de NVIDIA, o si no, el primer MAP de 16 MiB.
pub fn bar0(lineas: &[Linea]) -> Option<u64> {
    let por_pci = lineas.iter().find_map(|l| match l {
        Linea::Pcidev { vendedor: NVIDIA, recursos, .. } => recursos.first().map(|r| r & !0xF),
        _ => None,
    });
    por_pci.or_else(|| {
        lineas.iter().find_map(|l| match l {
            Linea::Map { fisica, medida: BAR0_MEDIDA } => Some(*fisica),
            _ => None,
        })
    })
}

/// **Los accesos a la BAR0**, en orden.
pub fn accesos(texto: &str) -> Result<Vec<Acceso>, String> {
    let lineas: Vec<Linea> = texto.lines().filter_map(leer_linea).collect();
    let base = bar0(&lineas).ok_or("la traza no dice donde esta la BAR0 de la NVIDIA (ni un PCIDEV 10de, ni un MAP de 16 MiB)")?;
    Ok(lineas
        .into_iter()
        .filter_map(|l| match l {
            Linea::Acceso { escribe, ancho, us, fisica, valor } if fisica >= base && fisica < base + BAR0_MEDIDA => Some(Acceso { escribe, ancho, us, reg: (fisica - base) as u32, valor }),
            _ => None,
        })
        .collect())
}

/// Las unidades de la GA106, con los mismos nombres que BMO-X
/// (`arranque/secuenciador.rs::unidad`, `arranque/falcon.rs`).
pub fn unidad(reg: u32) -> &'static str {
    match reg {
        0x0000_0000..=0x0000_0FFF => "PMC",
        0x0008_8000..=0x0008_8FFF => "PCI (config)",
        0x0010_0000..=0x0010_FFFF => "PFB",
        0x0011_0000..=0x0011_1FFF => "falcon GSP",
        0x0011_8000..=0x0011_8FFF => "PGC6/BSI",
        0x001F_A000..=0x001F_AFFF => "PFB (WPR)",
        0x0082_0000..=0x0082_FFFF => "fusibles",
        0x0084_0000..=0x0084_1FFF => "falcon SEC2",
        _ => "?",
    }
}

/// El nombre de un registro dentro de un falcon (`arranque/falcon.rs`).
pub fn nombre_falcon(off: u32) -> Option<&'static str> {
    Some(match off {
        0x040 => "MAILBOX0",
        0x044 => "MAILBOX1",
        0x080 => "OS",
        0x084 => "RM",
        0x0f4 => "HWCFG2",
        0x100 => "CPUCTL",
        0x104 => "BOOTVEC",
        0x108 => "HWCFG",
        0x10c => "DMACTL",
        0x110 => "DMATRFBASE",
        0x114 => "DMATRFMOFFS",
        0x118 => "DMATRFCMD",
        0x11c => "DMATRFFBOFFS",
        0x128 => "DMATRFBASE1",
        0x12c => "HWCFG1",
        0x130 => "CPUCTL_ALIAS",
        0x1c0 => "DMEMC",
        0x1c4 => "DMEMD",
        0x3c0 => "ENGINE",
        0x600 => "TRANSCFG",
        0x624 => "FBIF_CTL",
        0x1180 => "MOD_SEL",
        0x1198 => "BROM_UCODE_ID",
        0x119c => "BROM_ENGIDMASK",
        0x1210 => "BROM_PARAADDR",
        0x1388 => "RISCV_CPUCTL",
        0x1668 => "BCR_CTRL",
        _ => return None,
    })
}

/// **El nombre de un registro**: `falcon SEC2 CPUCTL`, o la unidad y su
/// desplazamiento si no se conoce.
pub fn nombre(reg: u32) -> String {
    let base = match reg {
        0x0011_0000..=0x0011_1FFF => Some(0x0011_0000),
        0x0084_0000..=0x0084_1FFF => Some(0x0084_0000),
        _ => None,
    };
    match base.and_then(|b| nombre_falcon(reg - b)) {
        Some(n) => format!("{} {}", unidad(reg), n),
        None => format!("{} +{:#x}", unidad(reg), reg),
    }
}

/// El CPUCTL del SEC2, y su bit STARTCPU (`falcon.rs`: CPUCTL 0x100, bit 1).
pub const SEC2_CPUCTL: u32 = 0x0084_0100;
pub const STARTCPU: u32 = 1 << 1;

/// **Donde NVIDIA suelta al booter**: el indice de la primera escritura al
/// CPUCTL del SEC2 con STARTCPU, o `None` si la traza no llega.
pub fn arranque_del_booter(a: &[Acceso]) -> Option<usize> {
    a.iter().position(|x| x.escribe && x.reg == SEC2_CPUCTL && x.valor & STARTCPU != 0)
}

/// **La receta**: los accesos hasta (incluido) el arranque del booter, o
/// todos si no llega, y si llego.
pub fn receta(a: &[Acceso]) -> (&[Acceso], bool) {
    match arranque_del_booter(a) {
        Some(i) => (&a[..=i], true),
        None => (a, false),
    }
}

/// Si un acceso es de las unidades del arranque (falcons, BSI, WPR,
/// fusibles), lo que se compara por defecto: lo demas (la pantalla, el
/// reloj) cambia entre corridas sin decir nada del booter.
pub fn del_arranque(x: &Acceso) -> bool {
    matches!(unidad(x.reg), "falcon GSP" | "falcon SEC2" | "PGC6/BSI" | "PFB (WPR)" | "fusibles")
}

/// **La primera diferencia** entre dos recetas, acceso a acceso (sin el
/// tiempo): `(indice, de a, de b)`. `None` si son iguales.
pub fn primera_diferencia(a: &[Acceso], b: &[Acceso], con_lecturas: bool) -> Option<(usize, Option<Acceso>, Option<Acceso>)> {
    let filtra = |v: &[Acceso]| -> Vec<Acceso> { v.iter().copied().filter(|x| del_arranque(x) && (con_lecturas || x.escribe)).collect() };
    let (fa, fb) = (filtra(a), filtra(b));
    let igual = |x: &Acceso, y: &Acceso| x.escribe == y.escribe && x.reg == y.reg && x.valor == y.valor;
    (0..fa.len().max(fb.len())).find(|&i| match (fa.get(i), fb.get(i)) {
        (Some(x), Some(y)) => !igual(x, y),
        _ => true,
    })
    .map(|i| (i, fa.get(i).copied(), fb.get(i).copied()))
}

/// Una fila para leer: `+tiempo  W  falcon SEC2 CPUCTL  = 0x2`.
pub fn fila(x: &Acceso, t0: u64) -> String {
    format!("+{:>9} us  {}  {:<32} {} {:#010x}", x.us.saturating_sub(t0), if x.escribe { "W" } else { "R" }, nombre(x.reg), if x.escribe { "<-" } else { "->" }, x.valor)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una traza SINTETICA con la forma de mmiotrace (no es de la 3060: las
    /// direcciones y los valores son inventados para la prueba).
    const SINTETICA: &str = "VERSION 20070824\n\
PCIDEV 2900 10de2504 71 fb000000 0 0\n\
MAP 0.000000 1 0xfb000000 0xffffc90000000000 0x1000000 0x0 0\n\
R 4 0.000010 1 0xfb000000 0xb46000a1 0x0 0\n\
W 4 0.000020 1 0xfb110040 0x0 0x0 0\n\
R 4 0.000030 1 0xfb840100 0x10 0x0 0\n\
MARK 0.000040 antes del booter\n\
W 4 0.000050 1 0xfb840040 0x15 0x0 0\n\
W 4 0.000060 1 0xfb840100 0x2 0x0 0\n\
R 4 0.000070 1 0xfb840040 0x0 0x0 0\n\
W 4 0.000080 1 0xa0000000 0x1 0x0 0\n";

    #[test]
    fn the_trace_is_cut_where_the_booter_starts_and_named_like_bmo_x() {
        let a = accesos(SINTETICA).unwrap();
        // la escritura fuera de la BAR0 no entra
        assert_eq!(a.len(), 6);
        assert_eq!(a[0].reg, 0);
        assert_eq!(nombre(a[1].reg), "falcon GSP MAILBOX0");
        let (r, llego) = receta(&a);
        assert!(llego);
        assert_eq!(r.len(), 5);
        assert_eq!(nombre(r[4].reg), "falcon SEC2 CPUCTL");
        assert!(fila(&r[4], r[0].us).contains("+       50 us  W  falcon SEC2 CPUCTL"));
    }

    #[test]
    fn two_recipes_say_their_first_difference() {
        let a = accesos(SINTETICA).unwrap();
        let mut b = a.clone();
        assert_eq!(primera_diferencia(&a, &b, true), None);
        b[3].valor = 0x0; // el MAILBOX0 del SEC2, otro valor
        let (i, x, y) = primera_diferencia(&a, &b, false).unwrap();
        assert_eq!(i, 1, "las escrituras del arranque: GSP MAILBOX0, SEC2 MAILBOX0...");
        assert_eq!((x.unwrap().valor, y.unwrap().valor), (0x15, 0x0));
        // una receta que se queda corta tambien es una diferencia
        assert_eq!(primera_diferencia(&a[..3], &a, false).map(|d| d.0), Some(1));
    }

    #[test]
    fn without_the_bar0_it_says_no() {
        assert!(accesos("R 4 0.1 1 0x1000 0x0 0x0 0\n").is_err());
        assert_eq!(microsegundos("1.5"), Some(1_500_000));
        assert_eq!(microsegundos("0.000123"), Some(123));
    }
}
