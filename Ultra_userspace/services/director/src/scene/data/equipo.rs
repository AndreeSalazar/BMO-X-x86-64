//! **EL EQUIPO** -- "Dispositivos y unidades", la primera solapa de ESTRATOS
//! (2026-09-29).
//!
//! [consumo] NADA      mide los volumenes al ENTRAR en la vista o con `R`;
//!                     pintar solo mira lo ya medido (L6h)
//!
//! ## Por que existe
//!
//! El propietario, con la captura de `Este equipo` de su Windows delante:
//! *"que ESTRATOS cambie en apariencia asi, me gusta eso ... porque en ESTRATOS
//! se ve como basico ... al estilo hyprland"*. La solapa `numeros` contesta
//! *como esta ESTRATOS*; esta contesta la pregunta de antes, **que discos
//! tengo y cuanto les queda**, de un vistazo y todos juntos.
//!
//! ```text
//!    v Dispositivos y unidades
//!    +--------------------------------+  +--------------------------------+
//!    | [==]  BMO (A:)          FAT32  |  | [==]  Personal (D:)      NTFS  |
//!    |  .    [#####-------------]     |  |  .    [###############----]    |
//!    |       30.3 GB disponibles de.. |  |       26.5 GB disponibles de.. |
//!    +--------------------------------+  +--------------------------------+
//! ```
//!
//! ## Lo de Hyprland, y lo que cuesta
//!
//! Tarjetas con aire entre ellas (sus `gaps`), esquinas redondeadas y
//! suavizadas, y la ELEGIDA con el borde en DEGRADADO, que es el
//! `col.active_border` de Hyprland (`rgba(33ccffee) rgba(00ff99ee)`). Sin alfa
//! ni GPU: el degradado son columnas de un pixel, unas trescientas por
//! tarjeta, y solo se pinta al cambiar algo.
//!
//! ## Los numeros son MEDIDOS, no leidos de una pista
//!
//! Lo libre sale del MAPA de cada volumen, contado entero: la FAT en DATOS y
//! EFI (`bmo_fat32::FatVolume::libres`, que no se fia del `FSInfo`), el
//! `$Bitmap` en el disco Personal (`bmo_ntfs::Volumen::libres`, contado una vez
//! al montar) y el mapa de bloques en ESTRATOS. Lo que no se pudo contar se
//! DICE, no se pinta como lleno ni como vacio.
//!
//! [!] El disco Personal y EFI son SOLO LECTURA y la tarjeta lo lleva escrito.
//! `C:` (el NVMe de Windows) no aparece: BMO-X ni lo mira (guardian `ajeno`).

use bmo_userland as bmo;
use core::ptr::addr_of_mut;

use super::*;
use crate::scene::borde;
use crate::scene::zonas::Zona;
use crate::text::decimal;

/// Las unidades, en el orden de `Este equipo` (por letra).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unidad {
    Datos,
    Personal,
    Estratos,
    Efi,
}

pub(crate) const TODAS: [Unidad; 4] = [Unidad::Datos, Unidad::Personal, Unidad::Estratos, Unidad::Efi];

impl Unidad {
    pub(crate) fn nombre(self) -> &'static str {
        match self {
            Unidad::Datos => "BMO (A:)",
            Unidad::Personal => "Personal (D:)",
            Unidad::Estratos => "ESTRATOS (F:)",
            Unidad::Efi => "EFI (arranque)",
        }
    }

    fn sistema(self) -> &'static str {
        match self {
            Unidad::Datos | Unidad::Efi => "FAT32",
            Unidad::Personal => "NTFS",
            Unidad::Estratos => "ESTRATOS",
        }
    }

    fn solo_lectura(self) -> bool {
        matches!(self, Unidad::Personal | Unidad::Efi)
    }

    /// El color del disco: el de su ventana donde lo tiene (ESTRATOS es
    /// verde), y uno propio y distinto en las demas.
    fn color(self) -> u32 {
        match self {
            Unidad::Datos => 0x0060_A5FA,
            Unidad::Personal => 0x00F0_B060,
            Unidad::Estratos => DATA_TITLE,
            Unidad::Efi => 0x00A7_8BFA,
        }
    }

    /// Su dibujo vectorial: la losa de un disco, las capas de ESTRATOS o el
    /// chip del arranque.
    fn dibujo(self) -> &'static [bmo_dibujo::Capa] {
        match self {
            Unidad::Datos | Unidad::Personal => &iconos::dibujos::DISCO,
            Unidad::Estratos => &iconos::dibujos::ESTRATOS,
            Unidad::Efi => &iconos::dibujos::CHIP,
        }
    }

    /// Donde se explora, si se puede.
    pub(crate) fn volumen(self) -> Option<fuente::Volumen> {
        match self {
            Unidad::Datos => Some(fuente::Volumen::Datos),
            Unidad::Estratos => Some(fuente::Volumen::Estratos),
            Unidad::Efi => Some(fuente::Volumen::Efi),
            // N1b: `personal ls` todavia no existe; hoy solo se mide.
            Unidad::Personal => None,
        }
    }
}

/// Lo medido de una unidad. `bytes == 0`: no esta montada.
#[derive(Clone, Copy)]
struct Medida {
    bytes: u64,
    libres: Option<u64>,
    /// `INFO_UNIDAD`, `que 2`: el testigo del FSInfo (DATOS, EFI) o donde se
    /// paro N1a (PERSONAL). Ver el ABI.
    extra: u64,
}

const SIN_MEDIR: Medida = Medida { bytes: 0, libres: None, extra: 0 };
static mut MEDIDAS: [Medida; 4] = [SIN_MEDIR; 4];

fn medidas() -> &'static mut [Medida; 4] {
    // SAFETY: solo el hilo del director, como el resto de la ventana.
    unsafe { &mut *addr_of_mut!(MEDIDAS) }
}

fn unidad_info(u: u64) -> Medida {
    let extra = bmo::info(bmo::INFO_UNIDAD | u << 8 | 2 << 16);
    let bytes = bmo::info(bmo::INFO_UNIDAD | u << 8);
    if bytes == 0 {
        return Medida { extra, ..SIN_MEDIR };
    }
    let l = bmo::info(bmo::INFO_UNIDAD | u << 8 | 1 << 16);
    Medida { bytes, libres: (l != bmo::UNIDAD_NO_SE).then_some(l), extra }
}

/// Lo que acompana al motivo de N1a: nada, un numero, o los puertos de una
/// mascara.
pub(crate) enum Detalle {
    Nada,
    Numero(u64),
    Puertos(u64),
}

/// **Por que el disco Personal no esta montado**, de la etapa en que se paro
/// N1a (`etapa | detalle << 8`, ver el ABI). Lo usan la tarjeta y la fila
/// `personal` del informe (`commands/disco.rs`): un solo texto por etapa.
pub(crate) fn motivo_n1a(etapa: u64) -> (&'static str, Detalle) {
    let (e, d) = (etapa & 0xFF, etapa >> 8);
    match e {
        1 => ("sin otro disco SATA; con disco:", Detalle::Puertos(d)),
        2 => ("su puerto no se preparo: ", Detalle::Numero(d)),
        3 => ("no contesto a IDENTIFY; puerto ", Detalle::Numero(d)),
        4 => ("es el MISMO disco de BMO-X", Detalle::Nada),
        5 if d == 1 => ("su tabla GPT no se pudo leer", Detalle::Nada),
        5 => ("no tiene ni GPT ni MBR", Detalle::Nada),
        6 => ("sin particion NTFS; vio ", Detalle::Numero(d)),
        7 => (
            match d {
                1 => "NTFS no monta: el disco no contesto",
                2 => "NTFS no monta: no es NTFS",
                3 => "NTFS no monta: volumen raro",
                4 => "NTFS no monta: demasiado grande",
                _ => "NTFS no monta: otro motivo",
            },
            Detalle::Nada,
        ),
        8 => ("montado", Detalle::Nada),
        _ => ("N1a no corrio: sin controlador AHCI", Detalle::Nada),
    }
}

/// Escribe el motivo de N1a en la tarjeta y devuelve la x.
fn por_que_no(p: &bmo::Pantalla, x: u32, y: u32, etapa: u64) -> u32 {
    let (texto, detalle) = motivo_n1a(etapa);
    let mut x = p.texto(x, y, texto, AMBAR);
    let mut b = [0u8; 10];
    let mut num = |x: u32, v: u64| {
        let n = decimal(v, &mut b);
        p.texto_bytes(x, y, &b[..n], AMBAR)
    };
    match detalle {
        Detalle::Nada => {}
        Detalle::Numero(v) => x = num(x, v),
        Detalle::Puertos(m) => {
            for k in 0..32 {
                if m >> k & 1 == 1 {
                    x = p.texto(x, y, " ", AMBAR);
                    x = num(x, k);
                }
            }
        }
    }
    x
}

/// **Mide todas las unidades.** Al entrar en la vista y con `R`; contar la FAT
/// de DATOS la primera vez son unos megas de lectura, y pintar no los paga.
pub(crate) fn releer() {
    let m = medidas();
    for (k, u) in TODAS.iter().enumerate() {
        m[k] = match u {
            Unidad::Datos => unidad_info(bmo::UNIDAD_DATOS),
            Unidad::Efi => unidad_info(bmo::UNIDAD_EFI),
            Unidad::Personal => unidad_info(bmo::UNIDAD_PERSONAL),
            Unidad::Estratos => {
                if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
                    SIN_MEDIR
                } else {
                    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM);
                    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
                    let usados = bmo::info(bmo::INFO_ES_USADOS);
                    Medida { bytes: bloques * tam, libres: Some(bloques.saturating_sub(usados) * tam), extra: 0 }
                }
            }
        };
    }
}

/// Esta montada? (Para decir por que no se abre.)
pub(crate) fn montada(k: usize) -> bool {
    medidas()[k].bytes != 0
}

// == La geometria: la comparten quien pinta y quien acierta ===================

/// Alto de la cabecera "Dispositivos y unidades" y su raya.
const CABECERA: u32 = bmo::GLIFO_ALTO + 14;
/// El aire entre tarjetas: los `gaps` de Hyprland.
const HUECO: u32 = 12;
/// El icono, y donde empieza el texto a su derecha.
const ICONO: u32 = 48;
const TEXTO: u32 = 12 + ICONO + 12;
/// Lo que pide la linea mas larga: el icono (`TEXTO`) + `Personal (D:) NTFS
/// [solo lectura]` (332 px) y su margen; `111.0 GB disponibles de 111.0 GB`
/// cabe de sobra. Con 280 se salia de la tarjeta (visto en la vista previa).
const TARJETA_MIN: u32 = 356;
const TARJETA_MAX: u32 = 420;
/// Tres lineas: nombre, barra y cifras, y la de los avisos.
const TARJETA_H: u32 = 96;
const RADIO: u32 = 10;

/// Cuantas tarjetas caben por fila.
pub(crate) fn columnas(z: &Zona) -> usize {
    (((z.w + HUECO) / (TARJETA_MIN + HUECO)).max(1) as usize).min(TODAS.len())
}

/// El rectangulo de la tarjeta `k`.
fn tarjeta(z: &Zona, k: usize) -> (u32, u32, u32, u32) {
    let cols = columnas(z) as u32;
    let w = ((z.w.saturating_sub((cols - 1) * HUECO)) / cols).min(TARJETA_MAX);
    let (c, f) = (k as u32 % cols, k as u32 / cols);
    (z.x + c * (w + HUECO), z.y + CABECERA + f * (TARJETA_H + HUECO), w, TARJETA_H)
}

/// **Sobre que tarjeta esta el puntero**, si sobre alguna.
pub(crate) fn en(z: &Zona, px: u32, py: u32) -> Option<usize> {
    (0..TODAS.len()).find(|&k| {
        let (x, y, w, h) = tarjeta(z, k);
        px >= x && px < x + w && py >= y && py < y + h && y + h <= z.y + z.h
    })
}

// == El pintado ===============================================================

/// El cuerpo de una tarjeta: un peldano sobre la ventana.
const TARJETA_FONDO: u32 = 0x001A_2520;
const TARJETA_BORDE: u32 = 0x002C_4038;
/// La elegida: un peldano mas, y el borde en degradado.
const TARJETA_ELEGIDA: u32 = 0x0020_2E28;
/// El `col.active_border` de Hyprland, de un extremo al otro.
const DEGRADADO: (u32, u32) = (0x0033_CCFF, 0x0000_FF99);
/// La barra: el carril y lo usado (azul como Windows; rojo pasado el 90%).
const CARRIL: u32 = 0x0026_332E;
const USADO: u32 = 0x0037_8BF0;
const LLENO: u32 = INK_BAD;
const AMBAR: u32 = 0x00F0_D070;

fn mezcla(a: u32, b: u32, t: u32, de: u32) -> u32 {
    let de = de.max(1);
    let canal = |s: u32| {
        let (x, y) = ((a >> s) & 0xFF, (b >> s) & 0xFF);
        ((x * (de - t) + y * t) / de) & 0xFF
    };
    canal(16) << 16 | canal(8) << 8 | canal(0)
}

/// Lo que falta arriba y abajo en la columna `c` de un redondeado de radio `r`.
fn sangria_col(c: u32, w: u32, r: u32) -> u32 {
    if c < r {
        borde::sangria(r, c)
    } else if c + r >= w {
        borde::sangria(r, w - 1 - c)
    } else {
        0
    }
}

/// **El borde de la elegida**: 2 px de degradado, columna a columna.
fn borde_degradado(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32), cuerpo: u32) {
    for c in 0..w {
        let s = sangria_col(c, w, RADIO);
        p.rect(x + c, y + s, 1, h - 2 * s, mezcla(DEGRADADO.0, DEGRADADO.1, c, w));
    }
    borde::relleno_r(p, x + 2, y + 2, w - 4, h - 4, RADIO - 2, cuerpo);
}

/// Bytes como los dice Windows: `30.3 GB`, `434 GB`, `600 MB` (en potencias
/// de 1024, que es lo que Windows llama GB). Devuelve la x donde acabo.
fn medida(p: &bmo::Pantalla, x: u32, y: u32, bytes: u64, color: u32) -> u32 {
    const MB: u64 = 1 << 20;
    const GB: u64 = 1 << 30;
    let mut b = [0u8; 10];
    if bytes >= GB {
        let decimas = bytes * 10 / GB;
        let n = decimal(decimas / 10, &mut b);
        let mut x = p.texto_bytes(x, y, &b[..n], color);
        if decimas < 1000 {
            x = p.texto(x, y, ".", color);
            let n = decimal(decimas % 10, &mut b);
            x = p.texto_bytes(x, y, &b[..n], color);
        }
        p.texto(x, y, " GB", color)
    } else {
        let n = decimal(bytes / MB, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], color);
        p.texto(x, y, " MB", color)
    }
}

fn pinta_tarjeta(p: &bmo::Pantalla, r: (u32, u32, u32, u32), u: Unidad, m: Medida, elegida: bool) {
    let (x, y, w, h) = r;
    // La sombra, un peldano por debajo del fondo de la ventana.
    borde::relleno_r(p, x + 2, y + 3, w, h, RADIO, 0x000B_100E);
    if elegida {
        borde_degradado(p, r, TARJETA_ELEGIDA);
    } else {
        borde::pastilla(p, r, RADIO, TARJETA_FONDO, TARJETA_BORDE, DATA_BG);
    }
    let montada = m.bytes != 0;
    let luz = if montada { INK_OK } else { INK_BAD };
    // El icono VECTORIAL (`iconos::dibujos`), a 48 px y centrado en el alto,
    // mezclado contra el cuerpo de la tarjeta; el candado encima si no se
    // escribe.
    let fondo = if elegida { TARJETA_ELEGIDA } else { TARJETA_FONDO };
    let (ix, iy) = (x + 12, y + (h - ICONO) / 2);
    let paleta = iconos::paleta(u.color(), luz);
    iconos::vector(p, ix, iy, ICONO, u.dibujo(), &paleta, fondo);
    if u.solo_lectura() {
        iconos::vector(p, ix, iy, ICONO, &iconos::dibujos::CANDADO, &paleta, fondo);
    }

    let tx = x + TEXTO;
    let ty = y + 10;
    let fin = p.texto(tx, ty, u.nombre(), INK);
    let fin = p.texto(fin + bmo::GLIFO_ANCHO, ty, u.sistema(), INK_DIM);
    if u.solo_lectura() {
        let e = "solo lectura";
        let ew = e.len() as u32 * bmo::GLIFO_ANCHO + 12;
        if fin + bmo::GLIFO_ANCHO + ew + 12 <= x + w {
            let bx = fin + bmo::GLIFO_ANCHO;
            borde::pastilla(p, (bx, ty - 1, ew, bmo::GLIFO_ALTO + 2), 4, fondo, AMBAR, fondo);
            p.texto(bx + 6, ty, e, AMBAR);
        }
    }

    let by = ty + bmo::GLIFO_ALTO + 6;
    let bw = w.saturating_sub(TEXTO + 16);
    let bh = 10;
    borde::relleno_r(p, tx, by, bw, bh, 3, CARRIL);
    let ly = by + bh + 6;
    // La tercera linea: lo que hay que saber y no cabe arriba.
    let ny = ly + bmo::GLIFO_ALTO + 2;
    if !montada {
        p.texto(tx, ly, "no montada", INK_DIM);
        if u == Unidad::Personal {
            por_que_no(p, tx, ny, m.extra);
        }
        return;
    }
    // ** EL TESTIGO (A: y EFI): lo que Windows apunto en el FSInfo. Si no
    // cuadra con la cuenta de la FAT por mas del 1%, se dice -- visto el
    // 29-09: A: con 31,7 GB libres y Windows diciendo 30,3.
    if matches!(u, Unidad::Datos | Unidad::Efi) && m.extra != bmo::UNIDAD_NO_SE {
        if let Some(l) = m.libres {
            if l.abs_diff(m.extra) * 100 > m.bytes {
                let x = p.texto(tx, ny, "Windows apunto ", AMBAR);
                let x = medida(p, x, ny, m.extra, AMBAR);
                p.texto(x, ny, " libres (FSInfo)", AMBAR);
            }
        }
    }
    match m.libres {
        Some(l) => {
            let usado = m.bytes.saturating_sub(l);
            let lleno = usado * bw as u64 / m.bytes;
            let color = if usado * 10 > m.bytes * 9 { LLENO } else { USADO };
            if lleno > 0 {
                borde::relleno_r(p, tx, by, (lleno as u32).max(6), bh, 3, color);
            }
            let x = medida(p, tx, ly, l, INK);
            let x = p.texto(x, ly, " disponibles de ", INK_DIM);
            medida(p, x, ly, m.bytes, INK);
        }
        None => {
            let x = medida(p, tx, ly, m.bytes, INK);
            p.texto(x, ly, "; lo libre no se pudo contar", INK_DIM);
        }
    }
}

/// **Pinta la vista**: la cabecera y una tarjeta por unidad.
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona, sel: usize) {
    let fin = p.texto(z.x, z.y + 2, "v ", INK_DIM);
    p.texto(fin, z.y + 2, "Dispositivos y unidades", INK);
    p.rect(z.x, z.y + bmo::GLIFO_ALTO + 7, z.w, 1, TARJETA_BORDE);
    let m = medidas();
    for (k, &u) in TODAS.iter().enumerate() {
        let r = tarjeta(z, k);
        // Lo que no cabe entero no se pinta a medias: se sale de la ventana.
        if r.1 + r.3 > z.y + z.h {
            break;
        }
        pinta_tarjeta(p, r, u, m[k], k == sel);
    }
}
