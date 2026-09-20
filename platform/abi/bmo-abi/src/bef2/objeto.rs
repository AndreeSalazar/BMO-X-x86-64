//! **EL OBJETO (`.bo`)** -- una unidad compilada que todavia no es un programa.
//!
//! [carril]  AMARILLO  lo que aqui pase por bueno lo junta el enlazador; un
//!                     objeto mal leido es un `.bex` que salta a la nada
//! [cuesta]  TAREA     un programa entero que no arranca
//! [riesgo]  UNICO     es el unico lector de objetos del arbol
//!
//! E1 de `docs/plan/PLAN_EL_ENLAZADOR.md` (2026-09-17): BMO-X enlaza
//! ESTATICO -- todo lo que un `.bex` ejecuta viaja dentro de el. Este fichero
//! es el CONTRATO entre un frontend que escribe objetos y la herramienta del
//! anfitrion que los junta (`bmo-enlazar`): un formato, no un cerebro.
//!
//! ## BEF2 (2026-09-19): el objeto es un BEF2 con la bandera `OBJETO`
//!
//! ```text
//!    cabecera    OBJETO, nunca EJECUTABLE; `entrada` se ignora; sin firma
//!    regiones    codigo, constantes, datos (bytes) y ceros (medida): offsets
//!                relativos a CADA region
//!    SIMBOLOS    anexo 0x07: [TablaCadenas][Symbol; n][nombres, cada uno con 0]
//!                `section_idx` = REGION (0 codigo, 1 constantes, 2 datos,
//!                3 ceros) o `SECTION_UNDEFINED` (0xFD): se usa aqui, se
//!                define en otra unidad
//!    ENLACE      anexo 0x08: [Enlace; n], lo que el enlazador resuelve y
//!                NUNCA llega al kernel
//! ```
//!
//! ## Simbolos
//!
//! Un simbolo `Section` es a lo que apunta "algo dentro de esta unidad": uno
//! Local por region con bytes, offset 0. Un `lea [rip+cadena]` se vuelve un
//! `Rel32` contra el ancla de las constantes con el offset como addend, porque
//! donde cae esa region solo se sabe al colocar todas las unidades. Siempre
//! Local: una region de una unidad no es algo que otra unidad pueda nombrar.
//!
//! Un simbolo sin definir tiene que ser Global: un local que nadie define es
//! un fallo de la unidad, no una pregunta para el enlazador.
//!
//! ## Los enlaces (lo que en ELF se llamaria "relocations de objeto")
//!
//! ```text
//!    0  kind     u8    1 Rel32, 2 Abs64, 3 Region
//!    1  donde    u8    REGION donde se parchea (0 codigo, 1 constantes, 2 datos)
//!    2  0        u16
//!    4  offset   u32   dentro de `donde`
//!    8  simbolo  u32   indice en SIMBOLOS -- o la REGION destino, si `kind` = 3
//!   12  0        u32
//!   16  addend   i64
//!
//!    Rel32    escribe S + A - P   (4 bytes: `call`, `lea rip`)
//!    Abs64    escribe S + A       (8 bytes: un puntero a un simbolo)
//!    Region   escribe R + A       (8 bytes: un puntero a una region de ESTA
//!             unidad -- lo que BMO C emite para `p = &tabla`)
//! ```
//!
//! Tres y solo tres. Un GOT es enlazado dinamico, y no existe.
//!
//! ## Lo que este lector rechaza
//!
//! Un objeto mal formado se rechaza AQUI, una vez, con su motivo, para que el
//! enlazador solo razone sobre unidades bien formadas. Resolver entre
//! unidades --un nombre definido dos veces, uno que nadie define-- es
//! pregunta del enlazador, no de este fichero.

use alloc::vec::Vec;

use super::{leer, Region, Vista, ANEXO_ENLACE, ANEXO_SIMBOLOS};
use crate::bef::symbols::{Symbol, SymbolBinding, SymbolKind, TablaCadenas, SECTION_UNDEFINED};

/// Bytes de un enlace.
pub const ENLACE: usize = 24;

/// Que escribe un enlace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Clase {
    /// `S + A - P`, 4 bytes.
    Rel32 = 1,
    /// `S + A`, 8 bytes.
    Abs64 = 2,
    /// `R + A`, 8 bytes: la direccion de una region de esta unidad.
    Region = 3,
}

impl Clase {
    pub const fn de(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Rel32),
            2 => Some(Self::Abs64),
            3 => Some(Self::Region),
            _ => None,
        }
    }

    /// Cuantos bytes parchea.
    pub const fn ancho(self) -> u64 {
        match self {
            Self::Rel32 => 4,
            Self::Abs64 | Self::Region => 8,
        }
    }
}

/// Un enlace de un objeto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enlace {
    pub clase: Clase,
    pub donde: Region,
    pub offset: u32,
    /// Indice en SIMBOLOS, o la region destino (`Region as u32`) si
    /// `clase == Region`.
    pub simbolo: u32,
    pub addend: i64,
}

impl Enlace {
    pub fn a_bytes(&self) -> [u8; ENLACE] {
        let mut b = [0u8; ENLACE];
        b[0] = self.clase as u8;
        b[1] = self.donde as u8;
        b[4..8].copy_from_slice(&self.offset.to_le_bytes());
        b[8..12].copy_from_slice(&self.simbolo.to_le_bytes());
        b[16..24].copy_from_slice(&self.addend.to_le_bytes());
        b
    }

    /// `None` si la clase o la region no existen, o el relleno no es cero.
    pub fn de_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < ENLACE || b[2] != 0 || b[3] != 0 || b[12..16] != [0; 4] {
            return None;
        }
        Some(Self {
            clase: Clase::de(b[0])?,
            donde: Region::de(b[1])?,
            offset: u32::from_le_bytes(b[4..8].try_into().ok()?),
            simbolo: u32::from_le_bytes(b[8..12].try_into().ok()?),
            addend: i64::from_le_bytes(b[16..24].try_into().ok()?),
        })
    }
}

/// Por que un objeto no vale. Cada uno manda a mirar una cosa distinta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// No es un BEF2 valido (`Falta` del juez).
    NotBef(super::Falta),
    /// Un BEF2, pero sin la bandera `OBJETO`.
    NotAnObject,
    /// El anexo SIMBOLOS no se lee (`TablaCadenas`).
    SymbolsMalformed,
    /// Simbolo `n`: su nombre esta fuera de rango, sin cero final o no es UTF-8.
    SymbolName(usize),
    /// Simbolo `n`: `section_idx` no es una region ni `SECTION_UNDEFINED`.
    SymbolSection(usize),
    /// Simbolo `n`: `offset + size` se sale de su region.
    SymbolOutsideSection(usize),
    /// Simbolo `n`: sin definir pero no Global.
    UndefinedLocal(usize),
    /// Simbolo `n`: Weak, o un binding/kind que este contrato no conoce.
    SymbolKindOrBinding(usize),
    /// ENLACE no es un numero entero de entradas, o una tiene relleno sucio.
    RelocsMalformed,
    /// Enlace `n`: una clase que este contrato no acepta.
    RelocKind(usize),
    /// Enlace `n`: `donde` es una region sin bytes (ceros).
    RelocSection(usize),
    /// Enlace `n`: los bytes que parchea se salen de su region.
    RelocOutside(usize),
    /// Enlace `n`: `simbolo` no nombra ningun simbolo (o ninguna region).
    RelocTarget(usize),
}

/// Un simbolo, leido y comprobado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectSymbol<'a> {
    pub name: &'a str,
    /// `None` = no se define aqui.
    pub section: Option<Region>,
    pub offset: u64,
    pub size: u64,
    pub global: bool,
    pub function: bool,
    /// El ancla de una region entera de esta unidad (ver la cabecera).
    pub seccion_ancla: bool,
}

/// Una unidad, leida y comprobada. Prestada de los bytes: no se copia nada.
#[derive(Debug)]
pub struct Object<'a> {
    pub code: &'a [u8],
    pub rodata: &'a [u8],
    pub data: &'a [u8],
    pub bss: u64,
    /// La bandera `QUIERE_PANTALLA` de la unidad: el enlazador la hereda.
    pub quiere_pantalla: bool,
    pub symbols: Vec<ObjectSymbol<'a>>,
    pub enlaces: Vec<Enlace>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

/// Lee `bytes` como un objeto, o dice por que no lo es.
pub fn read(bytes: &[u8]) -> Result<Object<'_>, Fault> {
    let v: Vista<'_> = leer(bytes).map_err(Fault::NotBef)?;
    if !v.es_objeto() {
        return Err(Fault::NotAnObject);
    }
    let code = v.region(Region::Codigo);
    let rodata = v.region(Region::Constantes);
    let data = v.region(Region::Datos);
    let bss = v.ceros as u64;
    // Cuantos bytes puede abarcar un simbolo en cada region.
    let extent = |r: Region| -> u64 {
        match r {
            Region::Codigo => code.len() as u64,
            Region::Constantes => rodata.len() as u64,
            Region::Datos => data.len() as u64,
            Region::Ceros => bss,
        }
    };

    // -- Simbolos. --
    let mut symbols = Vec::new();
    if let Some(raw) = v.anexo(ANEXO_SIMBOLOS) {
        let (n, strings_at) = TablaCadenas::leer(raw, Symbol::SIZE).ok_or(Fault::SymbolsMalformed)?;
        let strings = &raw[strings_at..];
        for i in 0..n {
            let at = TablaCadenas::SIZE + i * Symbol::SIZE;
            // Campo a campo: los bytes del anexo no estan alineados.
            let name_off = u32_at(raw, at).ok_or(Fault::SymbolsMalformed)? as usize;
            let offset = u64_at(raw, at + 8).ok_or(Fault::SymbolsMalformed)?;
            let size = u64_at(raw, at + 16).ok_or(Fault::SymbolsMalformed)?;
            let (kind, binding, section_idx) = (raw[at + 24], raw[at + 25], raw[at + 27]);

            let name = strings
                .get(name_off..)
                .and_then(|rest| rest.iter().position(|&b| b == 0).map(|z| &rest[..z]))
                .and_then(|n| core::str::from_utf8(n).ok())
                .filter(|n| !n.is_empty())
                .ok_or(Fault::SymbolName(i))?;

            let global = match binding {
                b if b == SymbolBinding::Local as u8 => false,
                b if b == SymbolBinding::Global as u8 => true,
                _ => return Err(Fault::SymbolKindOrBinding(i)),
            };
            let function = match kind {
                k if k == SymbolKind::Function as u8 => true,
                k if k == SymbolKind::Object as u8 => false,
                // Un ancla de region: siempre Local, y nombra una region de
                // ESTA unidad, asi que nunca puede estar sin definir.
                k if k == SymbolKind::Section as u8 => {
                    if global || section_idx == SECTION_UNDEFINED {
                        return Err(Fault::SymbolKindOrBinding(i));
                    }
                    false
                }
                _ => return Err(Fault::SymbolKindOrBinding(i)),
            };

            let section = if section_idx == SECTION_UNDEFINED {
                if !global {
                    return Err(Fault::UndefinedLocal(i));
                }
                None
            } else {
                let r = Region::de(section_idx).ok_or(Fault::SymbolSection(i))?;
                if offset.checked_add(size).map_or(true, |f| f > extent(r)) {
                    return Err(Fault::SymbolOutsideSection(i));
                }
                Some(r)
            };
            symbols.push(ObjectSymbol {
                name,
                section,
                offset,
                size,
                global,
                function,
                seccion_ancla: kind == SymbolKind::Section as u8,
            });
        }
    }

    // -- Enlaces. --
    let mut enlaces = Vec::new();
    let raw = v.anexo(ANEXO_ENLACE).unwrap_or(&[]);
    if raw.len() % ENLACE != 0 {
        return Err(Fault::RelocsMalformed);
    }
    for i in 0..raw.len() / ENLACE {
        let b = &raw[i * ENLACE..];
        if Clase::de(b[0]).is_none() {
            return Err(Fault::RelocKind(i));
        }
        let e = Enlace::de_bytes(b).ok_or(Fault::RelocsMalformed)?;
        if matches!(e.donde, Region::Ceros) {
            return Err(Fault::RelocSection(i));
        }
        if (e.offset as u64)
            .checked_add(e.clase.ancho())
            .map_or(true, |f| f > extent(e.donde))
        {
            return Err(Fault::RelocOutside(i));
        }
        let target_ok = match e.clase {
            Clase::Region => e.simbolo <= 3 && Region::de(e.simbolo as u8).is_some(),
            _ => (e.simbolo as usize) < symbols.len(),
        };
        if !target_ok {
            return Err(Fault::RelocTarget(i));
        }
        enlaces.push(e);
    }

    Ok(Object {
        code,
        rodata,
        data,
        bss,
        quiere_pantalla: v.quiere_pantalla(),
        symbols,
        enlaces,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::bef::symbols::{name_hash, SymbolVisibility};
    use crate::bef::symbols::en_bytes as simbolos_en_bytes;
    use alloc::vec;

    fn simbolo(nombre: &str, cadenas: &mut Vec<u8>, kind: SymbolKind, local: bool, sec: u8, off: u64, size: u64) -> Symbol {
        let name_off = cadenas.len() as u32;
        cadenas.extend_from_slice(nombre.as_bytes());
        cadenas.push(0);
        Symbol {
            name_off,
            name_hash: name_hash(nombre),
            virt_addr: off,
            size,
            kind: kind as u8,
            binding: if local { SymbolBinding::Local } else { SymbolBinding::Global } as u8,
            visibility: SymbolVisibility::Default as u8,
            section_idx: sec,
            _reserved: 0,
        }
    }

    /// Una unidad: `f` llama a `g` (de otra unidad) y guarda un puntero a
    /// sus constantes en sus datos.
    fn unidad(enlaces: &[Enlace]) -> Vec<u8> {
        let mut cadenas = Vec::new();
        let entradas = vec![
            simbolo(".code", &mut cadenas, SymbolKind::Section, true, 0, 0, 16),
            simbolo(".rodata", &mut cadenas, SymbolKind::Section, true, 1, 0, 5),
            simbolo("f", &mut cadenas, SymbolKind::Function, false, 0, 0, 16),
            simbolo("g", &mut cadenas, SymbolKind::Function, false, SECTION_UNDEFINED, 0, 0),
        ];
        let mut e = super::super::Escritor::objeto();
        e.codigo(vec![0xC3; 16])
            .constantes(b"hola\0".to_vec())
            .datos(vec![0u8; 8])
            .anexo(ANEXO_SIMBOLOS, simbolos_en_bytes(&entradas, &cadenas));
        let mut raw = Vec::new();
        for l in enlaces {
            raw.extend_from_slice(&l.a_bytes());
        }
        if !raw.is_empty() {
            e.anexo(ANEXO_ENLACE, raw);
        }
        e.construir().unwrap()
    }

    fn buenos() -> Vec<Enlace> {
        vec![
            Enlace { clase: Clase::Rel32, donde: Region::Codigo, offset: 1, simbolo: 3, addend: -4 },
            Enlace { clase: Clase::Region, donde: Region::Datos, offset: 0, simbolo: Region::Constantes as u32, addend: 0 },
        ]
    }

    #[test]
    fn un_objeto_bueno_se_lee_entero() {
        let img = unidad(&buenos());
        let o = read(&img).expect("vale");
        assert_eq!(o.code.len(), 16);
        assert_eq!(o.rodata, b"hola\0");
        assert_eq!(o.symbols.len(), 4);
        assert_eq!(o.symbols[3].section, None);
        assert!(o.symbols[3].global);
        assert!(o.symbols[1].seccion_ancla);
        assert_eq!(o.enlaces, buenos());
    }

    #[test]
    fn un_ejecutable_no_es_un_objeto() {
        let mut e = super::super::Escritor::ejecutable();
        e.codigo(vec![0xC3; 16]);
        assert_eq!(read(&e.construir().unwrap()).unwrap_err(), Fault::NotAnObject);
    }

    #[test]
    fn el_lector_caza_cada_enlace_malo() {
        let malo = |l: Enlace| read(&unidad(&[l])).unwrap_err();
        // fuera de su region
        assert_eq!(
            malo(Enlace { clase: Clase::Rel32, donde: Region::Codigo, offset: 14, simbolo: 3, addend: 0 }),
            Fault::RelocOutside(0)
        );
        // parchear los ceros
        assert_eq!(
            malo(Enlace { clase: Clase::Abs64, donde: Region::Ceros, offset: 0, simbolo: 3, addend: 0 }),
            Fault::RelocSection(0)
        );
        // un simbolo que no existe
        assert_eq!(
            malo(Enlace { clase: Clase::Abs64, donde: Region::Datos, offset: 0, simbolo: 9, addend: 0 }),
            Fault::RelocTarget(0)
        );
        // una region que no existe
        assert_eq!(
            malo(Enlace { clase: Clase::Region, donde: Region::Datos, offset: 0, simbolo: 7, addend: 0 }),
            Fault::RelocTarget(0)
        );
        // una clase inventada
        let mut img = unidad(&buenos());
        let v = leer(&img).unwrap();
        let a = v.anexos().find(|a| a.tipo == ANEXO_ENLACE).unwrap();
        img[a.tramo.offset as usize] = 9;
        assert_eq!(read(&img).unwrap_err(), Fault::RelocKind(0));
    }

    #[test]
    fn un_local_sin_definir_no_vale() {
        let mut cadenas = Vec::new();
        let entradas = vec![simbolo("x", &mut cadenas, SymbolKind::Function, true, SECTION_UNDEFINED, 0, 0)];
        let mut e = super::super::Escritor::objeto();
        e.codigo(vec![0xC3]).anexo(ANEXO_SIMBOLOS, simbolos_en_bytes(&entradas, &cadenas));
        assert_eq!(read(&e.construir().unwrap()).unwrap_err(), Fault::UndefinedLocal(0));
    }

    #[test]
    fn un_enlace_va_y_vuelve_en_bytes() {
        for l in buenos() {
            assert_eq!(Enlace::de_bytes(&l.a_bytes()), Some(l));
        }
        let mut b = buenos()[0].a_bytes();
        b[2] = 1;
        assert_eq!(Enlace::de_bytes(&b), None);
    }
}
