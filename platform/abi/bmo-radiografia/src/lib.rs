//! **VER POR DENTRO un `.bex` con los ojos del kernel** (15-10).
//!
//! > El propietario: *"click derecho y ver todo lo que hay adentro pero para
//! > leer que son cada uno y que cumplen, es el lente de mi Kernel"*.
//!
//! No es otro lector: son las TRES piezas con las que el kernel admite un
//! programa, en el mismo orden --
//!
//! ```text
//!    la puerta      bmo-bex-gate::revisar: la cabecera, las cuatro regiones,
//!                   la tabla de anexos; o el NO con su nombre
//!    los papeles    bmo-carga-juicio::Tabla: lo que el programa declara
//!                   (memoria, pantalla, audio, instrucciones de CPU...)
//!    la declaracion el MANIFIESTO (texto) que escribe el compilador: los
//!                   modulos, los permisos (pantalla, sonido...) y lo que hace
//!    la lente       bmo-lente-x86::recorre: el codigo leido instruccion a
//!                   instruccion desde la entrada y desde cada puntero a
//!                   funcion (las relocaciones), contra lo declarado
//! ```
//!
//! -- sobre el fichero entero en memoria, en vez de sobre las paginas ya
//! cargadas. Si aqui dice SI, el kernel dice SI; si el kernel cambia sus
//! reglas, esto las ve el mismo dia, porque son las suyas.

#![no_std]
#![forbid(unsafe_code)]

pub use bmo_bex_gate::{Cual, Falta, Region};
/// La lente, para quien quiera leer (y decir en castellano) el codigo.
pub use bmo_lente_x86 as lente;
pub use bmo_lente_x86::recorre::Recorrido;

/// **Un `.exe` de Windows por dentro** (LX1): lo que PROTON-X tiene que
/// saber de un juego antes de correrlo.
pub mod pe;

use bmo_bex_gate as gate;
use bmo_carga_juicio as juicio;

/// Cuantos anexos y requisitos se cuentan (los demas, se cuentan sin detalle).
pub const ANEXOS_MAX: usize = 16;
pub const REQUISITOS_MAX: usize = 16;

/// Un anexo: su tipo y cuantos bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anexo {
    pub tipo: u8,
    pub bytes: u64,
}

/// **Una katana** (la mesa de INTI): una regla, y donde esta en el codigo el
/// bloque de trampa que la cumple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Katana {
    /// El codigo que devuelve al atrapar: `1001` es `E1001`.
    pub codigo: u32,
    /// El bloque de trampa, dentro de la region de codigo.
    pub offset: u32,
    pub longitud: u32,
    /// El bloque cae entero dentro del codigo (lo que se puede comprobar).
    pub dentro: bool,
}

/// Cuantas katanas se cuentan con detalle.
pub const KATANAS_MAX: usize = 16;

/// Un requisito declarado: su clase, su cantidad y si es obligatorio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Requisito {
    pub clase: u16,
    pub cantidad: u64,
    pub obligatorio: bool,
}

/// **Lo que se ve dentro.**
#[derive(Clone, Copy, Debug)]
pub struct Radiografia {
    /// El NO de la puerta, si lo hubo: entonces lo demas esta vacio.
    pub falta: Option<Falta>,
    pub bytes: u64,
    pub entrada: u64,
    pub quiere_pantalla: bool,
    pub xcr0: u64,
    /// Las cuatro regiones, en el orden de `Cual::TODAS` (`None` = no la trae).
    pub regiones: [Option<Region>; 4],
    pub anexos: [Anexo; ANEXOS_MAX],
    pub cuantos_anexos: usize,
    pub requisitos: [Requisito; REQUISITOS_MAX],
    pub cuantos_requisitos: usize,
    /// Las instrucciones de CPU que DECLARA (la mascara de A6).
    pub declara: u64,
    pub relocs: usize,
    /// El MANIFIESTO (texto: lo que el compilador declara que es el programa
    /// -- sus modulos, sus permisos, sus funciones), como `(offset, bytes)`
    /// en el fichero. TITAN++ e INTI no escriben un `.bex` sin el.
    pub manifiesto: Option<(u64, u64)>,
    /// La MESA DE KATANAS (`.ibx` de INTI): cada regla con su trampa. `None`
    /// si no la trae; `Some(false)` si la trae y no se entiende.
    pub katanas_ok: Option<bool>,
    pub katanas: [Katana; KATANAS_MAX],
    pub cuantas_katanas: usize,
    /// Lo que la lente leyo. `None` si no hay codigo, o si el mapa no basta.
    pub lente: Option<Recorrido>,
}

impl Radiografia {
    const VACIA: Radiografia = Radiografia {
        falta: None,
        bytes: 0,
        entrada: 0,
        quiere_pantalla: false,
        xcr0: 0,
        regiones: [None; 4],
        anexos: [Anexo { tipo: 0, bytes: 0 }; ANEXOS_MAX],
        cuantos_anexos: 0,
        requisitos: [Requisito { clase: 0, cantidad: 0, obligatorio: false }; REQUISITOS_MAX],
        cuantos_requisitos: 0,
        declara: 0,
        relocs: 0,
        manifiesto: None,
        katanas_ok: None,
        katanas: [Katana { codigo: 0, offset: 0, longitud: 0, dentro: false }; KATANAS_MAX],
        cuantas_katanas: 0,
        lente: None,
    };

    /// El kernel lo admitiria: la puerta dijo que si y la lente no dice NO
    /// (una instruccion no declarada que sin ella falla).
    pub fn admisible(&self) -> bool {
        self.falta.is_none() && self.lente.map_or(true, |l| l.no_declarada.is_none())
    }
}

fn region_bytes<'a>(bytes: &'a [u8], r: &Region) -> &'a [u8] {
    let ini = r.file_offset as usize;
    let fin = ini.saturating_add(r.file_size as usize).min(bytes.len());
    bytes.get(ini..fin).unwrap_or(&[])
}

/// **Radiografia el `.bex` entero `bytes`.** `visto` y `pila` son los de la
/// lente (al menos `codigo / 8` y lo pendiente): sin sitio, la lente no se
/// pasa y se dice (`lente: None` con codigo).
pub fn radiografiar(bytes: &[u8], visto: &mut [u8], pila: &mut [u32]) -> Radiografia {
    let mut r = Radiografia::VACIA;
    r.bytes = bytes.len() as u64;
    let rev = match gate::revisar(bytes, bytes.len()) {
        Ok(v) => v,
        Err(f) => {
            r.falta = Some(f);
            return r;
        }
    };
    r.entrada = rev.entrada();
    r.quiere_pantalla = rev.quiere_pantalla();
    r.xcr0 = rev.xcr0();
    for (i, c) in Cual::TODAS.iter().enumerate() {
        r.regiones[i] = rev.region(*c);
    }
    for a in rev.anexos() {
        if r.cuantos_anexos < ANEXOS_MAX {
            r.anexos[r.cuantos_anexos] = Anexo { tipo: a.tipo, bytes: a.file_size };
        }
        r.cuantos_anexos += 1;
    }
    r.manifiesto = rev.anexo(gate::ANEXO_MANIFIESTO).map(|a| (a.file_offset, a.file_size));
    // La mesa de katanas: la forma de `bmo_abi::bef::katanas` (BKAT, cuantas,
    // version 1; 16 B por katana: codigo, offset, longitud, cero), leida a
    // mano como lee el lanzador el icono -- sin `alloc`, contra el contrato.
    if let Some(a) = rev.anexo(gate::ANEXO_KATANAS) {
        let t = bytes.get(a.file_offset as usize..(a.file_offset + a.file_size) as usize).unwrap_or(&[]);
        let codigo_len = r.regiones[0].map_or(0, |c| c.file_size);
        r.katanas_ok = Some(false);
        if t.len() >= 16 && u32_en(t, 0) == Some(u32::from_le_bytes(*b"BKAT")) {
            let n = u32_en(t, 4).unwrap_or(0) as usize;
            if n <= 4096 && t.len() >= 16 + n * 16 {
                r.katanas_ok = Some(true);
                r.cuantas_katanas = n;
                for i in 0..n.min(KATANAS_MAX) {
                    let e = 16 + i * 16;
                    let (codigo, offset, longitud) = (u32_en(t, e).unwrap_or(0), u32_en(t, e + 4).unwrap_or(0), u32_en(t, e + 8).unwrap_or(0));
                    let dentro = offset as u64 + longitud as u64 <= codigo_len;
                    r.katanas[i] = Katana { codigo, offset, longitud, dentro };
                }
            }
        }
    }
    // Los papeles: lo que declara (la mascara de instrucciones, como el kernel).
    if let Some(a) = rev.anexo(gate::ANEXO_REQUISITOS) {
        let trozo = bytes.get(a.file_offset as usize..(a.file_offset + a.file_size) as usize).unwrap_or(&[]);
        if let Some(t) = juicio::Tabla::abrir(trozo) {
            for q in t.iter() {
                if q.es_obligatorio() && q.clase == juicio::CLASE_INSTRUCCIONES {
                    r.declara |= q.cantidad;
                }
                if r.cuantos_requisitos < REQUISITOS_MAX {
                    r.requisitos[r.cuantos_requisitos] = Requisito { clase: q.clase, cantidad: q.cantidad, obligatorio: q.es_obligatorio() };
                }
                r.cuantos_requisitos += 1;
            }
        }
    }
    // Las relocaciones: cada una que apunta al codigo es un puntero a funcion,
    // y la lente empieza tambien desde ahi (como `task/admitir.rs`).
    let relocs: &[u8] = match rev.anexo(gate::ANEXO_RELOCS) {
        Some(a) => bytes.get(a.file_offset as usize..(a.file_offset + a.file_size) as usize).unwrap_or(&[]),
        None => &[],
    };
    r.relocs = relocs.len() / gate::RELOC_SIZE;
    let Some(codigo) = r.regiones[0].map(|c| region_bytes(bytes, &c)) else {
        return r;
    };
    if codigo.is_empty() || visto.len() < codigo.len().div_ceil(8) {
        return r;
    }
    let semillas = relocs.chunks_exact(gate::RELOC_SIZE).filter_map(|e| {
        let destino = e[gate::reloc::DESTINO];
        let relleno = u16::from_le_bytes([e[gate::reloc::RELLENO], e[gate::reloc::RELLENO + 1]]);
        let mut a = [0u8; 8];
        a.copy_from_slice(&e[gate::reloc::ADDEND..gate::reloc::ADDEND + 8]);
        let addend = i64::from_le_bytes(a);
        (relleno == 0 && destino == 0 && addend >= 0).then_some(addend as u64)
    });
    let declara = r.declara;
    let ventana = |en: usize, buf: &mut [u8; bmo_lente_x86::LARGO_MAX]| -> usize {
        let n = bmo_lente_x86::LARGO_MAX.min(codigo.len() - en);
        buf[..n].copy_from_slice(&codigo[en..en + n]);
        n
    };
    r.lente = Some(bmo_lente_x86::recorre::recorre(
        codigo.len(),
        r.entrada as usize,
        semillas,
        ventana,
        |n| juicio::instrucciones::por_nombre(n).is_some_and(|e| declara & e.bit != 0),
        visto,
        pila,
    ));
    r
}

fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// **La regla de INTI de un codigo de katana**, en castellano (las de
/// `toolchain/lang/inti/REGLAS.md`).
pub fn nombre_regla(codigo: u32) -> &'static str {
    match codigo {
        1001 => "desbordar un entero ATRAPA (y el cociente que no cabe)",
        1002 => "un indice fuera de la lista ATRAPA",
        1003 => "dividir entre cero ATRAPA",
        1005 => "un puntero colgante o liberado no se usa",
        1012 => "convertir un flotante a entero fuera de rango ATRAPA",
        _ => "una regla que esta version no sabe nombrar",
    }
}

/// El nombre de un anexo, para decirlo.
pub fn nombre_anexo(tipo: u8) -> &'static str {
    match tipo {
        gate::ANEXO_RELOCS => "relocaciones",
        gate::ANEXO_FIRMA => "firma",
        gate::ANEXO_REQUISITOS => "requisitos (lo que pide)",
        gate::ANEXO_RECURSOS => "recursos (icono, datos)",
        gate::ANEXO_MANIFIESTO => "manifiesto",
        gate::ANEXO_KATANAS => "katanas (la mesa de INTI)",
        gate::ANEXO_SIMBOLOS => "simbolos",
        gate::ANEXO_ENLACE => "enlace",
        gate::ANEXO_SOMBREADORES => "sombreadores (la 3060)",
        _ => "desconocido (se salta)",
    }
}

/// El nombre de una clase de requisito.
pub fn nombre_clase(clase: u16) -> &'static str {
    match clase {
        juicio::CLASE_MEMORIA => "memoria",
        juicio::CLASE_RECURSOS => "recursos",
        juicio::CLASE_PANTALLA => "pantalla",
        juicio::CLASE_AUDIO => "audio",
        juicio::CLASE_ENTRADA => "entrada (teclado y raton)",
        juicio::CLASE_CPU => "cpu",
        juicio::CLASE_PROCESOS => "procesos",
        juicio::CLASE_MONTON => "monton",
        juicio::CLASE_INSTRUCCIONES => "instrucciones de cpu",
        _ => "otra",
    }
}

/// Los nombres de las instrucciones de una mascara (`popcnt`, `avx2`...).
pub fn nombres_instrucciones(mascara: u64) -> impl Iterator<Item = &'static str> {
    juicio::instrucciones::nombres(mascara)
}
