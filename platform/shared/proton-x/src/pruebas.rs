//! El banco: un `.exe` de Windows REAL (`prueba/hola.exe`, de clang y lld-link;
//! como se rehace, en `prueba/HACER.txt`) y sus mutaciones.

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use crate::*;

const HOLA: &[u8] = include_bytes!("../prueba/hola.exe");
const FRASE: &[u8] = b"hola desde un .exe de Windows\r\n";

fn rva_de_la_frase(img: &[u8]) -> usize {
    img.windows(FRASE.len()).position(|w| w == FRASE).expect("la frase esta en la imagen")
}

/// La tabla de la casa del banco: las tres de `hola.exe`, en direcciones
/// inventadas y distintas.
fn tabla(dll: &str, f: &Funcion) -> Option<u64> {
    if !dll.eq_ignore_ascii_case("kernel32.dll") {
        return None;
    }
    match f {
        Funcion::Nombre(n) if n == "GetStdHandle" => Some(0x7000_0010),
        Funcion::Nombre(n) if n == "WriteFile" => Some(0x7000_0020),
        Funcion::Nombre(n) if n == "ExitProcess" => Some(0x7000_0030),
        _ => None,
    }
}

#[test]
fn hola_exe_esta_dentro_y_se_lee_entero() {
    let pe = leer(HOLA).unwrap();
    assert_eq!(pe.base, 0x1_4000_0000);
    assert_eq!(pe.entrada, 0x1000);
    let nombres: Vec<_> = pe.secciones.iter().map(|s| (s.nombre.as_str(), s.permiso())).collect();
    assert_eq!(nombres, [(".text", Permiso::Codigo), (".rdata", Permiso::Lectura), (".reloc", Permiso::Lectura)]);
    assert_eq!(pe.tls.rva, 0);
}

#[test]
fn lo_que_pide_son_tres_funciones_de_kernel32() {
    let pe = leer(HOLA).unwrap();
    let img = colocar(&pe, HOLA, pe.base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    let nombres: Vec<_> = imps.iter().map(|i| (i.dll.as_str(), i.funcion.to_string())).collect();
    assert_eq!(nombres, [("kernel32.dll", "ExitProcess".to_string()), ("kernel32.dll", "GetStdHandle".to_string()), ("kernel32.dll", "WriteFile".to_string())]);
}

/// Cada `call [rip+x]` del codigo (`FF 15 disp32`) cae en una ranura de la
/// IAT: las ranuras que se rellenan son las que el codigo usa de verdad.
#[test]
fn cada_llamada_del_codigo_cae_en_una_ranura_de_la_iat() {
    let pe = leer(HOLA).unwrap();
    let img = colocar(&pe, HOLA, pe.base).unwrap();
    let ranuras: Vec<u32> = importaciones(&pe, &img).unwrap().iter().map(|i| i.ranura).collect();
    let texto = &pe.secciones[0];
    let codigo = &img[texto.rva as usize..(texto.rva + texto.tam_virtual) as usize];
    let mut llamadas = 0;
    for k in 0..codigo.len().saturating_sub(6) {
        if codigo[k] == 0xFF && codigo[k + 1] == 0x15 {
            let disp = i32::from_le_bytes([codigo[k + 2], codigo[k + 3], codigo[k + 4], codigo[k + 5]]);
            let destino = (texto.rva as i64 + k as i64 + 6 + disp as i64) as u32;
            assert!(ranuras.contains(&destino), "call [rip] a {destino:#x}, fuera de la IAT {ranuras:x?}");
            llamadas += 1;
        }
    }
    assert_eq!(llamadas, 3, "WriteFile, GetStdHandle y ExitProcess");
}

#[test]
fn en_su_base_el_puntero_absoluto_vale_lo_que_dejo_el_enlazador() {
    let pe = leer(HOLA).unwrap();
    let img = colocar(&pe, HOLA, pe.base).unwrap();
    let frase = rva_de_la_frase(&img) as u64;
    let punteros: Vec<usize> = (0..img.len() - 8).step_by(8).filter(|&o| u64::from_le_bytes(img[o..o + 8].try_into().unwrap()) == pe.base + frase).collect();
    assert_eq!(punteros.len(), 1, "`mensaje` apunta a la frase");
}

/// Movido a otra base, la relocalizacion DIR64 lleva `mensaje` con el.
#[test]
fn movido_a_otra_base_la_relocalizacion_lo_sigue() {
    let pe = leer(HOLA).unwrap();
    let otra = 0x5000_0000u64;
    let img = colocar(&pe, HOLA, otra).unwrap();
    let frase = rva_de_la_frase(&img) as u64;
    let en_su_base = colocar(&pe, HOLA, pe.base).unwrap();
    let o = (0..en_su_base.len() - 8).step_by(8).find(|&o| u64::from_le_bytes(en_su_base[o..o + 8].try_into().unwrap()) == pe.base + frase).unwrap();
    assert_eq!(u64::from_le_bytes(img[o..o + 8].try_into().unwrap()), otra + frase);
    // Y NADA MAS cambio: una relocalizacion de mas seria un byte pisado.
    let distintos = img.iter().zip(&en_su_base).filter(|(a, b)| a != b).count();
    assert!(distintos <= 8, "{distintos} bytes distintos entre las dos bases");
}

#[test]
fn con_la_tabla_entera_cada_ranura_recibe_su_funcion() {
    let pe = leer(HOLA).unwrap();
    let mut img = colocar(&pe, HOLA, pe.base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    resolver(&mut img, &imps, tabla).unwrap();
    for i in &imps {
        let o = i.ranura as usize;
        assert_eq!(Some(u64::from_le_bytes(img[o..o + 8].try_into().unwrap())), tabla(&i.dll, &i.funcion));
    }
}

/// Sin `WriteFile` en la tabla: NO arranca, dice cual, y no escribio nada.
#[test]
fn si_falta_una_no_arranca_y_dice_cual() {
    let pe = leer(HOLA).unwrap();
    let mut img = colocar(&pe, HOLA, pe.base).unwrap();
    let antes = img.clone();
    let imps = importaciones(&pe, &img).unwrap();
    let e = resolver(&mut img, &imps, |d, f| if matches!(f, Funcion::Nombre(n) if n == "WriteFile") { None } else { tabla(d, f) }).unwrap_err();
    assert_eq!(e.to_string(), "no arranca: faltan 1 funcion(es) en la tabla de la casa: kernel32.dll!WriteFile");
    assert_eq!(img, antes, "a medias no se escribe nada");
}

fn firma(d: &[u8]) -> usize {
    u32::from_le_bytes(d[0x3C..0x40].try_into().unwrap()) as usize
}

#[test]
fn fuera_con_su_motivo_como_dice_rayosx() {
    for (maquina, palabra) in [(0x14C, "32 bits"), (0xAA64, "ARM64 ("), (0xA641, "ARM64EC"), (0xA64E, "ARM64X"), (0x1C4, "ARM de 32"), (0x1C0, "desconocida")] {
        let mut m = HOLA.to_vec();
        let e = firma(&m);
        m[e + 4..e + 6].copy_from_slice(&(maquina as u16).to_le_bytes());
        match leer(&m) {
            Err(Fallo::Fuera(motivo)) => assert!(motivo.contains(palabra), "{maquina:#x}: {motivo}"),
            otro => panic!("{maquina:#x}: {otro:?}"),
        }
    }
    let mut m = HOLA.to_vec();
    let e = firma(&m);
    m[e + 24..e + 26].copy_from_slice(&0x10Bu16.to_le_bytes());
    assert!(matches!(leer(&m), Err(Fallo::Fuera(t)) if t.contains("PE32 ")));
    let mut m = HOLA.to_vec();
    m[e + 24 + 112 + 14 * 8..e + 24 + 112 + 14 * 8 + 4].copy_from_slice(&0x2000u32.to_le_bytes());
    assert!(matches!(leer(&m), Err(Fallo::Fuera(t)) if t.starts_with(".NET")));
    assert_eq!(leer(b"nada"), Err(Fallo::NoEsPe));
}

/// `.text` marcada ademas como escribible: el W^X de la casa la rechaza.
#[test]
fn una_seccion_que_escribe_y_ejecuta_se_rechaza() {
    let mut m = HOLA.to_vec();
    let e = firma(&m);
    let tam_opc = u16::from_le_bytes([m[e + 20], m[e + 21]]) as usize;
    let s = e + 24 + tam_opc;
    let c = u32::from_le_bytes(m[s + 36..s + 40].try_into().unwrap()) | 0x8000_0000;
    m[s + 36..s + 40].copy_from_slice(&c.to_le_bytes());
    assert_eq!(leer(&m), Err(Fallo::EscribeYEjecuta(".text".into())));
}

/// Una relocalizacion de otro tipo (HIGHLOW, de 32 bits) no se aplica a
/// ciegas: se dice.
#[test]
fn una_relocalizacion_de_otro_tipo_se_dice() {
    let pe = leer(HOLA).unwrap();
    let reloc = pe.secciones.iter().find(|s| s.nombre == ".reloc").unwrap();
    let mut m = HOLA.to_vec();
    let entrada = reloc.desde as usize + 8;
    let e = u16::from_le_bytes([m[entrada], m[entrada + 1]]);
    assert_eq!(e >> 12, 10, "la primera es DIR64");
    m[entrada..entrada + 2].copy_from_slice(&((3 << 12) | (e & 0xFFF)).to_le_bytes());
    let pe = leer(&m).unwrap();
    assert!(matches!(colocar(&pe, &m, 0x5000_0000), Err(Fallo::Relocalizacion { tipo: 3, .. })));
    // En su base no hay nada que mover: carga.
    assert!(colocar(&pe, &m, pe.base).is_ok());
}

#[test]
fn un_fichero_cortado_dice_donde() {
    for largo in [0x3E, 0x90, 0x150, 0x300] {
        let r = leer(&HOLA[..largo]);
        assert!(r.is_err(), "cortado en {largo:#x} no puede leerse entero");
    }
}

// ============================ P1d: teb.exe ============================

const TEB_EXE: &[u8] = include_bytes!("../prueba/teb.exe");

#[test]
fn teb_exe_esta_dentro_pide_siete_y_no_trae_reloc() {
    let pe = leer(TEB_EXE).unwrap();
    assert_eq!(pe.relocalizaciones.rva, 0, "todo lo suyo es relativo a RIP");
    assert!(!pe.relocs_quitadas);
    // Se mueve de base sin nada que corregir: el cargador de Windows hace lo mismo.
    let img = colocar(&pe, TEB_EXE, 0x7_0000_0000).unwrap();
    let mut nombres: Vec<_> = importaciones(&pe, &img).unwrap().iter().map(|i| i.funcion.to_string()).collect();
    nombres.sort();
    assert_eq!(nombres, ["ExitProcess", "GetCurrentProcessId", "GetCurrentThreadId", "GetLastError", "GetStdHandle", "SetLastError", "WriteFile"]);
    assert_eq!(partir(&pe).unwrap(), Partes { codigo: 2 * PAGINA, datos: 2 * PAGINA });
}

#[test]
fn con_relocs_stripped_no_se_mueve() {
    let mut d = TEB_EXE.to_vec();
    let e = u32::from_le_bytes([d[0x3C], d[0x3D], d[0x3E], d[0x3F]]) as usize;
    d[e + 22] |= 1;
    let pe = leer(&d).unwrap();
    assert!(pe.relocs_quitadas);
    assert!(colocar(&pe, &d, pe.base).is_ok(), "en SU base si");
    assert_eq!(colocar(&pe, &d, 0x7_0000_0000), Err(Fallo::SinRelocalizaciones));
}

#[test]
fn el_teb_y_el_peb_tienen_la_forma_de_windows_x64() {
    let h = teb::Hilo { teb: 0x5000, peb: 0x7000, pila_tope: 0x8000_0000, pila_fondo: 0x7FFF_0000, proceso: 3, hilo: 9, base_imagen: 0xE010_3000 };
    let mut t = vec![0xAAu8; teb::TEB_BYTES];
    let mut p = vec![0xAAu8; teb::PEB_BYTES];
    teb::escribir_teb(&mut t, &h);
    teb::escribir_peb(&mut p, &h);
    let q = |b: &[u8], o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
    assert_eq!(q(&t, 0x30), 0x5000, "Self");
    assert_eq!(q(&t, 0x60), 0x7000, "el PEB");
    assert_eq!((q(&t, 0x08), q(&t, 0x10)), (0x8000_0000, 0x7FFF_0000), "StackBase y StackLimit");
    assert_eq!((q(&t, 0x40), q(&t, 0x48)), (3, 9), "ClientId");
    assert_eq!(u32::from_le_bytes(t[0x68..0x6C].try_into().unwrap()), 0, "LastErrorValue empieza en 0");
    assert_eq!(q(&p, 0x10), 0xE010_3000, "ImageBaseAddress");
    assert_eq!(p[0x02], 0, "BeingDebugged");
    // Todo lo demas, a cero: ni un byte de lo que habia.
    assert_eq!(t.iter().filter(|&&b| b == 0xAA).count(), 0);
    assert_eq!(p.iter().filter(|&&b| b == 0xAA).count(), 0);
}

// ============================ P2: ventanas ============================

use crate::ventanas::*;

const VENTANA_EXE: &[u8] = include_bytes!("../prueba/ventana.exe");

#[test]
fn ventana_exe_pide_dieciseis_de_tres_dll() {
    let pe = leer(VENTANA_EXE).unwrap();
    let img = colocar(&pe, VENTANA_EXE, 0x7_0000_0000).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    let cuantas = |dll: &str| imps.iter().filter(|i| i.dll.eq_ignore_ascii_case(dll)).count();
    assert_eq!((cuantas("user32.dll"), cuantas("gdi32.dll"), cuantas("kernel32.dll")), (13, 1, 2));
    // Los 256 KB del bufer de pixeles van en .data (ceros): la parte de datos.
    let partes = partir(&pe).unwrap();
    assert_eq!(partes.codigo, 2 * PAGINA);
    assert!(partes.datos as usize >= 320 * 200 * 4);
}

#[test]
fn la_cola_saca_en_el_orden_de_windows() {
    let mut c = Cola::nueva();
    let tecla = Msg { hwnd: 1, mensaje: WM_CHAR, wparam: b'a' as u64, lparam: 1 };
    c.invalidar(1);
    c.invalidar(1);
    c.publicar(tecla);
    c.salir(7);
    // Lo llegado primero, luego WM_QUIT, y WM_PAINT el ultimo.
    assert_eq!(c.sacar(), Some(tecla));
    assert_eq!(c.sacar().map(|m| (m.mensaje, m.wparam)), Some((WM_QUIT, 7)));
    assert_eq!(c.sacar().map(|m| (m.mensaje, m.hwnd)), Some((WM_PAINT, 1)));
    // Sin BeginPaint sigue invalida: el mismo WM_PAINT otra vez, UNO.
    assert_eq!(c.sacar().map(|m| m.mensaje), Some(WM_PAINT));
    c.validar(1);
    assert_eq!(c.sacar(), None);
}

#[test]
fn los_eventos_de_bmo_x_son_los_mensajes_de_windows() {
    // Una letra (bit 62): WM_CHAR.
    let e = 1 << 62 | 1 << 8 | 1 << 9 | b'z' as u64;
    assert_eq!(de_evento(5, e).map(|m| (m.mensaje, m.wparam)), Some((WM_CHAR, b'z' as u64)));
    // Un clic izquierdo en (10, 20): WM_LBUTTONDOWN con x | y << 16.
    let e = 1 << 63 | 1 << 8 | 1 << 9 | 1 | 10 << 16 | 20 << 32;
    assert_eq!(de_evento(5, e), Some(Msg { hwnd: 5, mensaje: WM_LBUTTONDOWN, wparam: MK_LBUTTON, lparam: 10 | 20 << 16 }));
    // Soltar ESC (scancode 0x01): WM_KEYUP con VK_ESCAPE y los bits 30 y 31.
    let m = de_evento(5, 1 << 8 | 0x01).unwrap();
    assert_eq!((m.mensaje, m.wparam, m.lparam >> 30), (WM_KEYUP, 0x1B, 3));
    // La A del teclado: VK 'A'.
    assert_eq!(de_evento(5, 1 << 8 | 1 << 9 | 0x1E).map(|m| (m.mensaje, m.wparam)), Some((WM_KEYDOWN, b'A' as u64)));
    // Un raton sin boton no es mensaje; un evento vacio tampoco.
    assert_eq!(de_evento(5, 1 << 63 | 1 << 8 | 30 << 16), None);
    assert_eq!(de_evento(5, 0), None);
}

#[test]
fn un_dib_de_abajo_arriba_cae_derecho() {
    // 2x2, de ABAJO arriba (biHeight positivo): la fila 0 del bufer es la de abajo.
    let mut cab = vec![0u8; 40];
    cab[0] = 40;
    cab[4] = 2;
    cab[8] = 2;
    cab[14] = 32;
    let dib = leer_dib(&cab).unwrap();
    assert!(!dib.de_arriba);
    let bits: Vec<u8> = [0x11u32, 0x22, 0x33, 0x44].iter().flat_map(|p| p.to_le_bytes()).collect();
    let mut d = vec![0u32; 3 * 3];
    let r = Rect { x: 1, y: 1, ancho: 2, alto: 2 };
    let todo = Rect { x: 0, y: 0, ancho: 2, alto: 2 };
    assert_eq!(copiar_dib(&mut d, 3, 3, 3, r, todo, &bits, &dib), Ok(2));
    // Arriba de la ventana va la fila de ARRIBA del dibujo: 0x33, 0x44.
    assert_eq!(&d[4..6], &[0xFF00_0033, 0xFF00_0044]);
    assert_eq!(&d[7..9], &[0xFF00_0011, 0xFF00_0022]);
    assert_eq!(d[0], 0, "fuera del rectangulo no se toca");
    // Estirar y los formatos que no son 32 bits dicen cual es su NO.
    assert_eq!(copiar_dib(&mut d, 3, 3, 3, Rect { ancho: 3, ..r }, todo, &bits, &dib), Err(NoPinta::Escala));
    cab[14] = 24;
    assert_eq!(leer_dib(&cab), Err(NoPinta::Formato { bits: 24, compresion: 0 }));
}

// ============================ P3b1: DXIL ============================

use crate::dxil::{self, Etapa};

const CUBO_VS: &[u8] = include_bytes!("../prueba/cubo_vs.dxil");
const CUBO_PS: &[u8] = include_bytes!("../prueba/cubo_ps.dxil");

fn semanticas(v: &[dxil::Elemento]) -> Vec<(&str, u32, u8)> {
    v.iter().map(|e| (e.semantica.as_str(), e.registro, e.mascara)).collect()
}

/// El testigo es `dxc -dumpbin` de los mismos bytes (ver prueba/HACER.txt):
/// lo que dice ahi es lo que el lector tiene que encontrar solo.
#[test]
fn el_sombreador_de_vertices_del_cubo_se_lee_entero() {
    let s = dxil::leer(CUBO_VS).unwrap();
    assert_eq!(s.etapa, Etapa::Vertice);
    assert_eq!(s.modelo, (6, 0));
    assert_eq!(s.partes, [*b"SFI0", *b"ISG1", *b"OSG1", *b"PSV0", *b"STAT", *b"HASH", *b"DXIL"]);
    assert_eq!(semanticas(&s.entradas), [("POSITION", 0, 0b0111), ("NORMAL", 1, 0b0111), ("COLOR", 2, 0b1111)]);
    assert_eq!(semanticas(&s.salidas), [("SV_Position", 0, 0b1111), ("NORMAL", 1, 0b0111), ("COLOR", 2, 0b1111)]);
    assert_eq!(s.salidas[0].sistema, 1, "SV_Position es el valor de sistema 1");
    assert_eq!(s.modulo.productor, "dxc(private) 1.8.0.4662 (416fab6b5)", "llvm.ident, como lo dice dxc -dumpbin");
    let e = s.modulo.entrada().unwrap();
    assert_eq!((e.nombre.as_str(), e.instrucciones), ("vertice", 80));
    let mut ops = s.modulo.operaciones();
    ops.sort();
    assert_eq!(ops, ["dx.op.cbufferLoadLegacy.f32", "dx.op.createHandle", "dx.op.loadInput.f32", "dx.op.storeOutput.f32", "dx.op.tertiary.f32"]);
}

#[test]
fn el_sombreador_de_pixeles_del_cubo_se_lee_entero() {
    let s = dxil::leer(CUBO_PS).unwrap();
    assert_eq!(s.etapa, Etapa::Pixel);
    assert_eq!(semanticas(&s.entradas), [("SV_Position", 0, 0b1111), ("NORMAL", 1, 0b0111), ("COLOR", 2, 0b1111)]);
    assert_eq!(semanticas(&s.salidas), [("SV_Target", 0, 0b1111)]);
    assert_eq!(s.salidas[0].sistema, 64, "SV_Target es el valor de sistema 64");
    let e = s.modulo.entrada().unwrap();
    assert_eq!((e.nombre.as_str(), e.instrucciones), ("pixel", 31));
    let mut ops = s.modulo.operaciones();
    ops.sort();
    assert_eq!(ops, ["dx.op.cbufferLoadLegacy.f32", "dx.op.createHandle", "dx.op.dot3.f32", "dx.op.loadInput.f32", "dx.op.storeOutput.f32", "dx.op.unary.f32"]);
}

#[test]
fn un_sombreador_roto_dice_donde() {
    // Sin la parte DXIL (su FourCC cambiado): no es de P3.
    let mut d = CUBO_VS.to_vec();
    let o = d.windows(4).rposition(|w| w == b"DXIL").unwrap();
    let parte = d[..o].windows(4).rposition(|w| w == b"DXIL").unwrap_or(o);
    d[parte..parte + 4].copy_from_slice(b"XXXX");
    assert!(matches!(dxil::leer(&d), Err(dxil::NoSombreador::SinDxil) | Err(dxil::NoSombreador::Contenedor(_))));
    // El bitcode cortado a la mitad: el lector dice en que bit se quedo.
    let s = dxil::leer(CUBO_VS).unwrap();
    assert!(!s.modulo.bloques.is_empty());
    let bc_desde = CUBO_VS.windows(4).position(|w| w == [b'B', b'C', 0xC0, 0xDE]).unwrap();
    let cortado = &CUBO_VS[bc_desde..bc_desde + 600];
    assert!(matches!(dxil::bits::leer(cortado), Err(dxil::NoLee::Corto { .. })));
    // Y lo que no es DXBC, tampoco.
    assert_eq!(dxil::leer(b"MZ.."), Err(dxil::NoSombreador::Contenedor("no empieza por DXBC")));
}


// ============================ P3b2: DXBC y la root signature ============================

use crate::{dxbc, raiz};

const RAIZ_RTS: &[u8] = include_bytes!("../prueba/raiz.rts");

/// La huella de DXBC (el MD5 con el final de Microsoft) de TRES blobs de dxc:
/// la root signature y los dos sombreadores del cubo.
#[test]
fn la_huella_de_dxbc_es_la_de_microsoft() {
    for blob in [RAIZ_RTS, CUBO_VS, CUBO_PS] {
        assert_eq!(dxbc::huella(blob), blob[4..20], "la huella que escribio dxc");
    }
    // Un byte cambiado, otra huella.
    let mut d = RAIZ_RTS.to_vec();
    d[60] ^= 1;
    assert_ne!(dxbc::huella(&d), RAIZ_RTS[4..20]);
}

fn firma_del_cubo() -> raiz::Firma {
    raiz::Firma {
        parametros: vec![raiz::Parametro { tipo: raiz::CBV, visibilidad: 0, carga: raiz::Carga::Descriptor { registro: 0, espacio: 0 } }],
        samplers: Vec::new(),
        banderas: raiz::CON_INPUT_LAYOUT,
    }
}

#[test]
fn la_root_signature_del_cubo_se_lee_y_se_escribe_como_dxc() {
    assert_eq!(raiz::leer(RAIZ_RTS), Ok(firma_del_cubo()));
    // Serializada por la casa: los MISMOS 88 bytes que dxc, huella incluida.
    assert_eq!(raiz::serializar(&firma_del_cubo()), RAIZ_RTS);
}

#[test]
fn una_root_signature_con_tabla_constantes_y_sampler_va_y_vuelve() {
    let f = raiz::Firma {
        parametros: vec![
            raiz::Parametro { tipo: raiz::TABLA, visibilidad: 5, carga: raiz::Carga::Tabla(vec![
                raiz::Rango { tipo: 0, cuantos: 2, registro: 0, espacio: 0, desde: 0 },
                raiz::Rango { tipo: 2, cuantos: 1, registro: 1, espacio: 0, desde: 2 },
            ]) },
            raiz::Parametro { tipo: raiz::CONSTANTES, visibilidad: 1, carga: raiz::Carga::Constantes { registro: 1, espacio: 0, cuantas: 4 } },
            raiz::Parametro { tipo: raiz::SRV, visibilidad: 0, carga: raiz::Carga::Descriptor { registro: 3, espacio: 1 } },
        ],
        samplers: vec![[21, 1, 1, 1, 0, 16, 4, 0, 0, u32::MAX, 0, 0, 5]],
        banderas: raiz::CON_INPUT_LAYOUT,
    };
    let d = raiz::serializar(&f);
    assert_eq!(dxbc::huella(&d), d[4..20]);
    assert_eq!(raiz::leer(&d), Ok(f));
    assert_eq!(raiz::leer(CUBO_VS), Err(raiz::NoFirma::SinRts0), "un sombreador no es una root signature");
}

// -- P3b3: los sombreadores del cubo, EJECUTADOS ------------------------------

use crate::dxil::programa::{self, compilar, Op};

fn cb_de(f: u32) -> Vec<u8> {
    let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
    c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect()
}

/// **El sombreador de vertices, corrido, da BIT A BIT las cuentas del juez**:
/// `wvp * pos` (el FMad sin fundir es la suma de izquierda a derecha de
/// `transformar`), la normal por la parte 3x3 de `world`, y el color tal cual.
#[test]
fn el_sombreador_de_vertices_corrido_da_las_cuentas_del_juez() {
    let p = compilar(&dxil::leer(CUBO_VS).unwrap()).unwrap();
    assert_eq!((p.entradas, p.salidas, p.filas_cb), (3, 3, 7));
    assert_eq!(p.ops.iter().filter(|o| matches!(o, Op::Mad { .. })).count(), 14, "las 14 FMad del testigo");
    let mut regs = Vec::new();
    for f in [0u32, 30, 60, 123] {
        let cb = cb_de(f);
        let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
        for v in bmo_cubo::vertices() {
            let e = [[v.pos[0], v.pos[1], v.pos[2], 1.0], [v.normal[0], v.normal[1], v.normal[2], 0.0], v.color];
            let mut s = [[0.0f32; 4]; 3];
            p.correr(&e, &cb, &mut s, &mut regs);
            let bits = |x: &[f32]| x.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
            assert_eq!(bits(&s[0]), bits(&bmo_cubo::mat::transformar(&c.wvp, [v.pos[0], v.pos[1], v.pos[2], 1.0])), "SV_Position, fotograma {f}");
            assert_eq!(bits(&s[1][..3]), bits(&bmo_cubo::mat::transformar_dir(&c.world, v.normal)), "NORMAL, fotograma {f}");
            assert_eq!(s[2], v.color);
        }
    }
}

/// **El de pixeles, corrido, da el color de `iluminar`**: la misma luz, la
/// misma saturacion, y a 8 bits el MISMO pixel en todas las caras.
#[test]
fn el_sombreador_de_pixeles_corrido_ilumina_como_el_juez() {
    let p = compilar(&dxil::leer(CUBO_PS).unwrap()).unwrap();
    assert_eq!((p.salidas, p.filas_cb, p.lee), (1, 9, 0b110), "NORMAL y COLOR; SV_Position no");
    let mut regs = Vec::new();
    for f in [0u32, 30, 60] {
        let cb = cb_de(f);
        let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
        for v in bmo_cubo::vertices() {
            let n = bmo_cubo::mat::transformar_dir(&c.world, v.normal);
            let e = [[0.0; 4], [n[0], n[1], n[2], 0.0], v.color];
            let mut s = [[0.0f32; 4]; 1];
            p.correr(&e, &cb, &mut s, &mut regs);
            let juez = bmo_cubo::iluminar(v.color, n, c.luz);
            assert_eq!(bmo_cubo::empaquetar(s[0]), bmo_cubo::empaquetar(juez), "fotograma {f}");
        }
    }
}

/// La raiz de la casa es la EXACTA (la de IEEE-754), en enteros.
#[test]
fn la_raiz_de_enteros_es_la_de_ieee() {
    let mut x = 1.0e-44f32;
    while x < 1.0e37 {
        for y in [x, f32::from_bits(x.to_bits() + 1), f32::from_bits(x.to_bits() + 12345)] {
            assert_eq!(programa::raiz(y).to_bits(), y.sqrt().to_bits(), "raiz({y:e})");
        }
        x *= 1.37;
    }
    for y in [0.0f32, -0.0, 1.0, 4.0, 2.0, f32::INFINITY, f32::MIN_POSITIVE, f32::MAX] {
        assert_eq!(programa::raiz(y).to_bits(), y.sqrt().to_bits(), "raiz({y:e})");
    }
    assert!(programa::raiz(-1.0).is_nan());
    assert_eq!(programa::saturar(f32::NAN), 0.0);
}

use crate::trama;

/// El cubo del fotograma `f` como LOTE (`lote::en_cpu`, la costura que usa
/// la casa): los bytes de los vertices, los indices, las constantes, y los
/// DXIL de dxc enlazados con el input layout de `cubo.c`.
fn cubo_por_la_casa(f: u32) -> (Vec<u32>, trama::Cuenta) {
    use crate::lote::{self, ElementoIa, Lote, Topologia};
    let (vs, ps) = (dxil::leer(CUBO_VS).unwrap(), dxil::leer(CUBO_PS).unwrap());
    let e = |s: &str, formato, desde| ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde };
    let entradas = [e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)];
    let enlace = lote::enlazar(&vs, &ps, &entradas).unwrap();
    let vertices: Vec<u8> = bmo_cubo::vertices().iter().flat_map(|v| v.pos.iter().chain(&v.normal).chain(&v.color).flat_map(|x| x.to_le_bytes())).collect();
    let ids: Vec<u32> = bmo_cubo::indices().iter().map(|&i| i as u32).collect();
    let cb = cb_de(f);
    let (w, h) = (bmo_cubo::referencia::ANCHO, bmo_cubo::referencia::ALTO);
    let reglas = trama::Reglas { viewport: [0.0, 0.0, w as f32, h as f32, 0.0, 1.0], tijera: [0, 0, w as i32, h as i32], descarte: 3, antihorario: false };
    let l = Lote { enlace: &enlace, entradas: &entradas, vertices: &vertices, paso: 40, ids: &ids, topologia: Topologia::Lista, cb: &cb, reglas };
    let mut px = vec![bmo_cubo::FONDO; (w * h) as usize];
    let mut d = trama::Destino { pixeles: &mut px, ancho: w, alto: h, bgra: true };
    let cuenta = lote::en_cpu(&l, &mut d).unwrap();
    (px, cuenta)
}

/// *** P3b3: LOS SOMBREADORES DE DXC, CORRIDOS EN LA CPU, DIBUJAN LO QUE
/// DIBUJO D3D12 EN LA 3060: las huellas de los fotogramas 0, 30 y 60, bit a
/// bit, sin el juez de por medio (el juez solo presta los datos del cubo).
#[test]
fn los_dxil_corridos_y_la_trama_dan_las_huellas_de_d3d12() {
    for (f, esperada) in bmo_cubo::referencia::HUELLAS {
        let (px, cuenta) = cubo_por_la_casa(f);
        assert_eq!(bmo_cubo::referencia::huella(&px), esperada, "fotograma {f}: {cuenta:?}");
        assert_eq!((cuenta.dibujados + cuenta.descartados, cuenta.sin_recortar), (12, 0));
        // Cada cara es plana: el sombreador corre una vez por triangulo, no
        // por pixel (la memoria de la trama).
        assert!(cuenta.sombreados <= cuenta.dibujados as u64, "{cuenta:?}");
    }
}

/// Un triangulo de pantalla completa... casi: de (0,0) a (8,0) a (0,8) en un
/// destino de 8x8, en coordenadas de recorte con `w` por vertice.
fn triangulo(w: [f32; 3], atributo: [f32; 3], horario: bool) -> Vec<trama::Sombreado> {
    // x_ndc = px / 4 - 1, y_ndc = 1 - py / 4; en recorte, por w.
    let p = [(0.0f32, 0.0f32), (8.0, 0.0), (0.0, 8.0)];
    let mut v: Vec<trama::Sombreado> = (0..3)
        .map(|k| trama::Sombreado { pos: [(p[k].0 / 4.0 - 1.0) * w[k], (1.0 - p[k].1 / 4.0) * w[k], 0.0, w[k]], atributos: vec![[atributo[k], 0.0, 0.0, 1.0]] })
        .collect();
    if !horario {
        v.swap(1, 2);
    }
    v
}

fn pinta(v: &[trama::Sombreado], descarte: u32, antihorario: bool) -> (Vec<u32>, trama::Cuenta) {
    let mut px = vec![0u32; 64];
    let reglas = trama::Reglas { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], tijera: [0, 0, 8, 8], descarte, antihorario };
    let mut d = trama::Destino { pixeles: &mut px, ancho: 8, alto: 8, bgra: true };
    let c = trama::dibujar(&reglas, v, &[[0, 1, 2]], &mut d, |e| e[0]);
    (px, c)
}

#[test]
fn la_trama_descarta_la_cara_que_toca() {
    let h = triangulo([1.0; 3], [1.0; 3], true);
    let a = triangulo([1.0; 3], [1.0; 3], false);
    // (0,0) (8,0) (0,8) en pantalla es HORARIO: la de delante por defecto.
    assert_eq!(pinta(&h, 3, false).1.dibujados, 1);
    assert_eq!(pinta(&a, 3, false).1.descartados, 1);
    assert_eq!(pinta(&h, 2, false).1.descartados, 1);
    assert_eq!(pinta(&a, 3, true).1.dibujados, 1, "FrontCounterClockwise da la vuelta");
    // Sin descarte se pintan las dos, y los MISMOS pixeles.
    assert_eq!(pinta(&h, 1, false).0, pinta(&a, 1, false).0);
    // Los centros con x + y < 7: 28. Los ocho de x + y = 7 caen JUSTO en la
    // diagonal, que baja de (8,0) a (0,8): ni "top" ni "left", fuera.
    assert_eq!(pinta(&h, 1, false).1.pixeles, 28);
}

#[test]
fn la_trama_interpola_con_perspectiva() {
    // El atributo vale 0 en el vertice de (0,0) y 1 en los otros dos. Con w
    // iguales, en el centro del pixel (0,0) -- (0.5, 0.5) -- vale 1/8;
    // con el vertice de (0,0) cuatro veces mas cerca (w = 1 contra 4), el
    // mismo pixel ve mucho menos de los lejanos: 1/29.
    let plano = pinta(&triangulo([1.0; 3], [0.0, 1.0, 1.0], true), 1, false);
    let lejos = pinta(&triangulo([1.0, 4.0, 4.0], [0.0, 1.0, 1.0], true), 1, false);
    let rojo = |p: u32| (p >> 16) & 0xFF;
    assert_eq!(rojo(plano.0[0]), trama::unorm8(1.0 / 8.0));
    assert_eq!(rojo(lejos.0[0]), trama::unorm8(1.0 / 29.0));
    // Cambia en cada pixel: el sombreador corre en cada uno... salvo uno. El
    // ultimo de la fila 5 y el primero de la 6 ven lo mismo (7/8) y van
    // seguidos: la memoria reusa el color, que es la misma cuenta.
    assert_eq!(plano.1.sombreados, plano.1.pixeles - 1);
    // Y un atributo igual en los tres es ESE, exacto: una sola vez.
    let fijo = pinta(&triangulo([1.0, 3.0, 7.0], [0.3; 3], true), 1, false);
    assert_eq!(fijo.1.sombreados, 1);
    assert!(fijo.0.iter().filter(|&&p| p != 0).all(|&p| rojo(p) == trama::unorm8(0.3)));
}

#[test]
fn la_trama_no_pinta_lo_que_no_sabe_recortar() {
    let mut v = triangulo([1.0; 3], [1.0; 3], true);
    v[1].pos[3] = -1.0;
    let (px, c) = pinta(&v, 1, false);
    assert_eq!((c.sin_recortar, c.pixeles), (1, 0));
    assert!(px.iter().all(|&p| p == 0));
}

// -- P4: los hilos, decididos ---------------------------------------------------

use crate::hilos::{Estado, Objeto, Planificador, Turno, WAIT_OBJECT_0, WAIT_TIMEOUT};

#[test]
fn un_evento_despierta_al_que_espera_y_el_automatico_se_apaga() {
    let mut p = Planificador::nuevo();
    let ev = p.nuevo_objeto(Objeto::Evento { manual: false, encendido: false });
    let (h1, _) = p.crear(false);
    // El principal espera el evento: no se cumple, queda esperando.
    assert_eq!(p.esperar(&[ev], false, None, 0), None);
    // Le toca al 1 (el principal no puede).
    assert_eq!(p.siguiente(0), Turno::Hilo(h1));
    p.actual = h1;
    p.encender(ev, true);
    // El 1 cede: el principal ve el evento, lo consume, y sigue.
    assert_eq!(p.siguiente(0), Turno::Hilo(0));
    assert_eq!(p.resultado(0), WAIT_OBJECT_0);
    assert_eq!(p.objeto(ev), Some(Objeto::Evento { manual: false, encendido: false }), "automatico: se apaga al soltar a uno");
}

#[test]
fn esperar_todos_o_cualquiera_y_los_plazos() {
    let mut p = Planificador::nuevo();
    let a = p.nuevo_objeto(Objeto::Evento { manual: true, encendido: false });
    let b = p.nuevo_objeto(Objeto::Semaforo { cuenta: 1, max: 4 });
    // Cualquiera: el semaforo (indice 1) ya esta.
    assert_eq!(p.esperar(&[a, b], false, None, 0), Some(WAIT_OBJECT_0 + 1));
    assert_eq!(p.objeto(b), Some(Objeto::Semaforo { cuenta: 0, max: 4 }));
    // Todos: falta el evento; con plazo en 100 ns.
    assert_eq!(p.esperar(&[a, b], true, Some(100), 0), None);
    assert_eq!(p.siguiente(50), Turno::Esperar(100), "nadie puede: se duerme hasta el plazo");
    assert_eq!(p.siguiente(100), Turno::Hilo(0));
    assert_eq!(p.resultado(0), WAIT_TIMEOUT);
    // Un plazo ya vencido contesta en el acto.
    assert_eq!(p.esperar(&[a], false, Some(5), 10), Some(WAIT_TIMEOUT));
    assert_eq!(p.soltar_semaforo(b, 4), Some(0));
    assert_eq!(p.soltar_semaforo(b, 1), None, "pasaria del maximo");
}

#[test]
fn esperar_a_un_hilo_es_esperar_a_que_acabe() {
    let mut p = Planificador::nuevo();
    let (h, obj) = p.crear(true);
    assert_eq!(p.estado(h), Some(&Estado::Suspendido(1)));
    assert_eq!(p.esperar(&[obj], false, None, 0), None);
    assert_eq!(p.siguiente(0), Turno::Bloqueo, "suspendido y esperado: nadie puede seguir NUNCA");
    assert_eq!(p.reanudar(h), Some(1));
    assert_eq!(p.siguiente(0), Turno::Hilo(h));
    p.actual = h;
    p.terminar(h, 42);
    assert_eq!(p.siguiente(0), Turno::Hilo(0));
    assert_eq!(p.salida(h), Some(42));
    assert_eq!(p.vivos(), 1);
}

#[test]
fn una_seccion_critica_es_recursiva_y_el_otro_espera() {
    let mut p = Planificador::nuevo();
    let (h1, _) = p.crear(false);
    let cs = 0x1000;
    assert!(p.entrar(cs, true, true));
    assert!(p.entrar(cs, true, true), "el propietario vuelve a entrar");
    p.actual = h1;
    assert!(!p.probar(cs, true, true));
    assert!(!p.entrar(cs, true, true), "el 1 queda esperando");
    assert_eq!(p.siguiente(0), Turno::Hilo(0));
    p.actual = 0;
    assert!(p.salir(cs, true));
    assert_eq!(p.siguiente(0), Turno::Hilo(0), "todavia no: le queda una vuelta de recursion");
    assert!(p.salir(cs, true));
    assert_eq!(p.siguiente(0), Turno::Hilo(h1), "libre: el 1 la coge al tocarle");
    assert_eq!(p.ver_cerrojo(cs).map(|c| (c.propietario, c.recursion)), Some((Some(h1), 1)));
    p.actual = h1;
    assert!(!p.salir(0x9999, true), "soltar lo que no es tuyo: no");
}

#[test]
fn srw_lectores_juntos_y_escritor_solo() {
    let mut p = Planificador::nuevo();
    let (h1, _) = p.crear(false);
    let l = 0x2000;
    assert!(p.entrar(l, false, false));
    p.actual = h1;
    assert!(p.entrar(l, false, false), "dos lectores a la vez");
    assert!(!p.probar(l, true, false), "un escritor no entra con lectores");
    assert!(p.salir(l, false));
    p.actual = 0;
    assert!(p.salir(l, false));
    assert!(p.probar(l, true, false));
    assert!(!p.probar(l, true, false), "SRW no es recursivo");
}

#[test]
fn una_condicion_despierta_en_orden_de_llegada() {
    let mut p = Planificador::nuevo();
    let (h1, _) = p.crear(false);
    let (h2, _) = p.crear(false);
    let cv = 0x3000;
    p.actual = h1;
    p.dormir_en(cv, None);
    p.actual = h2;
    p.dormir_en(cv, Some(1000));
    p.actual = 0;
    p.despertar(cv, false);
    assert_eq!(p.siguiente(0), Turno::Hilo(h1), "el primero que llego");
    p.actual = h1;
    assert_eq!(p.siguiente(10), Turno::Hilo(0), "el 2 sigue dormido");
    p.actual = 0;
    assert_eq!(p.siguiente(1000), Turno::Hilo(h1));
    assert_eq!(p.siguiente(1000), Turno::Hilo(h1));
    p.actual = h1;
    assert_eq!(p.siguiente(1000), Turno::Hilo(h2), "al 2 se le paso el plazo");
    assert_eq!(p.resultado(h2), WAIT_TIMEOUT);
}

#[test]
fn todos_esperando_para_siempre_es_un_bloqueo() {
    let mut p = Planificador::nuevo();
    let (h1, _) = p.crear(false);
    let (a, b) = (0x10, 0x20);
    assert!(p.entrar(a, true, true));
    p.actual = h1;
    assert!(p.entrar(b, true, true));
    assert!(!p.entrar(a, true, true));
    p.actual = 0;
    assert!(!p.entrar(b, true, true));
    assert_eq!(p.siguiente(0), Turno::Bloqueo);
}

#[test]
fn dormir_cede_hasta_la_hora() {
    let mut p = Planificador::nuevo();
    let (h1, _) = p.crear(false);
    p.dormir(500);
    assert_eq!(p.siguiente(0), Turno::Hilo(h1));
    p.actual = h1;
    p.dormir(300);
    assert_eq!(p.siguiente(0), Turno::Esperar(300));
    assert_eq!(p.siguiente(400), Turno::Hilo(h1));
}

#[test]
fn un_lote_sin_input_layout_para_una_semantica_no_se_enlaza() {
    use crate::lote::{self, ElementoIa};
    let (vs, ps) = (dxil::leer(CUBO_VS).unwrap(), dxil::leer(CUBO_PS).unwrap());
    let sin_color = [ElementoIa { semantica: "POSITION".into(), indice: 0, formato: 6, ranura: 0, desde: 0 }, ElementoIa { semantica: "NORMAL".into(), indice: 0, formato: 6, ranura: 0, desde: 12 }];
    assert_eq!(lote::enlazar(&vs, &ps, &sin_color).err().as_deref(), Some("el sombreador de vertices lee COLOR0 y el input layout no lo da"));
    assert_eq!(lote::triangulos(&[0, 1, 2, 3], lote::Topologia::Tira), vec![[0, 1, 2], [2, 1, 3]], "en la tira, el impar se da la vuelta");
}

// -- P3b3b: los sombreadores nativos, con las reglas de INTI ---------------------

/// La tabla de INTI: las filas de SSE que su emisor pone en Ring 3.
const INTI_INTRINSECOS: &str = include_str!("../../../../toolchain/forge/sem-asm/tables/arch/x86_64/intrinsics.toml");

/// Los bytes de una fila `[nombre]` de la tabla de INTI.
fn fila_de_inti(nombre: &str) -> Vec<u8> {
    let desde = INTI_INTRINSECOS.find(&alloc::format!("[{nombre}]")).unwrap_or_else(|| panic!("INTI ya no tiene [{nombre}]"));
    let t = &INTI_INTRINSECOS[desde..];
    // Linea a linea hasta la `]` que cierra (los comentarios traen `[rsi]`).
    let t = &t[t.find("bytes = [").unwrap() + 9..];
    t.lines()
        .map(|l| l.split('#').next().unwrap().trim())
        .take_while(|l| !l.starts_with(']'))
        .flat_map(|l| l.split(','))
        .filter_map(|x| x.trim().strip_prefix("0x"))
        .map(|h| u8::from_str_radix(h, 16).unwrap())
        .collect()
}

/// **Los opcodes son los de INTI**: sus filas empaquetadas (`addps`,
/// `mulps`, `subps`: `0F op`) y las escalares de aqui (`F3 0F op`) son la
/// misma operacion en un carril; y el `acumula` de INTI (dos redondeos,
/// primero `mulps` y despues `addps`) es el FMad de aqui.
#[test]
fn los_opcodes_nativos_son_los_de_las_filas_sse_de_inti() {
    use crate::nativo::{ADDSS, MULSS, SUBSS};
    let tiene = |fila: &[u8], op: u8| fila.windows(2).any(|w| w == [0x0F, op]);
    assert!(tiene(&fila_de_inti("sse_suma4f"), ADDSS));
    assert!(tiene(&fila_de_inti("sse_por4f"), MULSS));
    assert!(tiene(&fila_de_inti("sse_resta4f"), SUBSS));
    let acumula = fila_de_inti("sse_acumula4f");
    let (m, a) = (acumula.windows(2).position(|w| w == [0x0F, MULSS]).unwrap(), acumula.windows(2).position(|w| w == [0x0F, ADDSS]).unwrap());
    assert!(m < a, "acumula: primero el producto, despues la suma");
    // Y el FMad de aqui es eso mismo: mulss y despues addss, dos redondeos.
    let p = crate::dxil::programa::Programa {
        ops: vec![Op::Mad { d: 0, a: 1, b: 2, c: 3 }],
        iniciales: vec![0.0; 4],
        entradas: 0,
        salidas: 0,
        lee: 0,
        filas_cb: 0,
    };
    let b = crate::nativo::compilar(&p);
    let (m, a) = (b.windows(3).position(|w| w == [0xF3, 0x0F, MULSS]).unwrap(), b.windows(3).position(|w| w == [0xF3, 0x0F, ADDSS]).unwrap());
    assert!(m < a);
    assert!(!b.windows(2).any(|w| w == [0x0F, 0x38]), "ni una instruccion de FMA (0F 38 ..): FMad NO se funde");
}
