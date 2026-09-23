//! `GLSL.std.450`: las instrucciones extendidas que el juez conoce, y sus numeros.
//!
//! ** GENERADO por `herramientas/table.py --escribir` desde la gramatica de
//! Khronos (licencia MIT), SPIR-V 1.6 rev 4. No se edita a mano: las
//! familias las decide `FILAS` en el script; los numeros son de la
//! especificacion. `--cotejar` comprueba que coinciden.
//!
//! [consumo]  NADA   datos constantes: no gastan ni en reposo ni corriendo

#![allow(non_upper_case_globals)]

use crate::{GlslGroup, GlslInfo};

/// De `extinst.glsl.std.450.grammar.json`. Ordenadas por numero.
pub const GLSL450: &[GlslInfo] = &[
    GlslInfo { number: 4, name: "FAbs", operands: 1, group: GlslGroup::Float },
    GlslInfo { number: 5, name: "SAbs", operands: 1, group: GlslGroup::Int },
    GlslInfo { number: 8, name: "Floor", operands: 1, group: GlslGroup::Float },
    GlslInfo { number: 9, name: "Ceil", operands: 1, group: GlslGroup::Float },
    GlslInfo { number: 10, name: "Fract", operands: 1, group: GlslGroup::Float },
    GlslInfo { number: 13, name: "Sin", operands: 1, group: GlslGroup::Transcendental },
    GlslInfo { number: 14, name: "Cos", operands: 1, group: GlslGroup::Transcendental },
    GlslInfo { number: 26, name: "Pow", operands: 2, group: GlslGroup::Transcendental },
    GlslInfo { number: 27, name: "Exp", operands: 1, group: GlslGroup::Transcendental },
    GlslInfo { number: 28, name: "Log", operands: 1, group: GlslGroup::Transcendental },
    GlslInfo { number: 31, name: "Sqrt", operands: 1, group: GlslGroup::Float },
    GlslInfo { number: 32, name: "InverseSqrt", operands: 1, group: GlslGroup::Float },
    GlslInfo { number: 37, name: "FMin", operands: 2, group: GlslGroup::Float },
    GlslInfo { number: 38, name: "UMin", operands: 2, group: GlslGroup::Int },
    GlslInfo { number: 39, name: "SMin", operands: 2, group: GlslGroup::Int },
    GlslInfo { number: 40, name: "FMax", operands: 2, group: GlslGroup::Float },
    GlslInfo { number: 41, name: "UMax", operands: 2, group: GlslGroup::Int },
    GlslInfo { number: 42, name: "SMax", operands: 2, group: GlslGroup::Int },
    GlslInfo { number: 43, name: "FClamp", operands: 3, group: GlslGroup::Float },
    GlslInfo { number: 44, name: "UClamp", operands: 3, group: GlslGroup::Int },
    GlslInfo { number: 45, name: "SClamp", operands: 3, group: GlslGroup::Int },
    GlslInfo { number: 46, name: "FMix", operands: 3, group: GlslGroup::Float },
    GlslInfo { number: 48, name: "Step", operands: 2, group: GlslGroup::Float },
    GlslInfo { number: 50, name: "Fma", operands: 3, group: GlslGroup::Float },
];

pub const FAbs: u32 = 4;
pub const SAbs: u32 = 5;
pub const Floor: u32 = 8;
pub const Ceil: u32 = 9;
pub const Fract: u32 = 10;
pub const Sin: u32 = 13;
pub const Cos: u32 = 14;
pub const Pow: u32 = 26;
pub const Exp: u32 = 27;
pub const Log: u32 = 28;
pub const Sqrt: u32 = 31;
pub const InverseSqrt: u32 = 32;
pub const FMin: u32 = 37;
pub const UMin: u32 = 38;
pub const SMin: u32 = 39;
pub const FMax: u32 = 40;
pub const UMax: u32 = 41;
pub const SMax: u32 = 42;
pub const FClamp: u32 = 43;
pub const UClamp: u32 = 44;
pub const SClamp: u32 = 45;
pub const FMix: u32 = 46;
pub const Step: u32 = 48;
pub const Fma: u32 = 50;
