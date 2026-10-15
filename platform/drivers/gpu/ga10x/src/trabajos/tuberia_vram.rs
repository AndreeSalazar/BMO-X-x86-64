//! Las pruebas de la tuberia sobre una VRAM de mentira (lo que `preparar`
//! deja escrito): el caliente contra el frio, y el CODIGO de LI2b. Salieron
//! de `tuberia.rs` por L6a (12-10) moviendo texto.

extern crate std;

use super::tuberia::*;
use crate::raster::SPH;
use crate::Registros;

/// La VRAM por la ventana PRAMIN: lo escrito, por direccion.
struct Vram(std::collections::BTreeMap<u64, u32>, u32);
impl Registros for Vram {
    fn leer(&mut self, reg: u32) -> u32 {
        if reg == crate::vram::VENTANA_REG {
            return self.1;
        }
        *self.0.get(&(((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64)).unwrap_or(&0)
    }
    fn escribir(&mut self, reg: u32, v: u32) {
        if reg == crate::vram::VENTANA_REG {
            self.1 = v;
            return;
        }
        self.0.insert(((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64, v);
    }
}

/// ** El caliente con OTRA cuenta de triangulos deja la VRAM como la
/// dejaria el frio con esa cuenta: las ordenes, los datos y la entrada.
/// Lo unico distinto es lo que sobra del fotograma anterior MAS ALLA de lo
/// que este usa (sus vertices de mas), que nadie lee: la cuenta lo dice.
#[test]
fn el_caliente_con_otra_cuenta_es_el_frio() {
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let v = crate::cubo::ventana(&gop).unwrap();
    let mut vs = [0u8; 4 * PALABRAS_VS];
    let mut ps = [0u8; 4 * PALABRAS_PS];
    bytes(&vertice(), &mut vs);
    bytes(&pixel(), &mut ps);
    let muchos: std::vec::Vec<u8> = (0..18 * BYTES_VERTICE).map(|k| (k * 7 + 1) as u8).collect();
    let pocos: std::vec::Vec<u8> = (0..12 * BYTES_VERTICE).map(|k| (k * 5 + 3) as u8).collect();
    fn p<'a>(vs: &'a [u8], ps: &'a [u8], x: &'a [u8]) -> Paquete<'a> {
        Paquete { ficha: 1, vs, ps, vertices: x, n: x.len() / BYTES_VERTICE, limpiar: None, dibujo: Dibujo::default() }
    }
    let (a, b) = (p(&vs, &ps, &muchos), p(&vs, &ps, &pocos));
    assert_eq!(huella_fija(&v, &a, false), huella_fija(&v, &b, false));
    // Frio con 6 triangulos, y caliente con 4.
    let mut r = Vram(Default::default(), 0);
    assert!(preparar_con(&mut r, 3, &v, &a, false));
    assert!(preparar_caliente(&mut r, 3, &v, &b, false));
    // Frio con 4, de cero.
    let mut f = Vram(Default::default(), 0);
    assert!(preparar_con(&mut f, 3, &v, &b, false));
    let datos_de_mas = DATOS + pocos.len() as u64..DATOS + muchos.len() as u64;
    let mut iguales = 0;
    for (&dir, &x) in &f.0 {
        assert_eq!(r.0.get(&dir).copied().unwrap_or(0), x, "VRAM {dir:#x}: el caliente con otra cuenta no es el frio");
        iguales += 1;
    }
    for (&dir, &x) in &r.0 {
        if !f.0.contains_key(&dir) && x != 0 {
            assert!(datos_de_mas.contains(&dir), "VRAM {dir:#x} = {x:#x}: el caliente dejo algo que el frio no");
        }
    }
    assert!(iguales > 1000, "la comparacion tiene que cubrir las paginas del frio");
}

/// ** LI2b (12-10): un programa LARGO (4096 instrucciones) va entero a su
/// region (`vram::CODIGO`), y uno corto detras de el deja su ultima
/// pagina a cero -- lo del largo de antes no queda tras su EXIT -- sin
/// tocar las paginas que no usa. Las ordenes apuntan alli.
#[test]
fn un_programa_largo_en_su_region() {
    let largo: std::vec::Vec<u8> = (0..HUECO).map(|k| (k % 251) as u8 | 1).collect();
    let mut r = Vram(Default::default(), 0);
    assert!(crate::pluma::Pluma::nueva(&mut r).programa(VS, &largo));
    let palabra = |r: &Vram, d: u64| r.0.get(&d).copied().unwrap_or(0);
    assert_eq!(palabra(&r, VS + HUECO as u64 - 4), u32le(&largo, HUECO - 4), "la ultima palabra del largo");
    assert!(VS + HUECO as u64 <= PS, "no pisa el de pixel");
    let corto = [0x11u8; 4 * SPH + 16 * 40];
    assert!(crate::pluma::Pluma::nueva(&mut r).programa(VS, &corto));
    let fin = VS + corto.len() as u64;
    assert!((fin..fin.next_multiple_of(0x1000)).step_by(4).all(|d| palabra(&r, d) == 0), "su ultima pagina, a cero tras el");
    assert_eq!(palabra(&r, VS + 0x1000 * 5), u32le(&largo, 0x1000 * 5), "las paginas que no usa, sin tocar (nadie las lee)");
    // Las ordenes, a su VA (la palabra baja de `SET_PIPELINE_SHADER` + 0x14).
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let o = ordenes(&crate::cubo::ventana(&gop).unwrap(), 1);
    let w = &o.o[..o.n];
    for (j, va) in [(crate::raster::VERTICE, VS_VA), (crate::raster::PIXEL, PS_VA)] {
        let c = crate::copia::cabecera_en(0, crate::raster::set_pipeline_shader(j) + 0x14, 2);
        assert!(w.windows(3).any(|q| q[0] == c && q[1] == (va >> 32) as u32 && q[2] == va as u32), "el programa {j} en {va:#x}");
    }
}


/// ** El PORQUE (12-10): una VRAM que no guarda lo escrito en un tramo (como
/// una ventana PRAMIN que no llega) hace fallar `preparar_paso` en EL paso
/// que escribe alli, y lo dice con su numero de `porque::PASOS`.
#[test]
fn preparar_dice_el_paso_que_fallo() {
    struct Sorda(Vram, core::ops::Range<u64>);
    impl Registros for Sorda {
        fn leer(&mut self, reg: u32) -> u32 {
            self.0.leer(reg)
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            let dir = ((self.0 .1 as u64) << 16) + reg.wrapping_sub(crate::vram::VENTANA) as u64;
            if reg == crate::vram::VENTANA_REG || !self.1.contains(&dir) {
                self.0.escribir(reg, v);
            }
        }
    }
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let v = crate::cubo::ventana(&gop).unwrap();
    let mut vs = [0u8; 4 * PALABRAS_VS];
    let mut ps = [0u8; 4 * PALABRAS_PS];
    bytes(&vertice(), &mut vs);
    bytes(&pixel(), &mut ps);
    let x: std::vec::Vec<u8> = (0..6 * BYTES_VERTICE).map(|k| (k * 7 + 1) as u8).collect();
    let p = Paquete { ficha: 1, vs: &vs, ps: &ps, vertices: &x, n: 6, limpiar: None, dibujo: Dibujo::default() };
    assert_eq!(preparar_paso(&mut Vram(Default::default(), 0), 3, &v, &p, false, &mut crate::pluma::Escritos::default()), Ok(()));
    for (tramo, paso) in [(VS..VS + 4, 4u8), (PS..PS + 4, 5), (DATOS..DATOS + 4, 8)] {
        let r = preparar_paso(&mut Sorda(Vram(Default::default(), 0), tramo.clone()), 3, &v, &p, false, &mut crate::pluma::Escritos::default());
        assert_eq!(r, Err(paso), "el tramo {tramo:#x?}: {}", crate::porque::PASOS[paso as usize]);
    }
    let vacio = Paquete { n: 0, ..p };
    assert_eq!(preparar_paso(&mut Vram(Default::default(), 0), 3, &v, &vacio, false, &mut crate::pluma::Escritos::default()), Err(0));
}

/// ** Q0a1b (12-10): un destino que empieza a mitad de pagina (el mip de
/// 80x45 de Cyberpunk, en ...d400): el color va a `destino::VA` + lo que
/// se mete en la pagina, y la huella cambia con eso (el caliente no
/// reusa unas ordenes de otro sitio).
#[test]
fn q0a1b_el_destino_a_mitad_de_pagina() {
    use crate::tresde as td;
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let v = crate::cubo::ventana(&gop).unwrap();
    let dst = crate::destino::Destino { fila: 384, ancho: 80, alto: 45, flotante: crate::destino::Flotante::No, rgb: true };
    let ct = crate::copia::cabecera_en(0, td::SET_COLOR_TARGET_A0, 8);
    for (va, desp) in [(0x2_0012_3000u64, 0u64), (0x2_0012_3400, 0x400), (0x2_0012_3f80, 0xf80)] {
        assert!(dst.valido_en(va));
        let d = Dibujo { vertices: 3, destino: Some((va, dst)), ..Dibujo::default() };
        let o = ordenes_dibujo(&v, 1, true, d);
        let w = &o.o[..o.n];
        let i = w.iter().position(|&x| x == ct).unwrap();
        let base = crate::destino::VA + desp;
        assert_eq!(&w[i + 1..i + 3], &[(base >> 32) as u32, base as u32], "{va:#x}");
        assert_eq!(dst.paginas_en(va), (desp + 384 * 45).div_ceil(4096), "{va:#x}");
    }
    assert!(!dst.valido_en(0x2_0012_3410), "no a 128 B");
    let p = |va| Paquete { ficha: 1, vs: &[], ps: &[], vertices: &[], n: 3, limpiar: None, dibujo: Dibujo { vertices: 3, destino: Some((va, dst)), ..Dibujo::default() } };
    assert_ne!(huella_fija(&v, &p(0x2_0012_3400), false), huella_fija(&v, &p(0x2_0012_3000), false));
    assert_eq!(huella_fija(&v, &p(0x2_0012_3400), false), huella_fija(&v, &p(0x2_0099_9400), false), "la pagina no cuenta: la presta la IOMMU");
}

/// P3b4c.9 Z1: con `pantalla`, el color va a la ventana de la pantalla
/// (su VA y el formato del GOP), SIN la limpieza de VERRANO; la de la
/// app si, leida con el formato de SU back buffer (el destino).
#[test]
fn z1_el_back_buffer_va_a_la_pantalla() {
    use crate::tresde as td;
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let v = crate::cubo::ventana(&gop).unwrap();
    let dst = crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: true };
    let ct = crate::copia::cabecera_en(0, td::SET_COLOR_TARGET_A0, 8);
    let limpia = crate::copia::cabecera_en(0, td::CLEAR_SURFACE, 1);
    let valor = crate::copia::cabecera_en(0, td::SET_COLOR_CLEAR_VALUE0, 4);
    let d = Dibujo { vertices: 3, destino: Some((0x1000_0000, dst)), cadena: true, pantalla: true, ..Dibujo::default() };
    let o = ordenes_dibujo(&v, 1, true, d);
    let w = &o.o[..o.n];
    let i = w.iter().position(|&x| x == ct).unwrap();
    assert_eq!(&w[i + 1..i + 3], &[(v.va >> 32) as u32, v.va as u32], "la ventana de la pantalla, no la RAM");
    assert!(!w.contains(&limpia), "sin la limpieza de VERRANO");
    // La limpieza de la app: un pixel R8G8B8A8 (rojo) con su formato.
    let o = ordenes_dibujo(&v, 1, true, Dibujo { color: Some(0xFF00_00FF), ..d });
    let w = &o.o[..o.n];
    let k = w.iter().rposition(|&x| x == valor).unwrap();
    assert_eq!(&w[k + 1..k + 5], &color_de_limpieza(0xFF00_00FF, true), "leido como R8G8B8A8");
    assert_eq!(w.iter().filter(|&&x| x == limpia).count(), 1);
    // La huella: ir a la pantalla son OTRAS ordenes.
    let p = Paquete { ficha: 1, vs: &[], ps: &[], vertices: &[], n: 3, limpiar: None, dibujo: d };
    assert_ne!(huella_fija(&v, &p, true), huella_fija(&v, &Paquete { dibujo: Dibujo { pantalla: false, ..d }, ..p }, true));
}

/// P3b4c.6b: con Z, el color NO se queda en el pitch (Xid 69 en el
/// metal): el ultimo destino de color antes de la Z es la SOMBRA en
/// bloque, y lo que se limpia se limpia en ella, nunca en el pitch.
#[test]
fn con_z_el_color_va_a_la_sombra() {
    use crate::tresde as td;
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let v = crate::cubo::ventana(&gop).unwrap();
    let z = Some(crate::profundidad::Z { funcion: 2, escribir: true, limpiar: Some(crate::profundidad::UNO) });
    let dst = crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: true };
    let ct = crate::copia::cabecera_en(0, td::SET_COLOR_TARGET_A0, 8);
    let limpia = crate::copia::cabecera_en(0, td::CLEAR_SURFACE, 1);
    let zt = crate::copia::cabecera_en(0, crate::profundidad::SET_ZT_A, 5);
    for (d, limpias) in [
        (Dibujo { vertices: 3, z, ..Dibujo::default() }, 2),
        (Dibujo { vertices: 3, z, destino: Some((0x1000_0000, dst)), color: Some(0xFF00_0000), ..Dibujo::default() }, 2),
        (Dibujo { vertices: 3, z, destino: Some((0x1000_0000, dst)), ..Dibujo::default() }, 1),
    ] {
        let o = ordenes_dibujo(&v, 1, true, d);
        let w = &o.o[..o.n];
        let ultimo = w.iter().rposition(|&x| x == ct).unwrap();
        let i_zt = w.iter().position(|&x| x == zt).unwrap();
        assert!(ultimo < i_zt, "el color cambia ANTES de encender la Z");
        let s = crate::sombra::VA;
        let formato = if d.destino.is_some() { crate::cubo::FORMATO_RGB } else { td::FORMATO };
        assert_eq!(&w[ultimo + 1..ultimo + 9], &[(s >> 32) as u32, s as u32, 1280, 720, formato, crate::sombra::MEMORIA_BLOQUE, 1, (crate::sombra::BYTES >> 2) as u32]);
        let limpiezas: std::vec::Vec<usize> = w.iter().enumerate().filter(|&(_, &x)| x == limpia).map(|(k, _)| k).collect();
        assert_eq!(limpiezas.len(), limpias, "{d:?}");
        assert!(limpiezas.iter().all(|&k| k > ultimo), "nada se limpia en el pitch: {d:?}");
    }
}

/// ** E1 (14-10): el frio con la PLUMA. Un lote con 64 KiB de datos y dos
/// programas largos se prepara con unas pocas lecturas por el PCIe (antes,
/// una por palabra: ~25.000), deja la VRAM como antes, y el siguiente con
/// los mismos programas no los vuelve a escribir.
#[test]
fn e1_el_frio_casi_sin_lecturas() {
    struct Cuenta(Vram, usize, usize);
    impl Registros for Cuenta {
        fn leer(&mut self, reg: u32) -> u32 {
            self.1 += 1;
            self.0.leer(reg)
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            self.2 += 1;
            self.0.escribir(reg, v)
        }
    }
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let v = crate::cubo::ventana(&gop).unwrap();
    let vs: std::vec::Vec<u8> = (0..4 * SPH + 16 * 1500).map(|k| (k % 251) as u8 | 1).collect();
    let ps: std::vec::Vec<u8> = (0..4 * SPH + 16 * 900).map(|k| (k % 241) as u8 | 2).collect();
    let datos: std::vec::Vec<u8> = (0..DATOS_MAX).map(|k| (k * 7 + 1) as u8).collect();
    let p = Paquete { ficha: 1, vs: &vs, ps: &ps, vertices: &datos, n: 3, limpiar: None, dibujo: Dibujo::default() };
    let mut ya = crate::pluma::Escritos::NADA;
    let mut r = Cuenta(Vram(Default::default(), 0), 0, 0);
    assert_eq!(preparar_paso(&mut r, 3, &v, &p, false, &mut ya), Ok(()));
    assert!(r.1 <= 24, "{} lecturas", r.1);
    assert_eq!(ya, crate::pluma::Escritos { vs: crate::pluma::huella(&vs), ps: crate::pluma::huella(&ps) });
    for (k, q) in datos.chunks_exact(4).enumerate() {
        assert_eq!(r.0 .0[&(DATOS + 4 * k as u64)], u32le(q, 0));
    }
    assert_eq!(r.0 .0[&(PS + ps.len() as u64 - 4)], u32le(&ps, ps.len() - 4));
    let primeras = r.2;
    r.2 = 0;
    assert_eq!(preparar_paso(&mut r, 3, &v, &p, false, &mut ya), Ok(()));
    assert!(r.2 + (vs.len() + ps.len()) / 4 <= primeras, "los programas, no otra vez: {} contra {primeras}", r.2);
}


/// ** C1 (15-10): la LAMINA PRESTADA en el paquete y en `preparar`. El
/// paquete lleva 16 B de descriptor; el lector lo devuelve en `Dibujo` sin
/// datos; y lo que no se sostiene -- otra medida, sin direccion, con
/// indices -- no se lee.
#[test]
fn a_lent_lamina_travels_as_a_descriptor() {
    let mut vs = [0u8; 4 * PALABRAS_VS];
    let mut ps = [0u8; 4 * PALABRAS_PS];
    bytes(&vertice(), &mut vs);
    bytes(&pixel(), &mut ps);
    let d = Dibujo { vertices: 6, ..Dibujo::default() };
    let mut caja = [0u8; 4096];
    let total = escribir_paquete_prestado(&mut caja, 7, &vs, &ps, 0x1_0000_0040, 6, d).expect("se escribe");
    assert_eq!(total, CABECERA_1 + vs.len() + ps.len() + DESCRIPTOR_PRESTADA);
    let p = leer(&caja[..total]).expect("y se lee");
    assert_eq!(p.dibujo.prestada, Some(Prestada { va_app: 0x1_0000_0040, bytes: 6 * BYTES_VERTICE as u64, gpu_va: 0 }));
    assert!(p.vertices.is_empty(), "ni un vertice viaja en el paquete");
    assert_eq!(p.n, 6);
    // Lo que no se sostiene
    let roto = |f: &dyn Fn(&mut [u8])| {
        let mut b = caja;
        f(&mut b[..total]);
        leer(&b[..total]).is_none()
    };
    let datos = CABECERA_1 + vs.len() + ps.len();
    assert!(roto(&|b| b[datos + 8] = 0x41), "los bytes no son los de 6 vertices");
    assert!(roto(&|b| b[datos..datos + 8].fill(0)), "sin direccion");
    assert_eq!(escribir_paquete_prestado(&mut caja, 7, &vs, &ps, 0x1000, 6, Dibujo { indices: Some(0), ..d }), None, "con indices, no");
    assert_eq!(escribir_paquete_prestado(&mut caja, 7, &vs, &ps, 0, 6, d), None, "sin direccion, no");
}

/// ** Y `preparar`: la tabla apunta a la VA que dio el kernel y los DATOS de
/// la VRAM ni se tocan; sin VA, no se prepara a ciegas; en caliente, igual; y
/// prestada y copiada no comparten caliente (la huella las separa).
#[test]
fn preparar_points_the_table_at_the_lent_slot() {
    let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    let v = crate::cubo::ventana(&gop).unwrap();
    let mut vs = [0u8; 4 * PALABRAS_VS];
    let mut ps = [0u8; 4 * PALABRAS_PS];
    bytes(&vertice(), &mut vs);
    bytes(&pixel(), &mut ps);
    let sin_va = Dibujo { vertices: 6, prestada: Some(Prestada { va_app: 0x1_0000_0040, bytes: 192, gpu_va: 0 }), ..Dibujo::default() };
    let p = Paquete { ficha: 1, vs: &vs, ps: &ps, vertices: &[], n: 6, limpiar: None, dibujo: sin_va };
    assert_eq!(preparar_paso(&mut Vram(Default::default(), 0), 3, &v, &p, false, &mut crate::pluma::Escritos::default()), Err(0), "sin la VA del kernel, no");
    let gpu = 0xA_6000_0040u64;
    let p = Paquete { dibujo: Dibujo { prestada: Some(Prestada { gpu_va: gpu, ..sin_va.prestada.unwrap() }), ..sin_va }, ..p };
    let mut r = Vram(Default::default(), 0);
    assert_eq!(preparar_paso(&mut r, 3, &v, &p, false, &mut crate::pluma::Escritos::default()), Ok(()));
    let leer32 = |r: &mut Vram, dir: u64| {
        r.1 = (dir >> 16) as u32;
        r.0.get(&dir).copied().unwrap_or(0)
    };
    assert_eq!((leer32(&mut r, TABLA), leer32(&mut r, TABLA + 4)), (gpu as u32, (gpu >> 32) as u32), "la tabla, a la ranura prestada");
    assert!(r.0.range(DATOS..DATOS + 0x1000).next().is_none(), "los DATOS de la VRAM, ni tocados");
    // en caliente, la tabla se reescribe (otra ranura, otro sitio)
    let otra = 0xA_6000_1000u64;
    let q = Paquete { dibujo: Dibujo { prestada: Some(Prestada { gpu_va: otra, ..p.dibujo.prestada.unwrap() }), ..p.dibujo }, ..p };
    assert!(preparar_caliente(&mut r, 3, &v, &q, false));
    assert_eq!((leer32(&mut r, TABLA), leer32(&mut r, TABLA + 4)), (otra as u32, (otra >> 32) as u32));
    assert!(r.0.range(DATOS..DATOS + 0x1000).next().is_none());
    // la huella separa prestada de copiada
    let x: std::vec::Vec<u8> = (0..6 * BYTES_VERTICE).map(|k| k as u8).collect();
    let copiada = Paquete { vertices: &x, dibujo: Dibujo { prestada: None, ..p.dibujo }, ..p };
    assert_ne!(huella_fija(&v, &p, false), huella_fija(&v, &copiada, false));
    assert_eq!(huella_fija(&v, &p, false), huella_fija(&v, &q, false), "donde esta la ranura no es lo fijo");
}
