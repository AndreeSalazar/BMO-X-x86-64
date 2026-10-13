//! ** LI2f (12-10): EL COMPUTO DE UNA APP, de la casa a la 3060 de mentira --
//! los ids (S2R), la barrera y los UAV crudos y estructurados con la
//! direccion SUJETA (`uav.rs`), contra `Programa::despachar` de la CPU: los
//! MISMOS bytes en los UAV, grupo a grupo. Y lo de fuera: ni una direccion
//! que se salga de lo prestado (el simulador lo diria: `NoSimula::Fallo`).

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use bmo_proton_x::bufer::{Modo, Rebanadas, Uav};
use bmo_proton_x::dxil::programa::{Computo, Op, OpEntera, Programa, Ranuras};
use bmo_proton_x::textura::Recursos;

use crate::simula::Memoria;
use crate::simula_grupo::correr_grupo;
use crate::{emitir_con, Abi, DelUav, NoEmite, Precarga};

/// Un UAV de la prueba: sus bytes, su vista, y donde lo ve la 3060.
#[derive(Clone)]
struct Vista {
    bytes: Vec<u8>,
    modo: Modo,
    elementos: u32,
    paso: u32,
}

/// La memoria de la 3060: cada UAV en su ventana (la IOVA de la ranura k,
/// `BASE + k * RANURA`); fuera de ellas, un fallo.
struct Ventanas(Vec<Vec<u8>>);
const BASE: u64 = 0x1_2340_0000;
const RANURA: u64 = 0x10_0000;

impl Ventanas {
    fn donde(&self, dir: u64) -> Option<(usize, usize)> {
        let k = dir.checked_sub(BASE)? / RANURA;
        let o = (dir - BASE - k * RANURA) as usize;
        let b = self.0.get(k as usize)?;
        (dir % 4 == 0 && o + 4 <= b.len()).then_some((k as usize, o))
    }
}

impl Memoria for Ventanas {
    fn leer(&mut self, dir: u64) -> Option<u32> {
        let (k, o) = self.donde(dir)?;
        Some(u32::from_le_bytes(self.0[k][o..o + 4].try_into().unwrap()))
    }
    fn escribir(&mut self, dir: u64, v: u32) -> bool {
        match self.donde(dir) {
            Some((k, o)) => {
                self.0[k][o..o + 4].copy_from_slice(&v.to_le_bytes());
                true
            }
            None => false,
        }
    }
}

fn programa(ops: Vec<Op>, iniciales: Vec<u32>, uavs: usize, hilos: [u32; 3]) -> Programa {
    programa_con(ops, iniciales, uavs, hilos, 0)
}

fn programa_con(ops: Vec<Op>, iniciales: Vec<u32>, uavs: usize, hilos: [u32; 3], compartida: u32) -> Programa {
    let l = bmo_proton_x::dxil::programa::Lugar { espacio: 0, registro: 0, vista: 0 };
    Programa {
        ops,
        iniciales: iniciales.into_iter().map(f32::from_bits).collect(),
        entradas: 0,
        salidas: 0,
        lee: 0,
        filas_cb: 0,
        ranuras: Ranuras { uavs: vec![l; uavs], ..Ranuras::default() },
        computo: Computo { hilos, compartida, temprana: false },
    }
}

/// Lo que dejan en los UAV la CPU y la 3060 de mentira, `grupos` en x.
fn las_dos(p: &Programa, vistas: &[Vista], grupos: u32) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    las_dos_con(p, vistas, &[], &[], grupos)
}

/// Lo mismo con SRV de bufer (`srvs`, el tN) y un cbuffer. En la 3060 de
/// mentira el UAV k vive en la ranura k y el SRV t en la 8 + t.
fn las_dos_con(p: &Programa, vistas: &[Vista], srvs: &[Vista], cb: &[u8], grupos: u32) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut casa: Vec<Vec<u8>> = vistas.iter().map(|v| v.bytes.clone()).collect();
    {
        let mut uavs: Vec<Option<Uav>> = casa.iter_mut().zip(vistas).map(|(b, v)| Some(Uav { bytes: b, formato: 0, paso: v.paso, elementos: v.elementos, contador: None, rebanadas: Rebanadas::PLANA })).collect();
        let buferes: Vec<Option<bmo_proton_x::bufer::Bufer>> = srvs.iter().map(|v| Some(bmo_proton_x::bufer::Bufer { bytes: &v.bytes, formato: 0, paso: v.paso, elementos: v.elementos })).collect();
        let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &buferes, dinamicas: None };
        p.despachar([grupos, 1, 1], cb, &rec, &mut uavs);
    }
    let e = emitir_con(p, 248, Abi::Registros).unwrap_or_else(|x| panic!("no se emite: {x:?}"));
    // Y lo emitido, juzgado: R0..R6 y R7 con lo que el kernel le da.
    {
        use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_con, juzgar_drenado, Contexto, RESERVADOS};
        juzgar_drenado(&e.codigo, &Contexto { registros: e.registros + RESERVADOS, sph: None }).unwrap_or_else(|b| panic!("R0..R6: {b}"));
        juzgar_cuerpo_con(&e.codigo, e.registros, crate::permisos_computo(&e, p).unwrap()).unwrap_or_else(|b| panic!("R7: {b}"));
    }
    let mut ventanas: Vec<Vec<u8>> = vistas.iter().map(|v| v.bytes.clone()).collect();
    ventanas.resize(8, Vec::new());
    ventanas.extend(srvs.iter().map(|v| v.bytes.clone()));
    let mut mem = Ventanas(ventanas);
    for gx in 0..grupos {
        // La compartida de la 3060 NO empieza a cero: basura, que el
        // prologo tiene que limpiar.
        let mut compartida = vec![0xA5u8; 4 * p.computo.compartida as usize];
        correr_grupo(&e.codigo, p.computo.hilos, [gx, 0, 0], [&[]; 8], &mut compartida, &mut mem, |_, m| cargar(&e, vistas, srvs, cb, m))
        .unwrap_or_else(|x| panic!("la 3060 de mentira: {x:?}"));
    }
    mem.0.truncate(vistas.len());
    (casa, mem.0)
}

/// Lo que el kernel carga antes del cuerpo: lo de cada UAV y SRV (por
/// `DelUav::valor`) y las filas del cbuffer.
fn cargar(e: &crate::Emitido, vistas: &[Vista], srvs: &[Vista], cb: &[u8], m: &mut crate::simula::Maquina) {
    for q in &e.precargas {
        match *q {
            Precarga::Uav { u, que, reg } => {
                let (v, ranura) = if u & crate::SRV != 0 { (&srvs[(u & !crate::SRV) as usize], 8 + (u & !crate::SRV) as u64) } else { (&vistas[u as usize], u as u64) };
                let x = que.valor(BASE + ranura * RANURA, v.bytes.len() as u32, v.elementos, v.paso).expect("se presta");
                m.r[reg as usize] = x as u32;
                if que == DelUav::Base {
                    m.r[reg as usize + 1] = (x >> 32) as u32;
                }
            }
            Precarga::Fila { fila, reg } => {
                for k in 0..4 {
                    let o = 16 * fila as usize + 4 * k;
                    m.r[reg as usize + k] = cb.get(o..o + 4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
                }
            }
            _ => panic!("un computo sin entradas ni texturas"),
        }
    }
}

fn palabras(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// `salida[id] = entrada[id] * 2` en CRUDO (`ByteAddressBuffer`, el byte
/// `4 * id`), con 3 grupos de 8 y una entrada de 20 palabras: las 4 ultimas
/// leen FUERA (0 en las dos), y la salida de 22 palabras pierde las 2
/// ultimas escrituras (en las dos).
#[test]
fn crudo_el_doble_con_lo_de_fuera() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::Entera { d: 1, a: 0, b: 2, op: OpEntera::Shl },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9 },
        Op::Mul { d: 7, a: 3, b: 8 },
        Op::EscribeUav { u: 1, modo: Modo::Crudo, i: 1, desp: 9, z: 9, v: [7, 7, 7, 7], mascara: 1 },
    ];
    let mut ini = vec![0u32; 10];
    ini[2] = 2;
    ini[8] = 2.0f32.to_bits();
    let p = programa(ops, ini, 2, [8, 1, 1]);
    let entrada: Vec<u32> = (0..20).map(|k| (k as f32 + 0.5).to_bits()).collect();
    let vistas = [
        Vista { bytes: palabras(&entrada), modo: Modo::Crudo, elementos: 20, paso: 0 },
        Vista { bytes: palabras(&[0xDEAD_BEEF; 22]), modo: Modo::Crudo, elementos: 22, paso: 0 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas, 3);
    assert_eq!(gpu, casa, "los mismos bytes en los dos UAV");
    let w = |k: usize| u32::from_le_bytes(gpu[1][4 * k..4 * k + 4].try_into().unwrap());
    assert_eq!(f32::from_bits(w(3)), 7.0);
    assert_eq!(w(21), 0, "la palabra 21 se leyo FUERA de la entrada (0) y se escribio");
    let _ = vistas[0].modo;
}

/// ESTRUCTURADO (`StructuredBuffer<S>` de 12 bytes): cada hilo lee las
/// cuatro palabras desde `desp` = 4 de SU elemento -- la cuarta se sale del
/// elemento (0) -- y las escribe en el suyo de 16 bytes. Con 7 elementos de
/// entrada y 2 grupos de 4, el elemento 7 entero se lee fuera.
#[test]
fn estructurado_con_la_palabra_que_se_sale() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Estructurado, i: 0, desp: 1, z: 2 },
        Op::EscribeUav { u: 1, modo: Modo::Estructurado, i: 0, desp: 2, z: 2, v: [3, 4, 5, 6], mascara: 0xF },
    ];
    let mut ini = vec![0u32; 7];
    ini[1] = 4;
    let p = programa(ops, ini, 2, [4, 1, 1]);
    let entrada: Vec<u32> = (0..21).map(|k| 0x100 + k).collect();
    let vistas = [
        Vista { bytes: palabras(&entrada), modo: Modo::Estructurado, elementos: 7, paso: 12 },
        Vista { bytes: palabras(&[0x5555_5555; 32]), modo: Modo::Estructurado, elementos: 8, paso: 16 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas, 2);
    assert_eq!(gpu, casa);
    let w = |k: usize| u32::from_le_bytes(gpu[1][4 * k..4 * k + 4].try_into().unwrap());
    assert_eq!([w(4), w(5), w(6), w(7)], [0x104, 0x105, 0, 0], "el elemento 1: dos palabras dentro, la tercera sale del elemento");
    assert_eq!([w(28), w(29), w(30), w(31)], [0, 0, 0, 0], "el elemento 7 no esta en la entrada");
}

/// Los ids: SV_GroupIndex en un grupo de 4 x 2 x 2 y SV_GroupID.x, cada
/// hilo en su palabra; y una barrera (fuera de todo `si`) en medio.
#[test]
fn los_ids_y_la_barrera() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 3, c: 0 },
        Op::IdHilo { d: 1, que: 1, c: 0 },
        Op::Barrera,
        Op::Entera { d: 2, a: 1, b: 9, op: OpEntera::Mul },
        Op::SumaEntera { d: 3, a: 2, b: 0 },
        Op::Entera { d: 4, a: 3, b: 8, op: OpEntera::Shl },
        Op::IdHilo { d: 5, que: 2, c: 2 },
        Op::IdHilo { d: 6, que: 0, c: 1 },
        Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 4, desp: 10, z: 10, v: [3, 5, 6, 0], mascara: 7 },
    ];
    let mut ini = vec![0u32; 11];
    ini[8] = 4; // * 16 bytes
    ini[9] = 16; // hilos del grupo
    let p = programa(ops, ini, 1, [4, 2, 2]);
    let vistas = [Vista { bytes: vec![0xAA; 16 * 48], modo: Modo::Crudo, elementos: 4 * 48, paso: 0 }];
    let (casa, gpu) = las_dos(&p, &vistas, 3);
    assert_eq!(gpu, casa);
    let w = |k: usize| u32::from_le_bytes(gpu[0][4 * k..4 * k + 4].try_into().unwrap());
    // El hilo 13 del grupo 2: (1, 1, 1); el 33 del despacho.
    assert_eq!([w(4 * 45), w(4 * 45 + 1), w(4 * 45 + 2)], [45, 1, 1]);
}

/// UN CUERPO QUE NO SE PORTA BIEN: la 3060 de mentira corre lo emitido con
/// indices enormes (`i` = 0xFFFF_FFF0 y desp que acarrean), y no sale de lo
/// prestado nunca -- la direccion esta SUJETA al tope --; y con el TOPE
/// mal puesto (mas alla de lo prestado), el simulador SI lo ve: la prueba
/// sabe decir que no.
#[test]
fn la_direccion_nunca_sale_de_lo_prestado() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::SumaEntera { d: 1, a: 0, b: 2 },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9 },
        Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9, v: [3, 4, 5, 6], mascara: 0xF },
        Op::LeeUav { d: 10, u: 1, modo: Modo::Estructurado, i: 1, desp: 9, z: 9 },
        Op::EscribeUav { u: 1, modo: Modo::Estructurado, i: 0, desp: 2, z: 9, v: [10, 11, 12, 13], mascara: 0xF },
    ];
    let mut ini = vec![0u32; 14];
    ini[2] = 0xFFFF_FFF0;
    ini[9] = 0xFFFF_FFF8;
    let p = programa(ops, ini, 2, [32, 1, 1]);
    let vistas = [
        Vista { bytes: palabras(&[7; 8]), modo: Modo::Crudo, elementos: 8, paso: 0 },
        Vista { bytes: palabras(&[9; 12]), modo: Modo::Estructurado, elementos: 3, paso: 16 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas, 1);
    assert_eq!(gpu, casa, "todo fuera: nada se lee ni se escribe, y nada se sale");
    // El tope MAL puesto: 64 bytes mas alla. Sin la sujecion bien cargada,
    // se sale (y el simulador lo dice).
    let e = emitir_con(&p, 64, Abi::Registros).unwrap();
    let mut mem = Ventanas(vistas.iter().map(|v| v.bytes.clone()).collect());
    let r = correr_grupo(&e.codigo, [32, 1, 1], [0; 3], [&[]; 8], &mut [], &mut mem, |_, m| {
        for q in &e.precargas {
            if let Precarga::Uav { u, que, reg } = *q {
                let v = &vistas[u as usize];
                let x = que.valor(BASE + u as u64 * RANURA, v.bytes.len() as u32 + 64, v.elementos + 16, v.paso).unwrap();
                m.r[reg as usize] = x as u32;
                if que == DelUav::Base {
                    m.r[reg as usize + 1] = (x >> 32) as u32;
                }
            }
        }
    });
    assert!(matches!(r, Err(crate::simula::NoSimula::Fallo(_))), "{r:?}");
}

/// Lo que no va todavia, con su indice (por la CPU): un UAV con tipo, la
/// barrera dentro de un `si`, y lo de computo en un programa que no lo es.
#[test]
fn lo_que_no_va_todavia() {
    let tipado = programa(vec![Op::LeeUav { d: 0, u: 0, modo: Modo::Tipado, i: 4, desp: 4, z: 4 }], vec![0; 5], 1, [1, 1, 1]);
    assert_eq!(emitir_con(&tipado, 64, Abi::Registros).err(), Some(NoEmite::Operacion(0)));
    // La barrera bajo un `si` del id del HILO: no se demuestra que lleguen
    // todos (por la CPU); bajo uno del cbuffer o del id del GRUPO, si.
    let en_si = programa(vec![Op::IdHilo { d: 0, que: 2, c: 0 }, Op::Si { c: 0 }, Op::Barrera, Op::FinSi], vec![1; 1], 0, [2, 1, 1]);
    assert_eq!(emitir_con(&en_si, 64, Abi::Registros).err(), Some(NoEmite::Operacion(2)));
    let de_grupo = programa(vec![Op::IdHilo { d: 0, que: 1, c: 0 }, Op::Si { c: 0 }, Op::Barrera, Op::FinSi], vec![1; 1], 0, [2, 1, 1]);
    assert!(emitir_con(&de_grupo, 64, Abi::Registros).is_ok());
    // Un bucle que sale por algo del hilo: tampoco (no dan las mismas vueltas).
    let bucle = programa(vec![Op::IdHilo { d: 0, que: 2, c: 0 }, Op::Bucle, Op::Barrera, Op::RomperSi { c: 0, si_cero: false }, Op::FinBucle], vec![1; 1], 0, [2, 1, 1]);
    assert_eq!(emitir_con(&bucle, 64, Abi::Registros).err(), Some(NoEmite::Operacion(2)));
    // Lo escrito dentro de un `si` del hilo deja de ser uniforme despues.
    let tras = programa(vec![Op::IdHilo { d: 0, que: 2, c: 0 }, Op::Si { c: 0 }, Op::Copia { d: 1, a: 2 }, Op::FinSi, Op::Si { c: 1 }, Op::Barrera, Op::FinSi], vec![0, 0, 1], 0, [2, 1, 1]);
    assert_eq!(emitir_con(&tras, 64, Abi::Registros).err(), Some(NoEmite::Operacion(5)));
    let dibujo = programa(vec![Op::IdHilo { d: 0, que: 0, c: 0 }], vec![0; 1], 0, [0; 3]);
    assert_eq!(emitir_con(&dibujo, 64, Abi::Registros).err(), Some(NoEmite::Operacion(0)));
}

/// ** EL JUEZ DEL KERNEL (R0..R7) con lo que emite el computo: los cuerpos
/// de arriba pasan -- PERFECTO Y PRECISO, y R7 con sus UAV --; y un cuerpo
/// tramposo, no: otro tope, la base pisada, un salto dentro de la cadena,
/// un LDG sin cadena, un inmediato en la direccion.
#[test]
fn el_juez_deja_lo_emitido_y_no_las_trampas() {
    use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_con, juzgar_drenado, Contexto, Regla, RESERVADOS};
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::IdHilo { d: 10, que: 3, c: 0 },
        Op::Barrera,
        Op::Entera { d: 1, a: 0, b: 2, op: OpEntera::Shl },
        Op::LeeUav { d: 3, u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9 },
        Op::LeeUav { d: 11, u: 1, modo: Modo::Estructurado, i: 0, desp: 9, z: 9 },
        Op::Mul { d: 7, a: 3, b: 8 },
        Op::EscribeUav { u: 1, modo: Modo::Estructurado, i: 10, desp: 9, z: 9, v: [7, 11, 12, 13], mascara: 0xF },
    ];
    let mut ini = vec![0u32; 15];
    ini[2] = 2;
    ini[8] = 2.0f32.to_bits();
    let p = programa(ops, ini, 2, [8, 2, 1]);
    let e = emitir_con(&p, 64, Abi::Registros).unwrap();
    let permisos = crate::permisos_computo(&e, &p).unwrap();
    assert_eq!(permisos.uavs.n, 2);
    let ctx = Contexto { registros: e.registros + RESERVADOS, sph: None };
    juzgar_drenado(&e.codigo, &ctx).unwrap_or_else(|b| panic!("R0..R6: {b}"));
    juzgar_cuerpo_con(&e.codigo, e.registros, permisos).unwrap_or_else(|b| panic!("R7: {b}"));
    // Sin computo ni UAV (los permisos de un dibujo): no.
    assert_eq!(juzgar_cuerpo_con(&e.codigo, e.registros, Default::default()).unwrap_err().regla, Regla::R7CuerpoAjeno);
    let r7 = |c: &[(u64, u64)]| juzgar_cuerpo_con(c, e.registros, permisos).err().map(|b| b.detalle);
    let ldg = e.codigo.iter().position(|w| w.0 & 0x1FF == 0x181).unwrap();
    // Otro tope en el IMNMX (el del otro UAV).
    let mut t = e.codigo.clone();
    let otro = permisos.uavs.tope[1] as u64;
    t[ldg - 3].0 = t[ldg - 3].0 & !(0xFF << 32) | otro << 32;
    assert!(r7(&t).is_some_and(|d| d.contains("tope de SU UAV")), "{:?}", r7(&t));
    // Un inmediato en la direccion del LDG.
    let mut t = e.codigo.clone();
    t[ldg].0 |= 0x40 << 40;
    assert!(r7(&t).is_some());
    // El LOP3 que alinea, quitado (un MOV en su sitio).
    let mut t = e.codigo.clone();
    let a = (t[ldg].0 >> 24 & 0xFF) as u8;
    t[ldg - 4] = bmo_sm86::codifica::mov(a, bmo_sm86::codifica::r(a), t[ldg - 4].1 >> 41);
    assert!(r7(&t).is_some_and(|d| d.contains("LOP3")), "{:?}", r7(&t));
    // La base pisada por el cuerpo.
    let mut t = e.codigo.clone();
    let b = permisos.uavs.base[0];
    t.insert(0, bmo_sm86::codifica::mov(b, bmo_sm86::codifica::Fuente::Imm(0x1000), 0));
    assert!(r7(&t).is_some_and(|d| d.contains("base o el tope")), "{:?}", r7(&t));
    // Un salto que cae dentro de la cadena.
    let mut t = e.codigo.clone();
    t.insert(0, bmo_sm86::codifica::bra(7, 16 * (ldg as i64 - 1), 0));
    assert!(r7(&t).is_some_and(|d| d.contains("salto")), "{:?}", r7(&t));
}

/// ** LA COMPARTIDA: cada hilo escribe su id en `a[t]` (array 0, de 12, en
/// la palabra 3: hay 4 hilos de mas que se salen), barrera, y lee el de su
/// espejo `a[11 - t]`; ademas suma 1 a un contador (array 1, palabra 1) con
/// un atomico y lee lo que quedo en una palabra que nadie escribe (el
/// prologo: 0, aunque la 3060 empiece con basura). 2 grupos de 16.
#[test]
fn la_compartida_con_su_barrera_y_sus_atomicos() {
    let ops = vec![
        Op::IdHilo { d: 0, que: 3, c: 0 },
        Op::IdHilo { d: 1, que: 0, c: 0 },
        Op::EscribeCompartida { base: 3, n: 12, i: 0, s: 1 },
        Op::AtomicoCompartido { d: 2, base: 1, n: 1, i: 9, v: 10, como: bmo_proton_x::bufer::Atomo::Suma },
        Op::Barrera,
        Op::SumaEntera { d: 4, a: 11, b: 12 },
        Op::Entera { d: 5, a: 4, b: 0, op: OpEntera::Resta },
        Op::LeeCompartida { d: 6, base: 3, n: 12, i: 5 },
        Op::LeeCompartida { d: 7, base: 1, n: 1, i: 9 },
        Op::LeeCompartida { d: 8, base: 0, n: 1, i: 9 },
        Op::Entera { d: 13, a: 1, b: 14, op: OpEntera::Shl },
        Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 13, desp: 9, z: 9, v: [6, 7, 8, 2], mascara: 0xF },
    ];
    let mut ini = vec![0u32; 15];
    ini[10] = 1;
    ini[11] = 11;
    ini[14] = 4;
    let p = programa_con(ops, ini, 1, [16, 1, 1], 15);
    let vistas = [Vista { bytes: vec![0x77; 16 * 32], modo: Modo::Crudo, elementos: 4 * 32, paso: 0 }];
    let (casa, gpu) = las_dos(&p, &vistas, 2);
    assert_eq!(gpu, casa);
    let w = |k: usize| u32::from_le_bytes(gpu[0][4 * k..4 * k + 4].try_into().unwrap());
    // El hilo 2 del grupo 1 (el 18): lee a[9] = 25; el contador, 16; la palabra 0, 0; su atomico, 2.
    assert_eq!([w(4 * 18), w(4 * 18 + 1), w(4 * 18 + 2), w(4 * 18 + 3)], [25, 16, 0, 2]);
    // El 14 (t = 14): a[11 - 14] se sale: 0.
    assert_eq!(w(4 * 14), 0);
}

/// El juez con la compartida: el tope del IMNMX mas alla de lo que el
/// kernel da al grupo, no; y sin el IMAD por 4, no.
#[test]
fn el_juez_y_la_compartida() {
    use bmo_gpu_ga10x::sass::juez::juzgar_cuerpo_con;
    let ops = vec![Op::IdHilo { d: 0, que: 3, c: 0 }, Op::EscribeCompartida { base: 0, n: 8, i: 0, s: 0 }, Op::Barrera, Op::LeeCompartida { d: 1, base: 0, n: 8, i: 2 }, Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 2, desp: 2, z: 2, v: [1, 1, 1, 1], mascara: 1 }];
    let p = programa_con(ops, vec![0; 3], 1, [8, 1, 1], 8);
    let e = emitir_con(&p, 64, Abi::Registros).unwrap();
    let mut permisos = crate::permisos_computo(&e, &p).unwrap();
    assert!(juzgar_cuerpo_con(&e.codigo, e.registros, permisos).is_ok());
    permisos.compartida = 28;
    let b = juzgar_cuerpo_con(&e.codigo, e.registros, permisos).unwrap_err();
    assert!(b.detalle.contains("salir de la compartida"), "{b}");
    permisos.compartida = 32;
    let lds = e.codigo.iter().position(|w| w.0 & 0x1FF == 0x184).unwrap();
    let mut t = e.codigo.clone();
    let a = (t[lds].0 >> 24 & 0xFF) as u8;
    t[lds - 1] = bmo_sm86::codifica::mov(a, bmo_sm86::codifica::r(a), t[lds - 1].1 >> 41);
    assert!(juzgar_cuerpo_con(&t, e.registros, permisos).unwrap_err().detalle.contains("IMAD por 4"));
}

/// ** LOS ATOMICOS DE UN UAV y GetDimensions: cada hilo suma `id + 1` a la
/// palabra `4 * (id % 3)` de un crudo de 2 palabras (la de 8 se sale: 0 y
/// nada), un MAX con signo de `-id` en el elemento 1 de un estructurado, y
/// un CAS (si es 0, su id) en el elemento 0; y escribe lo de antes de cada
/// uno y las medidas de los dos. Hilo a hilo, en el orden de la casa: los
/// mismos bytes.
#[test]
fn los_atomicos_de_un_uav_y_sus_medidas() {
    use bmo_proton_x::bufer::Atomo;
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::Entera { d: 1, a: 0, b: 2, op: OpEntera::RemU },
        Op::Entera { d: 1, a: 1, b: 3, op: OpEntera::Shl },
        Op::SumaEntera { d: 4, a: 0, b: 5 },
        Op::Atomico { d: 6, u: 0, modo: Modo::Crudo, i: 1, desp: 9, z: 9, como: Atomo::Suma, v: 4, igual: 9 },
        Op::Entera { d: 7, a: 9, b: 0, op: OpEntera::Resta },
        Op::Atomico { d: 8, u: 1, modo: Modo::Estructurado, i: 5, desp: 9, z: 9, como: Atomo::MaxConSigno, v: 7, igual: 9 },
        Op::Atomico { d: 10, u: 1, modo: Modo::Estructurado, i: 9, desp: 9, z: 9, como: Atomo::CambiaSiIgual, v: 0, igual: 9 },
        Op::MedidasUav { d: 11, u: 0, modo: Modo::Crudo },
        Op::MedidasUav { d: 15, u: 1, modo: Modo::Estructurado },
        Op::Entera { d: 19, a: 0, b: 20, op: OpEntera::Shl },
        Op::EscribeUav { u: 2, modo: Modo::Crudo, i: 19, desp: 9, z: 9, v: [6, 8, 10, 11], mascara: 0xF },
        Op::EscribeUav { u: 2, modo: Modo::Crudo, i: 19, desp: 21, z: 9, v: [15, 15, 15, 15], mascara: 1 },
    ];
    let mut ini = vec![0u32; 22];
    ini[2] = 3;
    ini[3] = 2;
    ini[5] = 1;
    ini[20] = 5;
    ini[21] = 16;
    let p = programa(ops, ini, 3, [6, 1, 1]);
    let vistas = [
        Vista { bytes: palabras(&[100, 200]), modo: Modo::Crudo, elementos: 2, paso: 0 },
        Vista { bytes: palabras(&[0, 0xFFFF_FFF0, 0, 7]), modo: Modo::Estructurado, elementos: 2, paso: 8 },
        Vista { bytes: vec![0x33; 32 * 12], modo: Modo::Crudo, elementos: 8 * 12, paso: 0 },
    ];
    let (casa, gpu) = las_dos(&p, &vistas, 2);
    assert_eq!(gpu, casa);
    let w = |u: usize, k: usize| u32::from_le_bytes(gpu[u][4 * k..4 * k + 4].try_into().unwrap());
    assert_eq!([w(0, 0), w(0, 1)], [100 + 1 + 4 + 7 + 10, 200 + 2 + 5 + 8 + 11], "las sumas");
    assert_eq!(w(1, 0), 1, "el CAS: el hilo 0 pone 0 (era 0), el 1 pone 1, y ya nadie mas");
    assert_eq!(w(2, 8 * 2 + 3), 8, "GetDimensions de un crudo de 2 palabras: 8 bytes");
    assert_eq!(w(2, 8 * 2 + 4), 2, "y del estructurado, sus elementos");
}

/// *** EL CS DE nBodyGravity (Microsoft, MIT; `proton-x/prueba/muestras/
/// nbody/`), CORRIDO: 128 hilos, 512 palabras compartidas, sus dos barreras
/// dentro del bucle de los tiles (bajo un `si` del cbuffer: uniforme), el
/// SRV de las particulas y el UAV de las nuevas. 256 particulas, 2 grupos:
/// los MISMOS bytes que la CPU, y juzgado R0..R7.
#[test]
fn el_cs_de_nbody_corre_igual() {
    let p = bmo_proton_x::dxil::computo::preparar(include_bytes!("../../proton-x/prueba/muestras/nbody/nBodyGravityCS.cso")).unwrap().programa;
    assert_eq!(p.computo.hilos, [128, 1, 1]);
    let n = 256usize;
    // pos (x, y, z, 1) y velo (vx, vy, vz, 0) de cada particula: 32 bytes.
    let particulas: Vec<u32> = (0..n)
        .flat_map(|k| {
            let f = k as f32;
            [(f * 0.37).sin() * 10.0, (f * 0.11).cos() * 10.0, f * 0.05 - 6.0, 1.0, 0.01 * f, -0.02, 0.0, 0.0].map(f32::to_bits)
        })
        .collect();
    let srv = Vista { bytes: palabras(&particulas), modo: Modo::Estructurado, elementos: n as u32, paso: 32 };
    let uav = Vista { bytes: vec![0; 32 * n], modo: Modo::Estructurado, elementos: n as u32, paso: 32 };
    // g_param = (particulas, tiles, ...), g_paramf = (dt, amortiguar, ...).
    let mut cb = vec![0u8; 16 * p.filas_cb.max(2) as usize];
    for (k, w) in [n as u32, (n / 128) as u32, 0, 0, 0.1f32.to_bits(), 1.0f32.to_bits(), 0, 0].iter().enumerate() {
        cb[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
    let (casa, gpu) = las_dos_con(&p, &[uav], &[srv], &cb, 2);
    assert!(casa[0].iter().any(|&b| b != 0), "la CPU escribio las particulas nuevas");
    assert_eq!(gpu, casa, "los mismos bytes, particula a particula");
}


/// *** EL CAMINO DEL KERNEL, ENTERO Y PURO: nBody emitido -> su receta VRC1
/// -> el kernel la LEE, saca los permisos y JUZGA el cuerpo (R0..R7), pone
/// los VALORES de sus precargas (con la VA que le dio a cada bufer) y lo
/// PEGA -> la 3060 de mentira corre lo pegado, sin que la prueba le cargue
/// nada -> los mismos bytes que la CPU.
#[test]
fn nbody_por_el_camino_del_kernel() {
    use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_con, juzgar_drenado, Contexto, RESERVADOS};
    use bmo_gpu_ga10x::trabajos::{computo_app, receta_computo as rc};
    let p = bmo_proton_x::dxil::computo::preparar(include_bytes!("../../proton-x/prueba/muestras/nbody/nBodyGravityCS.cso")).unwrap().programa;
    let n = 256usize;
    let particulas: Vec<u32> = (0..n).flat_map(|k| [(k as f32 * 0.3).sin(), k as f32 * 0.01, 1.0, 1.0, 0.0, 0.0, 0.5, 0.0].map(f32::to_bits)).collect();
    let srv = Vista { bytes: palabras(&particulas), modo: Modo::Estructurado, elementos: n as u32, paso: 32 };
    let uav = Vista { bytes: vec![0; 32 * n], modo: Modo::Estructurado, elementos: n as u32, paso: 32 };
    let filas = [[n as u32, (n / 128) as u32, 0, 0], [0.1f32.to_bits(), 1.0f32.to_bits(), 0, 0]];
    let cb: Vec<u8> = filas.iter().flatten().flat_map(|w| w.to_le_bytes()).collect();
    // La CPU.
    let (casa, _) = las_dos_con(&p, core::slice::from_ref(&uav), core::slice::from_ref(&srv), &cb, 2);
    // PROTON-X: emitir y armar la receta (las VA del PROCESO, que el kernel
    // no usa para la 3060: le da otras).
    let e = emitir_con(&p, 248, Abi::Registros).unwrap();
    let buferes = [rc::Bufer { va: 0x7000_0000, bytes: uav.bytes.len() as u32, elementos: n as u32, paso: 32, ranura: 0 }, rc::Bufer { va: 0x7100_0000, bytes: srv.bytes.len() as u32, elementos: n as u32, paso: 32, ranura: crate::SRV }];
    let mut bytes = vec![0u8; 512 * 1024];
    let largo = crate::receta_computo(&mut bytes, &e, &p, [2, 1, 1], &buferes, &filas).unwrap();
    // EL KERNEL: leer, juzgar, valores, pegar.
    let r = rc::leer(&bytes[..largo]).unwrap_or_else(|x| panic!("{}", x.texto()));
    let cuerpo: Vec<(u64, u64)> = (0..r.instrucciones()).map(|k| r.instruccion(k)).collect();
    juzgar_cuerpo_con(&cuerpo, r.despacho.registros, rc::permisos(&r).unwrap()).unwrap_or_else(|b| panic!("R7: {b}"));
    let va_3060 = |k: usize| BASE + if r.bufer(k).solo_lee() { 8 + (r.bufer(k).ranura & !rc::SRV) as u64 } else { r.bufer(k).ranura as u64 } * RANURA;
    let mut v = [(0u8, 0u32); 64];
    let nv = rc::valores(&r, va_3060, &mut v).unwrap();
    let mut pegado = vec![(0u64, 0u64); r.instrucciones() + nv];
    let np = computo_app::pegar(&mut pegado, &v[..nv], &cuerpo).unwrap();
    juzgar_drenado(&pegado[..np], &Contexto { registros: r.despacho.registros + RESERVADOS, sph: None }).unwrap_or_else(|b| panic!("R0..R6 del pegado: {b}"));
    computo_app::qmd(0x2_0000_0000, &r.despacho, 0, 0).unwrap();
    // La 3060 de mentira, con lo pegado y nada mas.
    let mut ventanas = vec![uav.bytes.clone()];
    ventanas.resize(8, Vec::new());
    ventanas.push(srv.bytes.clone());
    let mut mem = Ventanas(ventanas);
    for gx in 0..2 {
        let mut compartida = vec![0x5Au8; r.despacho.compartida as usize];
        correr_grupo(&pegado[..np], r.despacho.hilos, [gx, 0, 0], [&[]; 8], &mut compartida, &mut mem, |_, _| {}).unwrap_or_else(|x| panic!("{x:?}"));
    }
    assert_eq!(mem.0[0], casa[0], "los mismos bytes que la CPU, por el camino del kernel");
}

/// ** EL CONTADOR (`Append` y `Consume`): 2 grupos de 8; cada hilo sube el
/// contador (da el de antes: su indice), escribe su id en esa palabra, y
/// lo baja (da el de despues) y lo escribe en la de al lado + 16. Hilo a
/// hilo, en el orden de la casa: los mismos bytes y el mismo contador. El
/// contador vive en su ventana (la ranura `CONTADOR | 0`).
#[test]
fn el_contador_append_y_consume() {
    use bmo_proton_x::bufer::Rebanadas;
    let ops = vec![
        Op::IdHilo { d: 0, que: 0, c: 0 },
        Op::Contador { d: 1, u: 0, inc: 1 },
        Op::Entera { d: 2, a: 1, b: 3, op: OpEntera::Shl },
        Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 2, desp: 4, z: 4, v: [0, 0, 0, 0], mascara: 1 },
        Op::Contador { d: 5, u: 0, inc: -1 },
        Op::Entera { d: 6, a: 5, b: 3, op: OpEntera::Shl },
        Op::EscribeUav { u: 0, modo: Modo::Crudo, i: 6, desp: 7, z: 4, v: [1, 1, 1, 1], mascara: 1 },
    ];
    let mut ini = vec![0u32; 8];
    ini[3] = 2;
    ini[7] = 64;
    let p = programa(ops, ini, 1, [8, 1, 1]);
    // La CPU, con su contador a 5.
    let mut casa = vec![0x11u8; 4 * 40];
    let mut contador_casa = 5u32;
    {
        let mut uavs = [Some(Uav { bytes: &mut casa, formato: 0, paso: 0, elementos: 40, contador: Some(&mut contador_casa), rebanadas: Rebanadas::PLANA })];
        let rec = Recursos { texturas: &[], muestreadores: &[], buferes: &[], dinamicas: None };
        p.despachar([2, 1, 1], &[], &rec, &mut uavs);
    }
    // La 3060 de mentira: el UAV en la ranura 0 y su contador en la 1.
    let e = emitir_con(&p, 64, Abi::Registros).unwrap_or_else(|x| panic!("{x:?}"));
    {
        use bmo_gpu_ga10x::sass::juez::{juzgar_cuerpo_con, juzgar_drenado, Contexto, RESERVADOS};
        juzgar_drenado(&e.codigo, &Contexto { registros: e.registros + RESERVADOS, sph: None }).unwrap_or_else(|b| panic!("R0..R6: {b}"));
        juzgar_cuerpo_con(&e.codigo, e.registros, crate::permisos_computo(&e, &p).unwrap()).unwrap_or_else(|b| panic!("R7: {b}"));
    }
    let mut mem = Ventanas(vec![vec![0x11u8; 4 * 40], 5u32.to_le_bytes().to_vec()]);
    for gx in 0..2 {
        correr_grupo(&e.codigo, [8, 1, 1], [gx, 0, 0], [&[]; 8], &mut [], &mut mem, |_, m| {
            for q in &e.precargas {
                if let Precarga::Uav { u, que, reg } = *q {
                    let (ranura, bytes, elementos) = if u & crate::CONTADOR != 0 { (1u64, 4, 1) } else { (0, 160, 40) };
                    let x = que.valor(BASE + ranura * RANURA, bytes, elementos, 0).unwrap();
                    m.r[reg as usize] = x as u32;
                    if que == DelUav::Base {
                        m.r[reg as usize + 1] = (x >> 32) as u32;
                    }
                }
            }
        })
        .unwrap_or_else(|x| panic!("{x:?}"));
    }
    assert_eq!(mem.0[0], casa, "los mismos bytes");
    assert_eq!(u32::from_le_bytes(mem.0[1][..4].try_into().unwrap()), contador_casa, "el mismo contador al final");
    assert_eq!(contador_casa, 5, "cada hilo sube y baja");
}
