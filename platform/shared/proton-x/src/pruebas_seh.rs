//! Las pruebas de P4c: desenrollar un marco (`desenrollar`) y lo que decide el
//! despachador de excepciones (`seh`), con UNWIND_INFO y pilas escritas a mano
//! segun la documentacion de Microsoft ("x64 exception handling").

use alloc::vec;
use alloc::vec::Vec;

use crate::desenrollar::{self, Contexto, Funcion, Memoria, NoDesenrolla, Trozo, CONTEXT_BYTES, RAX, RBP, RBX, RDI, RSI, RSP, UNW_FLAG_EHANDLER, UNW_FLAG_UHANDLER};
use crate::seh::{self, AlDesenrollar, Ambito, Despacho, Imagen, Registro, Subida, Vectores};

const BASE: u64 = 0x1_4000_0000;
const PILA: u64 = 0x7_0000_0000;
const R14: usize = 14;

const PUSH_NONVOL: u8 = 0;
const ALLOC_LARGE: u8 = 1;
const ALLOC_SMALL: u8 = 2;
const SET_FPREG: u8 = 3;
const SAVE_NONVOL: u8 = 4;
const SAVE_XMM128: u8 = 8;
const PUSH_MACHFRAME: u8 = 10;
const CHAININFO: u8 = 4;

/// Un codigo de desenrollado: donde acaba su instruccion, que es y su dato.
fn cod(off: u8, op: u8, info: u8) -> u16 {
    off as u16 | (op as u16) << 8 | (info as u16) << 12
}

/// Una imagen de mentira que empieza en BASE.
struct Img(Vec<u8>);

impl Img {
    fn nueva() -> Img {
        Img(vec![0; 0x3000])
    }
    fn u16(&mut self, rva: u32, v: u16) {
        self.0[rva as usize..rva as usize + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, rva: u32, v: u32) {
        self.0[rva as usize..rva as usize + 4].copy_from_slice(&v.to_le_bytes());
    }
    /// Una UNWIND_INFO en `rva`: version 1, `banderas`, la medida del
    /// prologo, el registro de marco (y su desplazamiento en unidades de 16) y
    /// los codigos. Devuelve la RVA de lo que va detras (manejador o cadena).
    fn info(&mut self, rva: u32, banderas: u8, prologo: u8, marco: (u8, u8), codigos: &[u16]) -> u32 {
        self.0[rva as usize] = 1 | banderas << 3;
        self.0[rva as usize + 1] = prologo;
        self.0[rva as usize + 2] = codigos.len() as u8;
        self.0[rva as usize + 3] = marco.0 | marco.1 << 4;
        for (i, c) in codigos.iter().enumerate() {
            self.u16(rva + 4 + 2 * i as u32, *c);
        }
        rva + 4 + 2 * ((codigos.len() as u32 + 1) & !1)
    }
    /// Una RUNTIME_FUNCTION en `rva`.
    fn funcion(&mut self, rva: u32, inicio: u32, fin: u32, info: u32) -> Funcion {
        self.u32(rva, inicio);
        self.u32(rva + 4, fin);
        self.u32(rva + 8, info);
        Funcion { inicio, fin, desenrollar: info, dir: BASE + rva as u64 }
    }
}

/// La imagen y una pila: dos trozos de memoria.
struct Dos<'a>(Trozo<'a>, Trozo<'a>);

impl Memoria for Dos<'_> {
    fn u64_en(&self, d: u64) -> Option<u64> {
        self.0.u64_en(d).or_else(|| self.1.u64_en(d))
    }
    fn u32_en(&self, d: u64) -> Option<u32> {
        self.0.u32_en(d).or_else(|| self.1.u32_en(d))
    }
    fn u16_en(&self, d: u64) -> Option<u16> {
        self.0.u16_en(d).or_else(|| self.1.u16_en(d))
    }
    fn u8_en(&self, d: u64) -> Option<u8> {
        self.0.u8_en(d).or_else(|| self.1.u8_en(d))
    }
}

/// Una pila de 64 KiB en PILA, con estos u64 desde `desde`.
fn pila(desde: u64, v: &[u64]) -> Vec<u8> {
    let mut p = vec![0u8; 0x10000];
    for (i, x) in v.iter().enumerate() {
        let o = (desde - PILA) as usize + 8 * i;
        p[o..o + 8].copy_from_slice(&x.to_le_bytes());
    }
    p
}

fn ctx(rip: u64, rsp: u64) -> Contexto {
    let mut c = Contexto { rip, ..Contexto::default() };
    c.gp[RSP] = rsp;
    c
}

/// El prologo de `hondo` en seh.c: push r14, rsi, rdi, rbx y sub rsp, 72.
fn prologo_hondo(im: &mut Img) -> Funcion {
    im.info(0x2000, 0, 9, (0, 0), &[cod(9, ALLOC_SMALL, 8), cod(5, PUSH_NONVOL, RBX as u8), cod(4, PUSH_NONVOL, RDI as u8), cod(3, PUSH_NONVOL, RSI as u8), cod(2, PUSH_NONVOL, R14 as u8)]);
    im.funcion(0x2800, 0x1000, 0x1100, 0x2000)
}

#[test]
fn un_marco_con_pushes_y_reserva_devuelve_los_registros_de_quien_llamo() {
    let mut im = Img::nueva();
    let f = prologo_hondo(&mut im);
    let s = PILA + 0x1000;
    // 72 bytes suyos, y lo que el prologo apilo: rbx, rdi, rsi, r14 y la vuelta.
    let p = pila(s + 72, &[0xB0, 0xD1, 0x51, 0x14, BASE + 0x1234]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    let mut c = ctx(BASE + 0x1040, s);
    let marco = desenrollar::un_marco(&m, BASE, &f, &mut c, UNW_FLAG_EHANDLER).unwrap();
    assert_eq!(marco.establecido, s, "sin registro de marco, el establisher es el rsp del cuerpo");
    assert_eq!(marco.manejador, None);
    assert_eq!((c.gp[RBX], c.gp[RDI], c.gp[RSI], c.gp[R14]), (0xB0, 0xD1, 0x51, 0x14));
    assert_eq!(c.rip, BASE + 0x1234);
    assert_eq!(c.gp[RSP], s + 72 + 5 * 8);
}

#[test]
fn dentro_del_prologo_solo_se_deshace_lo_ya_hecho() {
    let mut im = Img::nueva();
    let f = prologo_hondo(&mut im);
    // En el desplazamiento 4: hechos push r14, push rsi y push rdi (el que
    // acaba en 4); ni push rbx ni la reserva.
    let s = PILA + 0x1000;
    let p = pila(s, &[0xD1, 0x51, 0x14, BASE + 0x1234]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    let mut c = ctx(BASE + 0x1004, s);
    c.gp[RBX] = 0x77;
    desenrollar::un_marco(&m, BASE, &f, &mut c, UNW_FLAG_EHANDLER).unwrap();
    assert_eq!((c.gp[RDI], c.gp[RSI], c.gp[R14], c.gp[RBX]), (0xD1, 0x51, 0x14, 0x77));
    assert_eq!((c.rip, c.gp[RSP]), (BASE + 0x1234, s + 32));
}

#[test]
fn con_registro_de_marco_el_establisher_sale_de_rbp_aunque_rsp_se_haya_movido() {
    let mut im = Img::nueva();
    // push rbp; sub rsp, 0x60; lea rbp, [rsp + 0x60]: rbp, desplazamiento 6*16.
    im.info(0x2000, 0, 10, (RBP as u8, 6), &[cod(10, SET_FPREG, 0), cod(5, ALLOC_SMALL, 11), cod(1, PUSH_NONVOL, RBP as u8)]);
    let f = im.funcion(0x2800, 0x1000, 0x1100, 0x2000);
    let e = PILA + 0x2000;
    let p = pila(e + 0x60, &[0x5A5A, BASE + 0x1500]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    // El cuerpo hizo un alloca: rsp ya no es el establisher; rbp si lo sabe.
    let mut c = ctx(BASE + 0x1050, e - 0x300);
    c.gp[RBP] = e + 0x60;
    let marco = desenrollar::un_marco(&m, BASE, &f, &mut c, 0).unwrap();
    assert_eq!(marco.establecido, e);
    assert_eq!((c.gp[RBP], c.rip, c.gp[RSP]), (0x5A5A, BASE + 0x1500, e + 0x70));
}

#[test]
fn reserva_grande_guardar_en_la_pila_y_un_xmm() {
    let mut im = Img::nueva();
    // sub rsp, 0x1000 (ALLOC_LARGE, info 0: /8 en un hueco); mov [rsp+0x20], rbx;
    // movaps [rsp+0x30], xmm6.
    im.info(0x2000, 0, 20, (0, 0), &[cod(20, SAVE_XMM128, 6), 3, cod(15, SAVE_NONVOL, RBX as u8), 4, cod(7, ALLOC_LARGE, 0), 0x200]);
    let f = im.funcion(0x2800, 0x1000, 0x1100, 0x2000);
    let e = PILA + 0x4000;
    let mut p = pila(e + 0x20, &[0xBB]);
    p[(e - PILA) as usize + 0x30..(e - PILA) as usize + 0x40].copy_from_slice(&0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210u128.to_le_bytes());
    let o = (e - PILA) as usize + 0x1000;
    p[o..o + 8].copy_from_slice(&(BASE + 0x1777).to_le_bytes());
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    let mut c = ctx(BASE + 0x1080, e);
    desenrollar::un_marco(&m, BASE, &f, &mut c, 0).unwrap();
    assert_eq!(c.gp[RBX], 0xBB);
    assert_eq!(c.xmm[6], 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210);
    assert_eq!((c.rip, c.gp[RSP]), (BASE + 0x1777, e + 0x1008));
}

#[test]
fn reserva_grande_de_32_bits_en_dos_huecos() {
    let mut im = Img::nueva();
    im.info(0x2000, 0, 11, (0, 0), &[cod(11, ALLOC_LARGE, 1), 0x2340, 0x0001]);
    let f = im.funcion(0x2800, 0x1000, 0x1100, 0x2000);
    let e = PILA;
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &vec![0u8; 0x13000] });
    let mut c = ctx(BASE + 0x1080, e);
    desenrollar::un_marco(&m, BASE, &f, &mut c, 0).unwrap();
    assert_eq!(c.gp[RSP], e + 0x12340 + 8);
}

#[test]
fn una_info_encadenada_sigue_con_la_de_su_padre() {
    let mut im = Img::nueva();
    // El padre: push rbx. El trozo: sub rsp, 0x28, y la cadena al padre.
    im.info(0x2100, 0, 1, (0, 0), &[cod(1, PUSH_NONVOL, RBX as u8)]);
    let detras = im.info(0x2000, CHAININFO, 4, (0, 0), &[cod(4, ALLOC_SMALL, 4)]);
    im.funcion(detras, 0x1000, 0x1010, 0x2100);
    let f = im.funcion(0x2800, 0x1040, 0x1080, 0x2000);
    let s = PILA + 0x100;
    let p = pila(s + 0x28, &[0xCAFE, BASE + 0x1999]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    // Primero los codigos del trozo y despues, los de su padre.
    let mut c = ctx(BASE + 0x1060, s);
    desenrollar::un_marco(&m, BASE, &f, &mut c, 0).unwrap();
    assert_eq!((c.gp[RBX], c.rip, c.gp[RSP]), (0xCAFE, BASE + 0x1999, s + 0x28 + 16));
}

#[test]
fn el_manejador_sale_solo_si_se_pide_su_tipo_y_fuera_del_prologo() {
    let mut im = Img::nueva();
    let detras = im.info(0x2000, UNW_FLAG_EHANDLER | UNW_FLAG_UHANDLER, 4, (0, 0), &[cod(4, ALLOC_SMALL, 4)]);
    im.u32(detras, 0x1840);
    let f = im.funcion(0x2800, 0x1000, 0x1100, 0x2000);
    let s = PILA + 0x100;
    let p = pila(s + 0x28, &[BASE + 0x1999]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    let marco = desenrollar::un_marco(&m, BASE, &f, &mut ctx(BASE + 0x1050, s), UNW_FLAG_EHANDLER).unwrap();
    assert_eq!(marco.manejador, Some(BASE + 0x1840));
    assert_eq!(marco.datos, BASE + detras as u64 + 4, "HandlerData: justo detras del manejador");
    let marco = desenrollar::un_marco(&m, BASE, &f, &mut ctx(BASE + 0x1050, s), UNW_FLAG_UHANDLER).unwrap();
    assert_eq!(marco.manejador, Some(BASE + 0x1840));
    // Solo EHANDLER en la info: al desenrollar no se llama.
    im.0[0x2000] = 1 | UNW_FLAG_EHANDLER << 3;
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    assert_eq!(desenrollar::un_marco(&m, BASE, &f, &mut ctx(BASE + 0x1050, s), UNW_FLAG_UHANDLER).unwrap().manejador, None);
    // Dentro del prologo el marco no se establecio: su manejador no cuenta.
    let p = pila(s, &[BASE + 0x1999]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    assert_eq!(desenrollar::un_marco(&m, BASE, &f, &mut ctx(BASE + 0x1002, s), UNW_FLAG_EHANDLER).unwrap().manejador, None);
}

#[test]
fn lo_que_no_se_desenrolla_se_dice() {
    let mut im = Img::nueva();
    let f = im.funcion(0x2800, 0x1000, 0x1100, 0x2000);
    im.info(0x2000, 0, 0, (0, 0), &[]);
    im.0[0x2000] = 3;
    let p = pila(PILA, &[]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    assert_eq!(desenrollar::un_marco(&m, BASE, &f, &mut ctx(BASE + 0x1050, PILA), 0), Err(NoDesenrolla::Version(3)));
    im.info(0x2000, 0, 1, (0, 0), &[cod(1, 7, 0)]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    assert_eq!(desenrollar::un_marco(&m, BASE, &f, &mut ctx(BASE + 0x1050, PILA), 0), Err(NoDesenrolla::Codigo(7)));
    // Una pila que no se deja leer: para, no inventa.
    im.info(0x2000, 0, 1, (0, 0), &[cod(1, PUSH_NONVOL, RBX as u8)]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    assert_eq!(desenrollar::un_marco(&m, BASE, &f, &mut ctx(BASE + 0x1050, 0x10), 0), Err(NoDesenrolla::Memoria(0x10)));
}

#[test]
fn buscar_encuentra_la_funcion_por_busqueda_binaria_y_no_la_de_los_huecos() {
    let mut im = Img::nueva();
    let a = im.funcion(0x2800, 0x1000, 0x1010, 0x2000);
    let b = im.funcion(0x280C, 0x1020, 0x1050, 0x2010);
    let c = im.funcion(0x2818, 0x1050, 0x1060, 0x2020);
    let m = Trozo { base: BASE, bytes: &im.0 };
    let buscar = |rva| desenrollar::buscar(&m, BASE, 0x2800, 36, rva);
    assert_eq!(buscar(0x1000), Some(a));
    assert_eq!(buscar(0x100F), Some(a));
    assert_eq!(buscar(0x1015), None, "un hueco entre funciones: una hoja");
    assert_eq!(buscar(0x1020), Some(b));
    assert_eq!(buscar(0x104F), Some(b));
    assert_eq!(buscar(0x1050), Some(c), "el fin es exclusivo");
    assert_eq!((buscar(0x0FFF), buscar(0x1060)), (None, None));
}

#[test]
fn el_context_de_windows_de_ida_y_vuelta_y_sin_pisar_lo_que_no_sabe() {
    let mut c = Contexto { rip: 0x1122, mxcsr: 0x1F80, ..Contexto::default() };
    for i in 0..16 {
        c.gp[i] = 0x100 + i as u64;
        c.xmm[i] = (i as u128) << 64 | 0xAA;
    }
    let mut b = vec![0xEEu8; CONTEXT_BYTES];
    c.a_context(&mut b);
    assert_eq!(Contexto::de_context(&b), c);
    assert_eq!(u32::from_le_bytes(b[0x30..0x34].try_into().unwrap()), 0x0010_000B, "CONTEXT_AMD64 | CONTROL | INTEGER | FLOATING_POINT");
    assert_eq!(u64::from_le_bytes(b[0x78 + 8 * RSP..0x80 + 8 * RSP].try_into().unwrap()), 0x104, "Rsp en 0x98");
    assert_eq!(u64::from_le_bytes(b[0xF8..0x100].try_into().unwrap()), 0x1122, "Rip en 0xF8");
    // sobre_context: las banderas y los segmentos de quien lo dio se quedan.
    let mut b = vec![0xEEu8; CONTEXT_BYTES];
    c.gp[RAX] = 0xABC;
    c.sobre_context(&mut b);
    assert_eq!(Contexto::de_context(&b), c);
    assert_eq!(&b[0x30..0x34], &[0xEE; 4]);
    assert_eq!(&b[0x38..0x44], &[0xEE; 12]);
}

// -- seh ---------------------------------------------------------------------------

#[test]
fn el_registro_y_el_despacho_en_la_forma_de_windows() {
    let r = Registro { codigo: 0xE000_0001, banderas: seh::EXCEPTION_NONCONTINUABLE, anidado: 0, direccion: 0x1400_0104, parametros: vec![0x1234, 5] };
    let mut b = vec![0xFFu8; seh::REGISTRO_BYTES];
    r.a_bytes(&mut b);
    assert_eq!(&b[..8], &[0x01, 0, 0, 0xE0, 1, 0, 0, 0]);
    assert_eq!(b[0x18], 2, "NumberParameters en 0x18");
    assert_eq!(u64::from_le_bytes(b[0x20..0x28].try_into().unwrap()), 0x1234, "ExceptionInformation[0] en 0x20");
    assert_eq!(Registro::de_bytes(&b), r);
    let muchos = Registro { parametros: (0..20).collect(), ..Registro::default() };
    muchos.a_bytes(&mut b);
    assert_eq!(Registro::de_bytes(&b).parametros.len(), seh::MAX_PARAMETROS, "de 15 en adelante no caben");
    let d = Despacho { pc: 1, base_imagen: 2, funcion: 3, establecido: 4, destino: 5, contexto: 6, manejador: 7, datos: 8, historia: 9, indice: 10 };
    let mut b = vec![0xFFu8; seh::DESPACHO_BYTES];
    d.a_bytes(&mut b);
    assert_eq!(b[0x18], 4, "EstablisherFrame en 0x18");
    assert_eq!(b[0x38], 8, "HandlerData en 0x38");
    assert_eq!(b[seh::DESPACHO_INDICE], 10, "ScopeIndex en 0x48");
    assert_eq!(Despacho::de_bytes(&b), d);
}

/// Una cabecera de PE32+ minima en la imagen de mentira.
fn cabeceras(im: &mut Img, pdata: u32, pdata_tam: u32) {
    im.u16(0, 0x5A4D);
    im.u32(0x3C, 0x80);
    im.u32(0x80, 0x4550);
    im.u16(0x98, 0x20B);
    im.u32(0x98 + 56, 0x3000);
    im.u32(0x98 + 108, 16);
    im.u32(0x98 + 112 + 24, pdata);
    im.u32(0x98 + 112 + 28, pdata_tam);
}

#[test]
fn las_cabeceras_en_memoria_dan_la_medida_y_el_pdata() {
    let mut im = Img::nueva();
    cabeceras(&mut im, 0x2800, 24);
    let m = Trozo { base: BASE, bytes: &im.0 };
    assert_eq!(seh::imagen_en(&m, BASE), Some(Imagen { base: BASE, tam: 0x3000, pdata: 0x2800, pdata_tam: 24 }));
    im.u32(0x80, 0x4551);
    assert_eq!(seh::imagen_en(&Trozo { base: BASE, bytes: &im.0 }, BASE), None, "sin PE\\0\\0 no es una imagen");
    // Y un .exe de verdad, colocado: lo mismo que dice su cabecera.
    let d = include_bytes!("../prueba/ventana.exe");
    let pe = crate::leer(d).unwrap();
    let img = crate::colocar(&pe, d, 0x2_0000_0000).unwrap();
    let i = seh::imagen_en(&Trozo { base: 0x2_0000_0000, bytes: &img }, 0x2_0000_0000).unwrap();
    assert_eq!((i.tam, i.pdata, i.pdata_tam), (pe.tam_imagen, pe.excepciones.rva, pe.excepciones.tam));
}

#[test]
fn subir_hoja_marco_fuera_y_una_pila_que_no_sube() {
    let mut im = Img::nueva();
    cabeceras(&mut im, 0x2800, 24);
    im.info(0x2000, 0, 4, (0, 0), &[cod(4, ALLOC_SMALL, 4)]);
    im.funcion(0x2800, 0x1000, 0x1100, 0x2000);
    im.info(0x2010, 0, 1, (0, 0), &[cod(1, PUSH_MACHFRAME, 0)]);
    im.funcion(0x280C, 0x1100, 0x1200, 0x2010);
    let s = PILA + 0x100;
    let p = pila(s, &[BASE + 0x1234, 0, 0, s - 0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x7777]);
    let m = Dos(Trozo { base: BASE, bytes: &im.0 }, Trozo { base: PILA, bytes: &p });
    let imagenes = [seh::imagen_en(&m, BASE).unwrap()];
    // Fuera de toda imagen: el contexto no cambia.
    let mut c = ctx(0x5555, s);
    assert_eq!(seh::subir(&m, &imagenes, &mut c, 0), Ok(Subida::Fuera));
    assert_eq!(c, ctx(0x5555, s));
    // En la imagen pero sin RUNTIME_FUNCTION: una hoja.
    let mut c = ctx(BASE + 0x1500, s);
    assert_eq!(seh::subir(&m, &imagenes, &mut c, 0), Ok(Subida::Hoja));
    assert_eq!((c.rip, c.gp[RSP]), (BASE + 0x1234, s + 8));
    // Con su RUNTIME_FUNCTION: un marco, y donde estaba.
    let mut c = ctx(BASE + 0x1050, s - 0x28);
    let Ok(Subida::Marco { pc, marco, funcion, .. }) = seh::subir(&m, &imagenes, &mut c, 0) else { panic!("un marco") };
    assert_eq!((pc, marco.establecido, funcion.inicio), (BASE + 0x1050, s - 0x28, 0x1000));
    assert_eq!((c.rip, c.gp[RSP]), (BASE + 0x1234, s + 8));
    // Un marco de maquina que dice que quien llamo esta MAS ABAJO: roto.
    let mut c = ctx(BASE + 0x1150, s);
    assert_eq!(seh::subir(&m, &imagenes, &mut c, 0), Err(NoDesenrolla::NoSube(s - 0x40)));
    // RtlLookupFunctionEntry: la funcion, o nada en una hoja o fuera.
    assert_eq!(seh::funcion_de(&m, &imagenes[0], BASE + 0x1150).map(|f| f.dir), Some(BASE + 0x280C));
    assert_eq!(seh::funcion_de(&m, &imagenes[0], BASE + 0x1500), None);
    assert_eq!(seh::funcion_de(&m, &imagenes[0], 0x5555), None);
}

/// La tabla de ambitos de `inicio` en seh.c, como la escribe clang: los dos
/// `__try` anidados comparten rango y el de dentro va primero.
fn tabla_de_inicio() -> Vec<Ambito> {
    let a = |inicio, fin, manejador, destino| Ambito { inicio, fin, manejador, destino };
    vec![
        a(0x1070, 0x1080, 0x1310, 0x12FC),
        a(0x10D2, 0x10D8, seh::FILTRO_EJECUTAR, 0x12CF),
        a(0x1118, 0x1127, 0x15C0, 0x12AD),
        a(0x1118, 0x1127, seh::FILTRO_EJECUTAR, 0x12A7),
        a(0x1158, 0x1165, 0x15F0, 0x12C5),
    ]
}

#[test]
fn la_tabla_de_ambitos_se_lee_como_la_escribe_el_compilador() {
    let mut im = Img::nueva();
    let t = tabla_de_inicio();
    im.u32(0x2400, t.len() as u32);
    for (i, a) in t.iter().enumerate() {
        let f = 0x2404 + 16 * i as u32;
        im.u32(f, a.inicio);
        im.u32(f + 4, a.fin);
        im.u32(f + 8, a.manejador);
        im.u32(f + 12, a.destino);
    }
    let m = Trozo { base: BASE, bytes: &im.0 };
    assert_eq!(seh::ambitos(&m, BASE + 0x2400), Some(t));
    im.u32(0x2400, 1 << 20);
    assert_eq!(seh::ambitos(&Trozo { base: BASE, bytes: &im.0 }, BASE + 0x2400), None, "un millon de filas no es una tabla");
}

#[test]
fn primera_pasada_el_de_dentro_primero_y_luego_el_de_fuera() {
    let t = tabla_de_inicio();
    // La vuelta de la llamada de dentro de los dos __try anidados.
    assert_eq!(seh::al_buscar(&t, 0, 0x1126), Some(2));
    // CONTINUE_SEARCH del de dentro: se sigue por la fila de despues.
    assert_eq!(seh::al_buscar(&t, 3, 0x1126), Some(3));
    assert_eq!(seh::al_buscar(&t, 4, 0x1126), None);
    // El fin del rango no es del __try, y fuera de todos no hay nada.
    assert_eq!(seh::al_buscar(&t, 0, 0x10D8), None);
    assert_eq!(seh::al_buscar(&t, 0, 0x1200), None);
    // Un __finally no tiene filtro: la primera pasada lo salta.
    let t = [Ambito { inicio: 0x10, fin: 0x20, manejador: 0x500, destino: 0 }, Ambito { inicio: 0x10, fin: 0x30, manejador: 1, destino: 0x40 }];
    assert_eq!(seh::al_buscar(&t, 0, 0x18), Some(1));
}

#[test]
fn segunda_pasada_corre_los_finally_de_en_medio_y_para_en_el_destino() {
    // con_finally en seh.c: un __finally (destino 0) que cubre la llamada.
    let fin = [Ambito { inicio: 0x14FA, fin: 0x151D, manejador: 0x1550, destino: 0 }];
    assert_eq!(seh::al_desenrollar(&fin, 0, 0x1511, None), AlDesenrollar::Finally(0));
    assert_eq!(seh::al_desenrollar(&fin, 1, 0x1511, None), AlDesenrollar::Nada, "ScopeIndex ya lo paso: no se corre dos veces");
    // En el marco destino se para en SU __except, y un __except de en medio
    // no se toca.
    let t = tabla_de_inicio();
    assert_eq!(seh::al_desenrollar(&t, 0, 0x10D7, Some(0x12CF)), AlDesenrollar::Llego(1));
    assert_eq!(seh::al_desenrollar(&t, 0, 0x1126, Some(0x12A7)), AlDesenrollar::Llego(3));
    assert_eq!(seh::al_desenrollar(&t, 0, 0x1126, None), AlDesenrollar::Nada);
    // Un __finally DENTRO del __try destino corre; uno que lo envuelve, no.
    let a = |inicio, fin, manejador, destino| Ambito { inicio, fin, manejador, destino };
    let t = [a(0x10, 0x20, 0x500, 0), a(0x10, 0x30, 1, 0x40), a(0x08, 0x50, 0x600, 0)];
    assert_eq!(seh::al_desenrollar(&t, 0, 0x18, Some(0x40)), AlDesenrollar::Finally(0));
    assert_eq!(seh::al_desenrollar(&t, 1, 0x18, Some(0x40)), AlDesenrollar::Llego(1));
    // Si el destino es de otro marco, en este corren todos.
    assert_eq!(seh::al_desenrollar(&t, 1, 0x18, None), AlDesenrollar::Finally(2));
}

#[test]
fn los_vectorizados_en_su_orden_y_se_quitan_por_su_asa() {
    let mut v = Vectores::nuevos();
    let a = v.poner(false, 0xA);
    let b = v.poner(true, 0xB);
    let c = v.poner(false, 0xC);
    assert_eq!(v.en_orden(), [0xB, 0xA, 0xC], "el que pide ser primero va delante");
    assert!(a != 0 && b != 0 && c != 0 && a != b && b != c);
    assert!(v.quitar(a));
    assert!(!v.quitar(a), "dos veces no");
    assert_eq!(v.en_orden(), [0xB, 0xC]);
}
