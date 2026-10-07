//! # **EL JUEZ DE LA CACHE**: que tipo de memoria tiene DE VERDAD una pagina
//!
//! capa: puro -- numeros que entran y numeros que salen, probado en el anfitrion
//!
//! [carril]  VERDE     lee tres numeros del hardware y dice que pasa con la cache
//! [cuesta]  MAQUINA -- por herencia: `mm/vmm/directa.rs` reescribe una entrada
//!           de la tabla de paginas con lo que esto conteste. Si se equivoca,
//!           la caja negra vuelve a guardar lo ultimo SOLO en la cache
//!
//! # La pregunta del propietario (2026-10-07)
//!
//! > *"puedes mejorar MAS en que mi BMO-X lea en tiempo real que pasa con el
//! > cache en tiempo real?"*
//!
//! La caja negra (`cabina/caida.rs`) vivia en memoria **WB** (write-back):
//! cada byte se queda en la cache y la RAM se entera "mas tarde". Un reinicio
//! de golpe borra la cache sin escribirla, y lo ultimo se perdia. V7 lo
//! parcheo con `clflush` (cada linea, cada tick); V8 lo quita de raiz: la
//! pagina pasa a **WT** (write-through), y **cada escritura va a la RAM en el
//! mismo instante**. La cache ya no guarda nunca la unica copia.
//!
//! # Quien decide el tipo de una pagina: DOS tablas, no una
//!
//! ```text
//!    MTRR   (la placa, el firmware)   por rangos de fisica: "esto es RAM WB"
//!    PAT    (el kernel)               por pagina: 3 bits de la entrada eligen
//!                                     una de las 8 casillas del MSR 0x277
//!    efectivo = combinar(MTRR, PAT)   la tabla 11-7 de Intel
//! ```
//!
//! Por eso esto NO dice "WT" porque se pidio WT: lee el PAT de verdad, lee los
//! MTRR de verdad y los combina. **Si no lo sabe, dice que no lo sabe**
//! (`None`), y el kernel sigue con el `clflush` de V7.
//!
//! # La regla: ninguna funcion de aqui contesta SI por defecto
//!
//! Copiada de `bmo-mmio-juicio`. Un MTRR con una mascara rara, un rango que
//! tapa media pagina, una combinacion que no esta en la tabla: `None`.

#![cfg_attr(not(test), no_std)]

/// **Los tipos de memoria**, con el numero que usan el PAT y los MTRR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Tipo {
    /// Sin cache: cada lectura y cada escritura van al bus.
    Uc = 0,
    /// Escrituras juntadas en un bufer: rapidas, y SE PIERDEN en un reinicio.
    Wc = 1,
    /// Escritura directa: la cache lee, pero cada escritura va a la RAM ya.
    Wt = 4,
    /// Protegida contra escritura (lee de la cache, escribe al bus).
    Wp = 5,
    /// Write-back: la RAM se entera cuando la linea sale de la cache.
    Wb = 6,
    /// UC- (solo en el PAT): sin cache, salvo que el MTRR diga WC.
    UcMenos = 7,
}

impl Tipo {
    /// El byte de un MSR, si es un tipo que existe.
    pub fn de(byte: u8) -> Option<Tipo> {
        Some(match byte {
            0 => Tipo::Uc,
            1 => Tipo::Wc,
            4 => Tipo::Wt,
            5 => Tipo::Wp,
            6 => Tipo::Wb,
            7 => Tipo::UcMenos,
            _ => return None,
        })
    }

    /// **Una escritura en este tipo, esta en la RAM al acabar la instruccion?**
    /// Es la pregunta de la caja negra. WC dice que NO a proposito: junta
    /// escrituras en un bufer que un reinicio tambien borra.
    pub fn escribe_directo(self) -> bool {
        matches!(self, Tipo::Uc | Tipo::UcMenos | Tipo::Wt)
    }

    /// El nombre corto, para el DIARIO.
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Uc => "UC",
            Tipo::Wc => "WC",
            Tipo::Wt => "WT",
            Tipo::Wp => "WP",
            Tipo::Wb => "WB",
            Tipo::UcMenos => "UC-",
        }
    }
}

/// El PAT con el que arranca cualquier x86-64 (y el que se usa si el CPU no
/// tiene PAT: los bits PWT y PCD solos eligen las casillas 0..3 de este).
pub const PAT_DE_ARRANQUE: u64 = 0x0007_0406_0007_0406;

/// Bits de una entrada de pagina de 2 MiB (una PDE con PS).
pub const PRESENTE: u64 = 1 << 0;
pub const PWT: u64 = 1 << 3;
pub const PCD: u64 = 1 << 4;
pub const GRANDE: u64 = 1 << 7;
/// En una pagina de 2 MiB el bit alto del indice de PAT es el 12, no el 7
/// (el 7 es `GRANDE`).
pub const PAT_2M: u64 = 1 << 12;

/// La casilla `i` (0..8) del MSR del PAT.
pub fn casilla(pat: u64, i: u8) -> Option<Tipo> {
    if i > 7 {
        return None;
    }
    let b = (pat >> (8 * i as u64)) as u8;
    if b & 0xF8 != 0 {
        return None; // bits reservados: este PAT no es de fiar
    }
    Tipo::de(b)
}

/// Que casilla del PAT elige una PDE de 2 MiB: `PAT<<2 | PCD<<1 | PWT`.
pub fn indice_2m(pde: u64) -> u8 {
    ((pde & PAT_2M != 0) as u8) << 2 | ((pde & PCD != 0) as u8) << 1 | (pde & PWT != 0) as u8
}

/// El tipo que pide el PAT para esta PDE de 2 MiB.
pub fn tipo_pat_2m(pde: u64, pat: u64) -> Option<Tipo> {
    casilla(pat, indice_2m(pde))
}

/// Por que una PDE no se puede pasar a escritura directa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NoDirecta {
    /// La entrada no esta presente: no hay pagina que cambiar.
    NoPresente = 1,
    /// No es una pagina de 2 MiB (es una tabla de 4 KiB debajo).
    NoEsDeDosMegas = 2,
    /// Ninguna casilla del PAT es WT.
    PatSinWt = 3,
}

/// **La misma PDE, eligiendo la casilla WT del PAT.** Solo cambia los tres
/// bits del indice; la direccion, los permisos y el NX se quedan como estaban.
/// Se queda con la casilla WT mas baja (en el PAT de arranque y en el que
/// deja `s1_cpu`, la 1: solo `PWT`).
pub fn pde_directa(pde: u64, pat: u64) -> Result<u64, NoDirecta> {
    if pde & PRESENTE == 0 {
        return Err(NoDirecta::NoPresente);
    }
    if pde & GRANDE == 0 {
        return Err(NoDirecta::NoEsDeDosMegas);
    }
    let i = (0..8u8)
        .find(|&i| casilla(pat, i) == Some(Tipo::Wt))
        .ok_or(NoDirecta::PatSinWt)?;
    let limpia = pde & !(PWT | PCD | PAT_2M);
    let bits = if i & 1 != 0 { PWT } else { 0 }
        | if i & 2 != 0 { PCD } else { 0 }
        | if i & 4 != 0 { PAT_2M } else { 0 };
    Ok(limpia | bits)
}

/// MSR `IA32_MTRR_DEF_TYPE`: el bit que enciende los MTRR.
pub const MTRR_ENCENDIDOS: u64 = 1 << 11;
/// MSR `IA32_MTRR_PHYSMASKn`: el bit de "este rango vale".
pub const MTRR_VALE: u64 = 1 << 11;

/// **El tipo que dicen los MTRR para TODO `[ini, fin)`**, o `None` si no es
/// uno solo o no se sabe.
///
/// `def` es `IA32_MTRR_DEF_TYPE`; `vars` los pares `(PHYSBASEn, PHYSMASKn)`;
/// `ancho` los bits de direccion fisica del CPU (CPUID 0x80000008). Por debajo
/// de 1 MiB mandan los MTRR fijos, que esto no lee: `None`.
pub fn tipo_mtrr(def: u64, vars: &[(u64, u64)], ancho: u8, ini: u64, fin: u64) -> Option<Tipo> {
    if def & MTRR_ENCENDIDOS == 0 {
        return Some(Tipo::Uc);
    }
    if ini < 0x10_0000 || fin <= ini || !(12..=52).contains(&ancho) {
        return None;
    }
    // Los bits de direccion que existen en este CPU (12..ancho).
    let dir = 0x000F_FFFF_FFFF_F000u64 & ((1u64 << ancho) - 1);
    let mut visto: Option<Tipo> = None;
    for &(base, mascara) in vars {
        if mascara & MTRR_VALE == 0 {
            continue;
        }
        // La mascara con los bits de encima del ancho a uno: si es de la
        // forma 1..10..0, el rango es [desde, desde + tam) con tam potencia de 2.
        let m = (mascara & dir) | !((1u64 << ancho) - 1);
        let tam = (!m).wrapping_add(1);
        if !tam.is_power_of_two() || tam < 0x1000 {
            return None; // una mascara que no es un rango seguido
        }
        let desde = base & m & dir;
        let hasta = desde + tam;
        if desde >= fin || ini >= hasta {
            continue; // no toca
        }
        if !(desde <= ini && fin <= hasta) {
            return None; // tapa una parte: la pagina no tiene UN tipo
        }
        let t = de_mtrr(base as u8)?;
        visto = Some(match visto {
            None => t,
            Some(v) if v == t => v,
            // Las dos reglas de solape que da Intel; el resto, sin definir.
            Some(v) if v == Tipo::Uc || t == Tipo::Uc => Tipo::Uc,
            Some(Tipo::Wt) | Some(Tipo::Wb) if matches!(t, Tipo::Wt | Tipo::Wb) => Tipo::Wt,
            Some(_) => return None,
        });
    }
    match visto {
        Some(t) => Some(t),
        None => de_mtrr(def as u8),
    }
}

/// Un tipo de MTRR: los mismos numeros que el PAT, pero el 7 (UC-) no existe.
fn de_mtrr(b: u8) -> Option<Tipo> {
    Tipo::de(b).filter(|t| *t != Tipo::UcMenos)
}

/// **MTRR x PAT = el tipo que usa el CPU.** Solo las casillas que estan
/// claras en la tabla de Intel (11-7) y en la de AMD (7-11); el resto, `None`.
pub fn efectivo(mtrr: Tipo, pat: Tipo) -> Option<Tipo> {
    use Tipo::*;
    Some(match (mtrr, pat) {
        (_, Uc) => Uc,
        (_, Wc) => Wc,
        (Wc, UcMenos) => Wc,
        (_, UcMenos) => Uc,
        (Uc, Wt | Wb | Wp) => Uc,
        (Wb, t) => t,
        (Wt, Wt | Wb) => Wt,
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas;
