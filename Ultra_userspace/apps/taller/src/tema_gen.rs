//! GENERADO POR MAQUETA DESDE `Ultra_userspace/apps/taller/aspecto/titan.maqueta` -- NO EDITAR A MANO.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta
//!                     si algo cambio (L6h)
//!
//! Lo que se edita es el `.maqueta`. Cambiar esto es escribir una verdad
//! que la siguiente compilacion borra.
//!
//! ** LA PALETA DE BMO-X EN UN SITIO. Antes eran 62 constantes de color
//! repartidas por quince ficheros, 33 de ellas usadas UNA vez -- y cada
//! panel nuevo se inventaba las suyas porque no habia donde consultarlas.
//!
//! El formato es `0x00RRGGBB`, que es el que quiere el framebuffer.

#![allow(dead_code)]

pub const CANVAS_FONDO: u32 = 0x0003_0510;
pub const GRID: u32 = 0x0012_1A3C;
pub const BAR_FONDO: u32 = 0x0006_091A;
pub const NODE_FONDO: u32 = 0x0009_0E26;
pub const NODE_BORDE: u32 = 0x0022_2E6A;
pub const PICKED_FONDO: u32 = 0x0010_1A46;
pub const INK: u32 = 0x00E9_EEFF;
pub const INK_DIM: u32 = 0x0078_84B4;
pub const TITLE: u32 = 0x00AF_C3FF;
pub const BLUE: u32 = 0x003D_6BFF;
pub const VIOLET: u32 = 0x008C_52FF;
pub const ACCENT: u32 = 0x0070_D6FF;
pub const GOOD: u32 = 0x0052_E0A0;
pub const BAD: u32 = 0x00FF_4D6A;
pub const GREY: u32 = 0x0050_5878;
pub const CABLE: u32 = 0x005A_7CDA;
pub const CABLE_BORDE: u32 = 0x0035_4C9A;
pub const USE: u32 = 0x0036_C4D8;
pub const MUT: u32 = 0x00FF_B84B;
pub const DECIDE: u32 = 0x00F2_E27A;
pub const LOOP: u32 = 0x00B2_8CFF;
pub const TAB_FONDO: u32 = 0x0007_0B21;
pub const TAB_BORDE: u32 = 0x002C_3C8C;
pub const TAB_FILA: u32 = 0x00C9_D4FF;
pub const TAB_ELEGIDA_FONDO: u32 = 0x0016_246A;
