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
    assert!(escribir_programa(&mut r, VS, &largo));
    let palabra = |r: &Vram, d: u64| r.0.get(&d).copied().unwrap_or(0);
    assert_eq!(palabra(&r, VS + HUECO as u64 - 4), u32le(&largo, HUECO - 4), "la ultima palabra del largo");
    assert!(VS + HUECO as u64 <= PS, "no pisa el de pixel");
    let corto = [0x11u8; 4 * SPH + 16 * 40];
    assert!(escribir_programa(&mut r, VS, &corto));
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
    assert_eq!(preparar_paso(&mut Vram(Default::default(), 0), 3, &v, &p, false), Ok(()));
    for (tramo, paso) in [(VS..VS + 4, 4u8), (PS..PS + 4, 5), (DATOS..DATOS + 4, 8)] {
        let r = preparar_paso(&mut Sorda(Vram(Default::default(), 0), tramo.clone()), 3, &v, &p, false);
        assert_eq!(r, Err(paso), "el tramo {tramo:#x?}: {}", crate::porque::PASOS[paso as usize]);
    }
    let vacio = Paquete { n: 0, ..p };
    assert_eq!(preparar_paso(&mut Vram(Default::default(), 0), 3, &v, &vacio, false), Err(0));
}
