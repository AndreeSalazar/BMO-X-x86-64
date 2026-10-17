//! Que motivo tiene un metodo CONOCIDO en el contexto 3D de BMO-X.
//! [carril] VERDE -- tabla descriptiva; no autoriza ni emite ordenes
//! [consumo] NADA -- solo bajo consulta
//!
//! Los numeros vienen de las constantes del driver (clc797.h). No se
//! interpreta un numero del motor de copia como si fuera del motor 3D.

use crate::{raster as ra, tresde as td};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track { State, Memory, Synchronization, Draw, Unknown }
impl Track {
    pub const fn label(self) -> &'static str {
        match self { Self::State => "ESTADO", Self::Memory => "MEMORIA", Self::Synchronization => "SINCRONIA", Self::Draw => "DIBUJO", Self::Unknown => "SIN INTERPRETAR" }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meaning { pub name: &'static str, pub track: Track, pub why: &'static str }

/// Clases DECLARADAS en esta copia. No observa el runlist del dispositivo.
/// Empieza sin suponer una clase; SET_OBJECT puede cambiar el contexto.
pub struct Context { classes: [Option<u16>; 8] }
impl Context {
    pub const fn new() -> Self { Self { classes: [None; 8] } }
    pub fn describe(&mut self, c: super::empuje::Command) -> Meaning {
        if c.subchannel >= 8 { return unknown(); }
        if c.method == 0 {
            self.classes[c.subchannel as usize] = Some((c.value & 0xffff) as u16);
            return Meaning { name: "SET_OBJECT", track: Track::State, why: "declara clase en la copia; no acredita la clase real del canal" };
        }
        if self.classes[c.subchannel as usize] == Some(crate::gr::AMPERE_B as u16) {
            ampere_3d(c.subchannel, c.method)
        } else { unknown() }
    }
}
impl Default for Context { fn default() -> Self { Self::new() } }

fn unknown() -> Meaning {
    Meaning { name: "DESCONOCIDO", track: Track::Unknown, why: "bytes conservados; falta contexto o entrada en la tabla" }
}

/// Contexto explicito AMPERE_B en subcanal 0, como lo construye BMO-X.
/// No infiere ni cambia la clase asociada al canal real.
pub fn ampere_3d(subchannel: u8, method: u16) -> Meaning {
    use Track::*;
    let unknown = unknown();
    if subchannel != td::SUBCANAL as u8 || method % 4 != 0 { return unknown; }
    let (name, track, why) = match method as u32 {
        td::SET_OBJECT => ("SET_OBJECT", State, "declara la clase esperada; no demuestra que el canal real la tenga"),
        td::WAIT_FOR_IDLE => ("WAIT_FOR_IDLE", Synchronization, "pide esperar al motor; verlo preparado no demuestra finalizacion"),
        td::SET_REPORT_SEMAPHORE_A..=0x1b0c => ("REPORT_SEMAPHORE", Synchronization, "configura direccion, valor o modo del informe; falta observar la paga"),
        ra::BEGIN => ("BEGIN", Draw, "selecciona la primitiva del dibujo"),
        ra::END => ("END", Draw, "cierra la secuencia de dibujo; no es una confirmacion de GPU"),
        ra::SET_VERTEX_ARRAY_START => ("VERTEX_ARRAY_START", Draw, "selecciona el primer vertice"),
        0x0d78 => ("VERTEX_ARRAY_COUNT", Draw, "declara cuantos vertices consumir"),
        td::CLEAR_SURFACE => ("CLEAR_SURFACE", Draw, "solicita la limpieza del destino configurado"),
        ra::SET_DEPTH_TEST => ("SET_DEPTH_TEST", State, "activa o desactiva la prueba de profundidad"),
        td::SET_CT_SELECT => ("SET_CT_SELECT", State, "selecciona los destinos de color"),
        _ => return family(method as u32).unwrap_or(unknown),
    };
    Meaning { name, track, why }
}

fn family(method: u32) -> Option<Meaning> {
    use Track::*;
    let (name, track, why) = if (td::SET_COLOR_TARGET_A0..td::SET_COLOR_TARGET_A0 + 8*0x40).contains(&method) && (method-td::SET_COLOR_TARGET_A0)%0x40 <= 0x20 {
        ("COLOR_TARGET", Memory, "configura direccion, formato o medida del destino; no lee su contenido")
    } else if (0x2000..0x2000+6*64).contains(&method) && matches!((method-0x2000)%64, 0 | 0x0c | 0x10 | 0x14 | 0x18) {
        ("PIPELINE_SHADER", State, "configura etapa, registros o direccion de programa; no acredita su ejecucion")
    } else if (td::SET_COLOR_CLEAR_VALUE0..td::SET_COLOR_CLEAR_VALUE0+16).contains(&method) {
        ("COLOR_CLEAR_VALUE", State, "declara un canal del color de limpieza")
    } else { return None; };
    Some(Meaning { name, track, why })
}
