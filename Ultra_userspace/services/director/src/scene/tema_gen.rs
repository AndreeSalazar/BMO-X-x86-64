//! GENERADO POR MAQUETA DESDE `toolchain/tools/maqueta/tema/tema.maqueta` -- NO EDITAR A MANO.
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

pub const INK: u32 = 0x00E6_EDF6;
pub const INK_DIM: u32 = 0x009A_96B8;
pub const INK_OK: u32 = 0x007E_E787;
pub const INK_BAD: u32 = 0x00FF_8A7A;
pub const ACCENT: u32 = 0x005E_F2E6;
pub const BOX_FONDO: u32 = 0x001A_1631;
pub const BOX_BORDE: u32 = 0x003A_3163;
pub const FIELD_FONDO: u32 = 0x0011_0E22;
pub const TASKBAR_FONDO: u32 = 0x0009_080F;
pub const TASKBAR_BORDE: u32 = 0x002B_2250;
pub const BG_TOP_FONDO: u32 = 0x0016_1236;
pub const FINO_FONDO: u32 = 0x0011_0E24;
pub const FINO_BORDE: u32 = 0x00C8_A86B;
pub const MARCA_FONDO: u32 = 0x0023_1D47;
pub const MARFIL: u32 = 0x00F3_EEE4;
pub const PERLA: u32 = 0x00A6_A2C0;
pub const LATON: u32 = 0x00C8_A86B;
pub const FASE_FONDO: u32 = 0x0005_070D;
pub const FASE_BORDE: u32 = 0x001C_4E66;
pub const FASE_CIAN: u32 = 0x005E_E8FF;
pub const FASE_NEON: u32 = 0x00FF_2E88;
pub const FASE_TINTA: u32 = 0x00EA_F7FF;
pub const FASE_TENUE: u32 = 0x007F_96AE;
pub const MISION_FONDO: u32 = 0x0009_080F;
pub const MISION_BORDE: u32 = 0x002B_2250;
pub const MISION_CIELO_FONDO: u32 = 0x0004_030A;
pub const MISION_REJILLA: u32 = 0x0016_1236;
pub const MISION_OJO: u32 = 0x005E_F2E6;
pub const MISION_NEON: u32 = 0x00FF_2E88;
pub const MISION_AZUL: u32 = 0x003D_A5FF;
pub const MISION_TINTA: u32 = 0x00F2_F7F9;
pub const MISION_TENUE: u32 = 0x009A_96B8;
pub const MISION_GO: u32 = 0x007E_E787;
pub const MISION_NOGO: u32 = 0x00FF_5A6E;
pub const MISION_CUIDADO: u32 = 0x00FF_D45E;
