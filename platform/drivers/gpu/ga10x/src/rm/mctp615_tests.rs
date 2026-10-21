use super::*;
extern crate std;
use std::{vec, vec::Vec};

fn respuesta(seq: u64, status: u32, datos: &[u8]) -> Vec<u8> {
    let mut b = vec![0; ENVUELTA + GMC_CABECERA + datos.len()];
    let total = b.len();
    p32(&mut b, 0, MAGIC); p32(&mut b, 4, total as u32);
    p32(&mut b, 8, 0xC000_0001); p32(&mut b, 12, 0x2610_DE7E);
    p32(&mut b, 16, (GMC_CABECERA + datos.len()) as u32);
    p32(&mut b, 24, INIT | RESPUESTA); p32(&mut b, 28, datos.len() as u32);
    p64(&mut b, 32, seq); p32(&mut b, 40, status);
    b[64..].copy_from_slice(datos); b
}

#[test]
fn el_peer_simulado_contesta_la_secuencia_y_el_formato_615() {
    let mut b = [0; PAGINA];
    let s = Sistema { id: 0x2504_10DE, bar0: 0xDE00_0000, bdf: 0x2900, ..Sistema::default() };
    let n = iniciar(&mut b, 77, &s).unwrap();
    // Vector independiente: magic numerica de NVIDIA, en little endian.
    assert_eq!(&b[..4], &[0x50, 0x54, 0x43, 0x4D]);
    assert_eq!(&b[8..16], &[1, 0, 0, 0xC0, 0x7E, 0xDE, 0x10, 0x26]);
    assert_eq!(u32d(&b, 24), 0x10001);
    assert_eq!(u32d(&b, 40), 49152);
    let req = Gmc::leer(&b[..n]).unwrap();
    assert!(!req.respuesta); assert_eq!(req.secuencia, 77);
    // IMM32(key PCI_DEVICE_ID, value), seguido de otros NVKV.
    assert_eq!(&req.datos[..8], &[1, 0, 0, 0, 0xDE, 0x10, 4, 0x25]);
    config(req.datos).unwrap();
    let dato = (0x1234u64 << 32 | 0xFF0).to_le_bytes();
    let r = respuesta(req.secuencia, 0, &dato);
    let cfg = Gmc::leer(&r).unwrap().respuesta_init(77).unwrap();
    assert_eq!(cfg.cliente, 0x1234);
    assert_eq!(Gmc::leer(&r).unwrap().respuesta_init(78), Err(No::Correlacion));
}

#[test]
fn longitudes_flags_tipo_y_estado_no_se_ignoran() {
    let datos = 1u64.to_le_bytes();
    let buena = respuesta(1, 0, &datos);
    for n in 0..buena.len() { assert!(Gmc::leer(&buena[..n]).is_err()); }
    for (o, v) in [(0, 0), (4, 0), (4, 65537), (8, 1), (12, 0x2610_DE7F),
                   (16, 0), (20, 1), (24, INIT | 0x80000000), (28, 999), (44, 1)] {
        let mut b = buena.clone(); p32(&mut b, o, v);
        assert!(Gmc::leer(&b).is_err(), "offset {o}");
    }
    let mala = respuesta(1, 9, &datos);
    assert_eq!(Gmc::leer(&mala).unwrap().respuesta_init(1), Err(No::Estado(9)));
    assert!(Marco::leer(&[0; 24]).is_err());
}

#[test]
fn contadores_llenos_y_vuelta_no_se_confunden_con_vacio() {
    assert_eq!(disponibles(0, 0), Ok(0));
    assert_eq!(disponibles(63, 0), Ok(63));
    assert_eq!(disponibles(64, 0), Err(No::Puntero));
    assert_eq!(disponibles(3, u32::MAX - 5), Ok(9));
    for pri in [0xBADF_1002, 0xFFFF_FFFF] {
        assert_eq!(disponibles(pri, 0), Err(No::Puntero));
        assert_eq!(disponibles(0, pri), Err(No::Puntero));
    }
    assert_eq!(paginas(4096), Ok(1)); assert_eq!(paginas(4097), Ok(2));
    assert_eq!(paginas(65536), Ok(16)); assert!(paginas(65537).is_err());
}

#[test]
fn nvkv_truncado_desbordado_y_opcode_desconocido_no_pasa() {
    assert!(config(&[]).is_err()); assert!(config(&[0; 7]).is_err());
    for w in [0xF000_0000u64, 2 << 28, 0xFFFF_FFFF_2000_0000, 2 << 32 | 1 << 28 | 0xFFFF] {
        assert!(config(&w.to_le_bytes()).is_err());
    }
    let mut b = Vec::new();
    b.extend_from_slice(&(3u64 << 32 | 1 << 28 | 0xFF0).to_le_bytes());
    for w in [10u32, 11, 12, 0] { b.extend_from_slice(&w.to_le_bytes()); }
    assert_eq!(config(&b).unwrap(), Config { claves: 3, cliente: 10, dispositivo: 11, subdispositivo: 12 });
    let mut cab = crate::libos::cabecera_cpu_615();
    assert!(cabecera_cola(&cab).is_ok()); cab[12] = 62;
    assert!(cabecera_cola(&cab).is_err());
}

#[test]
fn el_contrato_no_deja_salir_otro_comando_ni_datos_inyectados() {
    let s = Sistema::default();
    let mut b = [0; PAGINA]; let n = iniciar(&mut b, 1, &s).unwrap();
    assert_eq!(crate::contrato::permitido_615(&b[..n], 1, &s), Ok(INIT));
    for o in [24, 32, 64, n-1] {
        let mut alterado = b; alterado[o] ^= 1;
        assert!(crate::contrato::permitido_615(&alterado[..n], 1, &s).is_err());
    }
}

#[test]
fn tx_no_hereda_contador_y_exige_que_se_relea_cero() {
    struct Regs { respuesta: u32, accesos: Vec<(bool, u32, u32)> }
    impl crate::Registros for Regs {
        fn escribir(&mut self, r: u32, v: u32) { self.accesos.push((true, r, v)); }
        fn leer(&mut self, r: u32) -> u32 { self.accesos.push((false, r, self.respuesta)); self.respuesta }
    }
    for respuesta in [0, 7, 0xBADF_1002, u32::MAX] {
        let mut r = Regs { respuesta, accesos: Vec::new() };
        assert_eq!(iniciar_tx(&mut r).is_ok(), respuesta == 0);
        assert_eq!(r.accesos, [(true, TX_HEAD, 0), (false, TX_HEAD, respuesta)]);
    }
}
