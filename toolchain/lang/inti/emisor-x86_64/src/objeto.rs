//! **El OBJETO de INTI** (`.bo`): la unidad que `bmo-enlazar` junta con las
//! de C y C++ (2026-09-20).
//!
//! ## Por que existe
//!
//! Eddi: *"INTI, C y C++, los tres para poder tener apps basicas"*. Hasta hoy
//! INTI era una isla: compilaba a `.ibx` y una llamada a un nombre que no
//! estuviera en el modulo se quedaba sin destino -- `lib.rs` lo decia con esas
//! palabras: *"hace falta enlazado para arreglarlo de verdad"*. Un programa de
//! C no podia llamar a una funcion de INTI, ni al reves.
//!
//! Con esto, un modulo de INTI sale como BEF2 con la bandera `OBJETO`: sus
//! funciones son simbolos globales, sus tablas congeladas y su monton son
//! regiones con enlace, y cada llamada sin destino es un simbolo INDEFINIDO
//! con un `Rel32` que resuelve el enlazador. Es el mismo contrato que escribe
//! `lang/c/emisor-x86_64/src/codegen/objeto.rs`, y por eso los tres se
//! enlazan: **no hay un formato de INTI y otro de C; hay `.bo`.**
//!
//! ## La convencion es la misma, y eso es lo que lo hace posible
//!
//! Los seis registros de argumento, `rax`, los preservados y la pila de
//! `types/convention.rs` los IMPORTAN los tres emisores. Un `entero64` de INTI
//! es un `long long` de C en el mismo registro. Lo que NO cruza son los
//! objetos de INTI (`texto`, `lista`, `tabla`): llevan cabecera propia y un
//! `char*` no la tiene. La frontera son escalares y direcciones `crudo`.
//!
//! ## Lo que un objeto NO lleva, y hay que decirlo
//!
//! - **Los requisitos** (`necesita`) y las **katanas**: el enlazador de hoy no
//!   los junta entre unidades. Un modulo de INTI que necesite pantalla y se
//!   enlace desde C no lo declara en el `.bex` final. Es trabajo del
//!   enlazador, no de este fichero, y esta apuntado en `PLAN_EL_ENLAZADOR.md`.
//! - **El arranque**: solo si el modulo trae `principal`, y entonces se
//!   exporta como `main`, que es la entrada que busca el enlazador.
//!
//! [carril]  VERDE     escribe un formato; no decide nada de la maquina, y el
//!                     banco lee el objeto con el contrato (`objeto::read`)
//! [cuesta]  TAREA     un simbolo mal puesto es un `call` a otro sitio: el
//!                     programa enlazado hace otra cosa, o no arranca
//! [riesgo]  ESPEJO    copia la forma de `codegen/objeto.rs` de C; si aquel
//!                     cambia el contrato del `.bo`, este se queda atras y el
//!                     enlazador lo dice con nombre

use bmo_abi::bef::symbols::{name_hash, Symbol, SymbolBinding, SymbolKind, SymbolVisibility, SECTION_UNDEFINED};
use bmo_abi::bef2::{self, objeto::{Clase, Enlace}, Escritor, Region};

use crate::{rodata_de, Emitido};

/// **El `.bo` de un modulo ya emitido.**
pub fn empaquetar_objeto(e: &Emitido) -> Result<Vec<u8>, String> {
    let mut b = Escritor::objeto();
    let mut regiones: Vec<(Region, u64)> = vec![(Region::Codigo, e.codigo.len() as u64)];
    b.codigo(e.codigo.clone());

    let (rodata, donde) = rodata_de(e);
    if !rodata.is_empty() {
        regiones.push((Region::Constantes, rodata.len() as u64));
        b.constantes(rodata);
    }
    if !e.reubicaciones_del_monton.is_empty() {
        // Ocho bytes: la direccion del monton de la tarea, como en el `.ibx`.
        regiones.push((Region::Datos, 8));
        b.datos(vec![0u8; 8]);
    }

    // -- Los simbolos --------------------------------------------------------
    let mut entradas: Vec<Symbol> = Vec::new();
    let mut cadenas: Vec<u8> = Vec::new();
    let mut push = |entradas: &mut Vec<Symbol>, nombre: &str, kind: SymbolKind, local: bool, sec: u8, off: u64, size: u64| -> u32 {
        let name_off = cadenas.len() as u32;
        cadenas.extend_from_slice(nombre.as_bytes());
        cadenas.push(0);
        entradas.push(Symbol {
            name_off,
            name_hash: name_hash(nombre),
            virt_addr: off,
            size,
            kind: kind as u8,
            binding: if local { SymbolBinding::Local } else { SymbolBinding::Global } as u8,
            visibility: SymbolVisibility::Default as u8,
            section_idx: sec,
            _reserved: 0,
        });
        (entradas.len() - 1) as u32
    };

    // Un simbolo local por region, como en C: lo que un enlace "de dentro"
    // nombra por su numero de region.
    for (r, len) in &regiones {
        let n = match r {
            Region::Codigo => ".code",
            Region::Constantes => ".rodata",
            Region::Datos => ".data",
            Region::Ceros => ".bss",
        };
        push(&mut entradas, n, SymbolKind::Section, true, *r as u8, 0, *len);
    }

    // Las funciones, en orden de codigo, con su medida hasta la siguiente.
    let mut funcs: Vec<(usize, String)> = e.inicios.iter().map(|(n, o)| (*o, n.clone())).collect();
    funcs.sort();
    for (i, (off, n)) in funcs.iter().enumerate() {
        let fin = funcs.get(i + 1).map(|x| x.0).unwrap_or(e.codigo.len());
        push(&mut entradas, n, SymbolKind::Function, false, Region::Codigo as u8, *off as u64, (fin - off) as u64);
    }
    // ** El arranque es `main` para el enlazador: es lo primero que se emitio
    // (offset 0) y solo existe si el modulo trae `principal`.
    if e.arranca {
        let fin = funcs.first().map(|x| x.0).unwrap_or(e.codigo.len());
        push(&mut entradas, "main", SymbolKind::Function, false, Region::Codigo as u8, 0, fin as u64);
    }

    // Lo que este modulo LLAMA y no trae: indefinido, ordenado para que el
    // objeto sea los mismos bytes cada vez.
    let mut faltan: Vec<String> = e.externas.iter().map(|(_, n)| n.clone()).collect();
    faltan.sort();
    faltan.dedup();
    let mut indefinido: Vec<(String, u32)> = Vec::new();
    for n in faltan {
        let s = push(&mut entradas, &n, SymbolKind::Function, false, SECTION_UNDEFINED, 0, 0);
        indefinido.push((n, s));
    }

    // -- Los enlaces ---------------------------------------------------------
    let mut enlaces: Vec<Enlace> = Vec::new();
    for (off, i) in &e.reubicaciones {
        if let Some(d) = donde.get(*i as usize) {
            enlaces.push(Enlace {
                clase: Clase::Region,
                donde: Region::Codigo,
                offset: *off as u32,
                simbolo: Region::Constantes as u32,
                addend: *d as i64,
            });
        }
    }
    for off in &e.reubicaciones_del_monton {
        enlaces.push(Enlace {
            clase: Clase::Region,
            donde: Region::Codigo,
            offset: *off as u32,
            simbolo: Region::Datos as u32,
            addend: 0,
        });
    }
    for (hueco, nombre) in &e.externas {
        let sym = indefinido.iter().find(|(n, _)| n == nombre).map(|(_, s)| *s).expect("se acaba de anadir");
        enlaces.push(Enlace {
            clase: Clase::Rel32,
            donde: Region::Codigo,
            offset: *hueco as u32,
            simbolo: sym,
            // El hueco son los 4 bytes del `call`: el desplazamiento se mide
            // desde la instruccion siguiente, que es hueco + 4.
            addend: -4,
        });
    }

    b.anexo(bef2::ANEXO_SIMBOLOS, bmo_abi::bef::symbols::en_bytes(&entradas, &cadenas));
    if !enlaces.is_empty() {
        let mut raw = Vec::with_capacity(enlaces.len() * bef2::objeto::ENLACE);
        for en in &enlaces {
            raw.extend_from_slice(&en.a_bytes());
        }
        b.anexo(bef2::ANEXO_ENLACE, raw);
    }
    let bytes = b.construir().map_err(|x| x.to_string())?;
    // El contrato del objeto se exige aqui, no en el enlazador: un `.bo` que
    // no lo cumple es un fallo de ESTE fichero.
    bef2::objeto::read(&bytes).map_err(|f| format!("el objeto de INTI no cumple el contrato: {f:?}"))?;
    Ok(bytes)
}
