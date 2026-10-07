//! El banco del juez de la cache. Cada regla con su prueba de NO: el caso
//! que, sin esa regla, saldria mal.

use super::*;

/// El PAT que deja `s1_cpu` (`init_pat`): la casilla 4 en WC para el
/// framebuffer, el resto como al arrancar.
const PAT_S1: u64 = 0x0007_0401_0007_0406;
/// La PDE del physmap que cubre la caja negra (64 MiB), como la deja `s2_mem`:
/// presente, escribible, de 2 MiB, NX. Tipo: casilla 0 (WB).
const PDE_CAJA: u64 = 0x0400_0000 | PRESENTE | 0b10 | GRANDE | (1 << 63);
/// Un MTRR variable: `[base, base + tam)` con el tipo `t`, para un CPU de 48 bits.
fn var(base: u64, tam: u64, t: u8) -> (u64, u64) {
    let mascara = !(tam - 1) & 0x0000_FFFF_FFFF_F000;
    (base | t as u64, mascara | MTRR_VALE)
}
const DEF_UC: u64 = MTRR_ENCENDIDOS; // tipo 0
const CAJA: (u64, u64) = (0x0400_0000, 0x0420_0000);

#[test]
fn el_pat_de_arranque_y_el_de_s1_cpu_tienen_wt_en_la_casilla_1() {
    assert_eq!(casilla(PAT_DE_ARRANQUE, 0), Some(Tipo::Wb));
    assert_eq!(casilla(PAT_DE_ARRANQUE, 1), Some(Tipo::Wt));
    assert_eq!(casilla(PAT_DE_ARRANQUE, 2), Some(Tipo::UcMenos));
    assert_eq!(casilla(PAT_DE_ARRANQUE, 3), Some(Tipo::Uc));
    assert_eq!(casilla(PAT_S1, 4), Some(Tipo::Wc));
    assert_eq!(casilla(PAT_S1, 1), Some(Tipo::Wt));
    assert_eq!(casilla(PAT_S1, 8), None);
    // NO: un byte con bits reservados no es un tipo (un PAT roto no se cree).
    assert_eq!(casilla(0x0000_0000_0000_0E06 | 0x40 << 8, 1), None);
}

#[test]
fn la_caja_negra_de_v7_estaba_en_wb_y_eso_no_escribe_directo() {
    // ** LA PRUEBA DE NO: la PDE tal como estaba hasta V8.
    let t = tipo_pat_2m(PDE_CAJA, PAT_S1).unwrap();
    assert_eq!(t, Tipo::Wb);
    assert!(!t.escribe_directo());
}

#[test]
fn pasarla_a_wt_solo_cambia_los_bits_del_indice() {
    let d = pde_directa(PDE_CAJA, PAT_S1).unwrap();
    assert_eq!(d, PDE_CAJA | PWT);
    assert_eq!(tipo_pat_2m(d, PAT_S1), Some(Tipo::Wt));
    assert!(Tipo::Wt.escribe_directo());
    // Una PDE que apuntaba a la casilla WC (PAT_2M) sale con SOLO PWT: el
    // bit 12 se limpia (si se quedara, seria la casilla 5).
    let wc = PDE_CAJA | PAT_2M;
    assert_eq!(tipo_pat_2m(wc, PAT_S1), Some(Tipo::Wc));
    assert_eq!(pde_directa(wc, PAT_S1), Ok(PDE_CAJA | PWT));
    // Y con PCD puesto (UC-), igual.
    assert_eq!(pde_directa(PDE_CAJA | PCD, PAT_S1), Ok(PDE_CAJA | PWT));
}

#[test]
fn si_wt_solo_esta_en_la_casilla_5_usa_el_bit_12() {
    // Un PAT donde la 1 es WB y la 5 es WT.
    let pat = 0x0007_0406_0007_0606;
    assert_eq!(pde_directa(PDE_CAJA, pat), Ok(PDE_CAJA | PAT_2M | PWT));
    assert_eq!(tipo_pat_2m(PDE_CAJA | PAT_2M | PWT, pat), Some(Tipo::Wt));
}

#[test]
fn no_se_toca_lo_que_no_es_una_pagina_de_2_mib_ni_un_pat_sin_wt() {
    assert_eq!(pde_directa(PDE_CAJA & !PRESENTE, PAT_S1), Err(NoDirecta::NoPresente));
    // Una tabla de 4 KiB debajo: el bit 7 seria otra cosa en sus entradas.
    assert_eq!(pde_directa(PDE_CAJA & !GRANDE, PAT_S1), Err(NoDirecta::NoEsDeDosMegas));
    assert_eq!(pde_directa(PDE_CAJA, 0x0606_0606_0606_0606), Err(NoDirecta::PatSinWt));
}

#[test]
fn los_mtrr_apagados_son_uc_y_sin_rangos_manda_el_de_serie() {
    assert_eq!(tipo_mtrr(6, &[], 48, CAJA.0, CAJA.1), Some(Tipo::Uc));
    assert_eq!(tipo_mtrr(MTRR_ENCENDIDOS | 6, &[], 48, CAJA.0, CAJA.1), Some(Tipo::Wb));
    // NO: un tipo de serie que no existe en los MTRR (el 7) no es "algo".
    assert_eq!(tipo_mtrr(MTRR_ENCENDIDOS | 7, &[], 48, CAJA.0, CAJA.1), None);
}

#[test]
fn la_ram_de_la_placa_es_un_rango_wb_sobre_un_uc_de_serie() {
    // Lo tipico de un AMI de AM4: 0..2 GiB WB, el resto UC.
    let v = [var(0, 0x8000_0000, 6)];
    assert_eq!(tipo_mtrr(DEF_UC, &v, 48, CAJA.0, CAJA.1), Some(Tipo::Wb));
    // Un rango que no la toca no cambia nada.
    let v = [var(0, 0x8000_0000, 6), var(0x8000_0000, 0x4000_0000, 0)];
    assert_eq!(tipo_mtrr(DEF_UC, &v, 48, CAJA.0, CAJA.1), Some(Tipo::Wb));
    // Un rango sin el bit de "vale" no cuenta.
    let (b, m) = var(0, 0x1000_0000, 0);
    let v = [var(0, 0x8000_0000, 6), (b, m & !MTRR_VALE)];
    assert_eq!(tipo_mtrr(DEF_UC, &v, 48, CAJA.0, CAJA.1), Some(Tipo::Wb));
}

#[test]
fn los_solapes_que_da_intel_y_los_que_no() {
    let wb = var(0, 0x8000_0000, 6);
    assert_eq!(tipo_mtrr(DEF_UC, &[wb, var(0, 0x0800_0000, 0)], 48, CAJA.0, CAJA.1), Some(Tipo::Uc));
    assert_eq!(tipo_mtrr(DEF_UC, &[wb, var(0, 0x0800_0000, 4)], 48, CAJA.0, CAJA.1), Some(Tipo::Wt));
    // NO: WB y WC solapados no estan definidos, no se adivinan.
    assert_eq!(tipo_mtrr(DEF_UC, &[wb, var(0, 0x0800_0000, 1)], 48, CAJA.0, CAJA.1), None);
}

#[test]
fn un_rango_que_tapa_media_pagina_no_tiene_un_tipo() {
    // 1 MiB UC dentro de la pagina de 2 MiB de la caja.
    let v = [var(0, 0x8000_0000, 6), var(0x0410_0000, 0x10_0000, 0)];
    assert_eq!(tipo_mtrr(DEF_UC, &v, 48, CAJA.0, CAJA.1), None);
    // NO: con el rango entero dentro de la caja si se sabria (la caja de 1
    // MiB de abajo no lo toca).
    assert_eq!(tipo_mtrr(DEF_UC, &v, 48, CAJA.0, 0x0410_0000), Some(Tipo::Wb));
}

#[test]
fn una_mascara_con_huecos_o_por_debajo_de_1_mib_no_se_juzga() {
    let raro = (6u64, 0x0000_FFFF_F0F0_0000 | MTRR_VALE);
    assert_eq!(tipo_mtrr(DEF_UC, &[raro], 48, CAJA.0, CAJA.1), None);
    assert_eq!(tipo_mtrr(DEF_UC, &[], 48, 0x8_0000, 0x9_0000), None);
    assert_eq!(tipo_mtrr(DEF_UC, &[], 60, CAJA.0, CAJA.1), None);
}

#[test]
fn mtrr_por_pat_la_tabla_de_intel() {
    assert_eq!(efectivo(Tipo::Wb, Tipo::Wt), Some(Tipo::Wt));
    assert_eq!(efectivo(Tipo::Wb, Tipo::Wb), Some(Tipo::Wb));
    assert_eq!(efectivo(Tipo::Uc, Tipo::Wt), Some(Tipo::Uc));
    assert_eq!(efectivo(Tipo::Uc, Tipo::Wc), Some(Tipo::Wc));
    assert_eq!(efectivo(Tipo::Wb, Tipo::UcMenos), Some(Tipo::Uc));
    assert_eq!(efectivo(Tipo::Wc, Tipo::UcMenos), Some(Tipo::Wc));
    assert_eq!(efectivo(Tipo::Wt, Tipo::Wb), Some(Tipo::Wt));
    // NO: lo que no esta claro en las dos tablas, no se contesta.
    assert_eq!(efectivo(Tipo::Wp, Tipo::Wt), None);
    assert_eq!(efectivo(Tipo::Wc, Tipo::Wt), None);
}

#[test]
fn wc_no_es_escribir_directo() {
    // ** NO: WC parece "sin cache", pero junta escrituras en un bufer que un
    // reinicio de golpe tambien borra. Para la caja negra es tan malo como WB.
    assert!(!Tipo::Wc.escribe_directo());
    assert!(!Tipo::Wp.escribe_directo());
    assert!(Tipo::Uc.escribe_directo());
    assert!(Tipo::UcMenos.escribe_directo());
}

#[test]
fn el_veredicto_entero_de_la_caja_negra() {
    let mtrr = tipo_mtrr(DEF_UC, &[var(0, 0x8000_0000, 6)], 48, CAJA.0, CAJA.1).unwrap();
    let antes = efectivo(mtrr, tipo_pat_2m(PDE_CAJA, PAT_S1).unwrap()).unwrap();
    assert_eq!(antes, Tipo::Wb);
    let d = pde_directa(PDE_CAJA, PAT_S1).unwrap();
    let ahora = efectivo(mtrr, tipo_pat_2m(d, PAT_S1).unwrap()).unwrap();
    assert_eq!(ahora, Tipo::Wt);
    assert!(ahora.escribe_directo() && !antes.escribe_directo());
}
