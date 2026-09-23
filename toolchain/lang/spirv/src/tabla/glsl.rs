//! `GLSL.std.450`: las instrucciones extendidas que el juez conoce, y sus numeros.
//!
//! ** GENERADO por `herramientas/tabla.py --escribir` desde la gramatica de
//! Khronos (licencia MIT), SPIR-V 1.6 rev 4. No se edita a mano: las
//! familias las decide `FILAS` en el script; los numeros son de la
//! especificacion. `--cotejar` comprueba que coinciden.
//!
//! [consumo]  NADA   datos constantes: no gastan ni en reposo ni corriendo

#![allow(non_upper_case_globals)]

use crate::{FilaGlsl, GrupoGlsl};

/// De `extinst.glsl.std.450.grammar.json`. Ordenadas por numero.
pub const GLSL450: &[FilaGlsl] = &[
    FilaGlsl { numero: 4, nombre: "FAbs", operandos: 1, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 5, nombre: "SAbs", operandos: 1, grupo: GrupoGlsl::Entero },
    FilaGlsl { numero: 8, nombre: "Floor", operandos: 1, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 9, nombre: "Ceil", operandos: 1, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 10, nombre: "Fract", operandos: 1, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 13, nombre: "Sin", operandos: 1, grupo: GrupoGlsl::Trascendente },
    FilaGlsl { numero: 14, nombre: "Cos", operandos: 1, grupo: GrupoGlsl::Trascendente },
    FilaGlsl { numero: 26, nombre: "Pow", operandos: 2, grupo: GrupoGlsl::Trascendente },
    FilaGlsl { numero: 27, nombre: "Exp", operandos: 1, grupo: GrupoGlsl::Trascendente },
    FilaGlsl { numero: 28, nombre: "Log", operandos: 1, grupo: GrupoGlsl::Trascendente },
    FilaGlsl { numero: 31, nombre: "Sqrt", operandos: 1, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 32, nombre: "InverseSqrt", operandos: 1, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 37, nombre: "FMin", operandos: 2, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 38, nombre: "UMin", operandos: 2, grupo: GrupoGlsl::Entero },
    FilaGlsl { numero: 39, nombre: "SMin", operandos: 2, grupo: GrupoGlsl::Entero },
    FilaGlsl { numero: 40, nombre: "FMax", operandos: 2, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 41, nombre: "UMax", operandos: 2, grupo: GrupoGlsl::Entero },
    FilaGlsl { numero: 42, nombre: "SMax", operandos: 2, grupo: GrupoGlsl::Entero },
    FilaGlsl { numero: 43, nombre: "FClamp", operandos: 3, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 44, nombre: "UClamp", operandos: 3, grupo: GrupoGlsl::Entero },
    FilaGlsl { numero: 45, nombre: "SClamp", operandos: 3, grupo: GrupoGlsl::Entero },
    FilaGlsl { numero: 46, nombre: "FMix", operandos: 3, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 48, nombre: "Step", operandos: 2, grupo: GrupoGlsl::Flotante },
    FilaGlsl { numero: 50, nombre: "Fma", operandos: 3, grupo: GrupoGlsl::Flotante },
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
