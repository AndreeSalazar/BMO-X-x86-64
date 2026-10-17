//! Rechazos del lector LIBOS: bytes sinteticos, sin aparato.
//! [carril] VERDE
//! [consumo] NADA -- solo banco solicitado
extern crate std;
use super::*;
use diccionario::Elf;
use std::vec;
use std::vec::Vec;

fn poner(b:&mut[u8], o:usize, n:u64) { b[o..o+8].copy_from_slice(&n.to_le_bytes()); }
fn anillo(n:usize, put:u64, palabras:&[u64]) -> Vec<u8> {
    let mut b=vec![0;(n+1)*8]; poner(&mut b,0,put);
    let inicio=put-palabras.len() as u64;
    for (i,v) in palabras.iter().enumerate() {
        poner(&mut b,8*(1+((inicio+i as u64)%n as u64) as usize),*v);
    }
    b
}
struct D(Option<u8>);
impl Diccionario for D { fn argumentos(&mut self,_:u64)->Option<u8>{self.0} }

#[test]
fn vacio_es_vacio_y_no_una_causa_de_0x15() {
    let b=anillo(7,0,&[]); let mut l=Lector::nuevo(&b).unwrap();
    assert_eq!(l.siguiente(&mut D(None)),Ok(None));
    assert_eq!(resumen(&b).unwrap().retenidas,0);
    assert_eq!(resumen(&b[..8]),Err(Error::Longitud));
}
#[test]
fn orden_inverso_conserva_argumentos_y_wrap() {
    // Dos registros: args, meta, ts. El segundo cruza el final del anillo.
    let b=anillo(8,11,&[12,13,0x1010,111,22,23,0x2020,222]);
    let mut l=Lector::nuevo(&b).unwrap(); let mut d=D(Some(2));
    let r=l.siguiente(&mut d).unwrap().unwrap();
    assert_eq!((r.timestamp,r.metadata_va,r.argumentos[..2].to_vec()),(222,0x2020,vec![22,23]));
    assert_eq!(l.siguiente(&mut d).unwrap().unwrap().timestamp,111);
    assert_eq!(l.siguiente(&mut d),Ok(None));
    assert_eq!(resumen(&b).unwrap().sobrescritas,3); // palabras, no registros
}
#[test]
fn falta_diccionario_no_salta_a_un_argumento_que_parece_timestamp() {
    let b=anillo(4,4,&[5,6,7,8]); let mut l=Lector::nuevo(&b).unwrap();
    assert_eq!(l.siguiente(&mut D(None)),Err(Error::SinDiccionario(7)));
    assert_eq!(l.cabecera(),Ok(Some((8,7))));
    assert_eq!(l.siguiente(&mut D(Some(21))),Err(Error::Argumentos(21)));
    assert_eq!(l.siguiente(&mut D(Some(3))),Err(Error::Truncado));
    assert_eq!(l.cabecera(),Ok(Some((8,7))));
}
#[test]
fn put_maximo_no_desborda_y_el_truncado_no_avanza() {
    let b=anillo(3,u64::MAX,&[1,2,3]); let mut l=Lector::nuevo(&b).unwrap();
    assert!(l.siguiente(&mut D(Some(2))).is_err());
    assert_eq!(l.cabecera(),Ok(Some((3,2))));
    let b=anillo(3,2,&[1,0]);
    assert_eq!(Lector::nuevo(&b).unwrap().siguiente(&mut D(Some(0))),Err(Error::TimestampCero));
}

// ELF sintetico con .logging direccionada, NOBITS y nombres. No es firmware.
fn elf() -> Vec<u8> {
    let mut b=vec![0;1024]; b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    poner(&mut b,40,64); b[52..54].copy_from_slice(&64u16.to_le_bytes());
    b[58..60].copy_from_slice(&64u16.to_le_bytes());
    b[60..62].copy_from_slice(&4u16.to_le_bytes());
    b[62..64].copy_from_slice(&3u16.to_le_bytes());
    // section1: bytes a VA 0x1000, offset 512
    b[128..132].copy_from_slice(&1u32.to_le_bytes());
    b[132..136].copy_from_slice(&1u32.to_le_bytes());
    poner(&mut b,144,0x1000); poner(&mut b,152,512); poner(&mut b,160,256);
    // section2: BSS, no bytes in file
    b[196..200].copy_from_slice(&8u32.to_le_bytes());
    poner(&mut b,208,0x2000); poner(&mut b,216,800); poner(&mut b,224,100);
    b[260..264].copy_from_slice(&3u32.to_le_bytes());
    poner(&mut b,280,900); poner(&mut b,288,10);
    b[900..910].copy_from_slice(b"\0.logging\0");
    poner(&mut b,512,0x1080); poner(&mut b,520,0x1090);
    b[528..532].copy_from_slice(&42u32.to_le_bytes()); b[532]=2;
    b[640..647].copy_from_slice(b"file.c\0"); b[656..662].copy_from_slice(b"x=%llx");
    b
}
#[test]
fn va_metadata_y_cadena_se_resuelven_sin_punteros_del_host() {
    let b=elf(); let mut e=Elf::nuevo(b.as_slice(),b.len() as u64).unwrap();
    let s=e.seccion(1).unwrap(); assert_eq!(&s.nombre[..s.n],b".logging");
    assert_eq!(e.mapear(0x1000,24),Ok(512));
    let m=e.metadata(0x1000).unwrap(); assert_eq!((m.linea,m.argumentos),(42,2));
    let mut out=[0;32]; let n=e.texto(m.archivo,&mut out).unwrap();
    assert_eq!(&out[..n],b"file.c");
    let logs=anillo(4,4,&[12,13,0x1000,99]);
    assert_eq!(Lector::nuevo(&logs).unwrap().siguiente(&mut e).unwrap().unwrap().argumentos[..2],[12,13]);
    assert_eq!(e.mapear(0x2000,1),Err(diccionario::Error::NoMapeada));
    assert_eq!(e.mapear(u64::MAX,2),Err(diccionario::Error::Limites));
}
#[test]
fn metadata_extendida_y_cadenas_acotadas() {
    let mut b=elf();
    poner(&mut b,544,0x8000_0000_0000_0000); poner(&mut b,552,0x1080);
    let meta=b[512..536].to_vec(); b[560..584].copy_from_slice(&meta);
    let mut e=Elf::nuevo(b.as_slice(),b.len() as u64).unwrap();
    let m=e.metadata(0x1020).unwrap(); assert_eq!((m.remoto,m.argumentos),(0x1080,2));
    assert_eq!(e.texto(0x1090,&mut[0;3]),Err(diccionario::Error::SinNul));
}
#[test]
fn elf_hostil_ambiguo_truncado_y_nombres_sin_nul_se_rechazan() {
    let mut b=elf(); poner(&mut b,40,u64::MAX);
    assert!(Elf::nuevo(b.as_slice(),b.len() as u64).is_err());
    let mut b=elf(); b[196..200].copy_from_slice(&1u32.to_le_bytes());
    poner(&mut b,208,0x1000);
    assert_eq!(Elf::nuevo(b.as_slice(),1024).unwrap().mapear(0x1000,1),Err(diccionario::Error::Ambigua));
    let mut b=elf(); b[909]=b'X';
    assert_eq!(Elf::nuevo(b.as_slice(),1024).unwrap().seccion(1),Err(diccionario::Error::SinNul));
    let b=elf(); assert!(Elf::nuevo(&b[..100],1024).unwrap().seccion(1).is_err());
}
#[test]
fn phdr_usa_filesz_y_no_memsz() {
    let mut b=elf(); poner(&mut b,32,320);
    b[54..56].copy_from_slice(&56u16.to_le_bytes()); b[56..58].copy_from_slice(&1u16.to_le_bytes());
    b[320..324].copy_from_slice(&1u32.to_le_bytes()); poner(&mut b,328,800);
    poner(&mut b,336,0x3000); poner(&mut b,352,8); poner(&mut b,360,100);
    let mut e=Elf::nuevo(b.as_slice(),1024).unwrap();
    assert_eq!(e.mapear(0x3000,8),Ok(800));
    assert_eq!(e.mapear(0x3008,1),Err(diccionario::Error::NoMapeada));
}
