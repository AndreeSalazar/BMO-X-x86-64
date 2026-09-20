//! **EL CORPUS DE MENTIRAS.**
//!
//! Cada prueba coge una imagen buena y le cambia UNA cosa, la que un fichero de
//! fuera podria traer cambiada. La imagen sigue midiendo lo mismo y sigue
//! pareciendo un `.bex`.
//!
//! ## Por que esto vale mas que antes
//!
//! Este corpus iba a ser una **red sobre una duplicacion**: pasarle los mismos
//! ficheros al validador del toolchain y al del kernel y exigir que coincidieran.
//! Eso caza la divergencia despues de escribirla.
//!
//! Con la decision en un solo sitio, es un test unitario y ya esta -- y lo que
//! demuestra lo heredan **los dos** consumidores sin escribir nada.
//!
//! ** BEF2 desde el 2026-09-19 (B6: BEF1 murio). Las filas que mentian con
//! banderas, arquitectura, orden de bytes o alineacion se fueron con los campos
//! que mentian: BEF2 no los tiene.

extern crate std;
use super::*;
use std::vec;
use std::vec::Vec;

/// Escribe un `.bex` BEF2 a mano. Sin usar el escritor de `bmo-abi` **a
/// proposito**: si las pruebas de la puerta usaran el mismo codigo que fabrica
/// los ficheros buenos, comprobarian que el escritor es coherente consigo
/// mismo, que no es la pregunta. Un extranjero no usa nuestro escritor.
struct Imagen {
    banderas: u8,
    xcr0: u64,
    entrada: u32,
    /// (offset, bytes) de codigo, constantes y datos, y los bytes de ceros.
    codigo: (u32, u32),
    constantes: (u32, u32),
    datos: (u32, u32),
    ceros: u32,
    /// (tipo, offset, bytes)
    anexos: Vec<(u8, u32, u32)>,
    total: u32,
    abi: u8,
}

impl Imagen {
    /// Codigo en 0x100 (64 B), constantes en 0x140 (16), datos en 0x150 (16),
    /// relocs en 0x160 (16 = un reloc) y firma en 0x170 (8: cero hashes). 768 B.
    fn buena() -> Self {
        Self {
            banderas: bef2::EJECUTABLE,
            xcr0: bef2::XCR0_PRESERVADO,
            entrada: 0,
            codigo: (0x100, 64),
            constantes: (0x140, 16),
            datos: (0x150, 16),
            ceros: 4096,
            anexos: vec![(bef2::ANEXO_RELOCS, 0x160, 16), (bef2::ANEXO_FIRMA, 0x170, 8)],
            total: 768,
            abi: bef2::ABI,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; self.total as usize];
        b[0..4].copy_from_slice(&bef2::MAGIC.to_le_bytes());
        b[4] = self.abi;
        b[5] = self.banderas;
        b[8..16].copy_from_slice(&self.xcr0.to_le_bytes());
        b[16..20].copy_from_slice(&self.entrada.to_le_bytes());
        b[20..24].copy_from_slice(&(self.anexos.len() as u32).to_le_bytes());
        for (o, (off, len)) in [(24usize, self.codigo), (32, self.constantes), (40, self.datos)] {
            b[o..o + 4].copy_from_slice(&off.to_le_bytes());
            b[o + 4..o + 8].copy_from_slice(&len.to_le_bytes());
        }
        b[48..52].copy_from_slice(&self.ceros.to_le_bytes());
        b[52..56].copy_from_slice(&self.total.to_le_bytes());
        for (i, (tipo, off, len)) in self.anexos.iter().enumerate() {
            let e = 64 + i * 16;
            if e + 16 > b.len() {
                break;
            }
            b[e] = *tipo;
            b[e + 4..e + 8].copy_from_slice(&off.to_le_bytes());
            b[e + 8..e + 12].copy_from_slice(&len.to_le_bytes());
        }
        // Un reloc: datos[0] <- constantes + 0.
        b[0x160] = 2;
        b[0x161] = 1;
        // Codigo: `ret`s.
        for x in &mut b[0x100..0x140] {
            *x = 0xC3;
        }
        b
    }
}

fn falta_de(img: &Imagen) -> Falta {
    let b = img.bytes();
    revisar(&b, b.len()).err().expect("tenia que fallar")
}

#[test]
fn una_imagen_buena_pasa() {
    let b = Imagen::buena().bytes();
    let r = revisar(&b, b.len()).expect("la buena pasa");
    assert_eq!(r.entry_offset(), 0);
    // Cuatro regiones presentadas como secciones, mas dos anexos.
    assert_eq!(r.cuantas(), 6);
    assert!(r.buscar(CODE).is_some());
    assert!(r.buscar(RODATA).is_some());
    assert!(r.buscar(DATA).is_some());
    assert!(r.buscar(BSS).is_some());
    assert!(r.buscar(RELOCS).is_some());
    assert!(r.buscar(SIGNATURE).is_some());
    // Solo el codigo es ejecutable.
    for s in r.secciones() {
        assert_eq!(s.flags & SECCION_FLAG_EXEC != 0, s.kind == CODE);
    }
}

/// BEF1 murio: su magic se rechaza por el primer numero, como un ELF o un PE.
#[test]
fn otro_magic_no_es_un_bex() {
    let mut b = Imagen::buena().bytes();
    b[0..4].copy_from_slice(b"BEF1");
    assert_eq!(revisar(&b, b.len()).err().unwrap(), Falta::CabeceraInvalida);
    b[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    assert_eq!(revisar(&b, b.len()).err().unwrap(), Falta::CabeceraInvalida);
}

#[test]
fn otro_abi_no_pasa() {
    let mut img = Imagen::buena();
    img.abi = 3;
    assert_eq!(falta_de(&img), Falta::OtraVersionDelAbi);
}

#[test]
fn una_bandera_desconocida_se_rechaza() {
    let mut img = Imagen::buena();
    img.banderas |= 1 << 6;
    assert_eq!(falta_de(&img), Falta::PideAlgoQueNadieImplementa);
}

#[test]
fn un_objeto_no_se_carga_y_se_dice_por_que() {
    let mut img = Imagen::buena();
    img.banderas = bef2::OBJETO;
    assert_eq!(falta_de(&img), Falta::EsUnObjetoSinEnlazar);
    // Y un ejecutable con un anexo ENLACE es un objeto disfrazado.
    let mut img = Imagen::buena();
    img.anexos.push((bef2::ANEXO_ENLACE, 0x180, 24));
    assert_eq!(falta_de(&img), Falta::EsUnObjetoSinEnlazar);
}

#[test]
fn ni_ejecutable_ni_objeto_no_es_nada() {
    let mut img = Imagen::buena();
    img.banderas = 0;
    assert_eq!(falta_de(&img), Falta::NoEsEjecutable);
}

#[test]
fn una_extension_de_cpu_que_no_se_preserva_se_rechaza() {
    let mut img = Imagen::buena();
    img.xcr0 = bef2::XCR0_PRESERVADO | (1 << 2); // AVX
    assert_eq!(falta_de(&img), Falta::ExtensionDeCpuQueNoSePreserva);
}

#[test]
fn dos_regiones_no_pueden_pisarse() {
    let mut img = Imagen::buena();
    img.constantes = (0x100, 16); // encima del codigo
    assert_eq!(falta_de(&img), Falta::SeccionesSeSolapan);
    let mut img = Imagen::buena();
    img.anexos[1] = (bef2::ANEXO_FIRMA, 0x160, 8); // encima de los relocs
    assert_eq!(falta_de(&img), Falta::SeccionesSeSolapan);
}

#[test]
fn dos_regiones_pegadas_no_se_pisan() {
    let mut img = Imagen::buena();
    img.constantes = (0x140, 16);
    img.datos = (0x150, 16);
    let b = img.bytes();
    assert!(revisar(&b, b.len()).is_ok());
}

#[test]
fn una_region_no_puede_salirse_del_fichero() {
    let mut img = Imagen::buena();
    img.datos = (0x2F8, 16);
    assert_eq!(falta_de(&img), Falta::SeccionFueraDelFichero);
    // Ni una VACIA con el offset fuera: lo encontro la pasada hostil.
    let mut img = Imagen::buena();
    img.datos = (0xFFFF_0000, 0);
    assert_eq!(falta_de(&img), Falta::SeccionFueraDelFichero);
}

#[test]
fn un_anexo_vacio_no_es_un_anexo() {
    let mut img = Imagen::buena();
    img.anexos[0] = (bef2::ANEXO_RELOCS, 0x160, 0);
    assert_eq!(falta_de(&img), Falta::SeccionInvalida);
}

#[test]
fn el_entry_no_puede_caer_fuera_del_codigo() {
    let mut img = Imagen::buena();
    img.entrada = 64;
    assert_eq!(falta_de(&img), Falta::EntryFueraDelCodigo);
}

#[test]
fn sin_codigo_no_hay_programa() {
    let mut img = Imagen::buena();
    img.codigo = (0x100, 0);
    assert_eq!(falta_de(&img), Falta::SinCodigo);
}

#[test]
fn sin_firma_no_pasa() {
    let mut img = Imagen::buena();
    img.anexos.pop();
    assert_eq!(falta_de(&img), Falta::CabeceraQueSeDesmiente);
}

#[test]
fn una_imagen_cortada_lo_dice_como_transporte() {
    let b = Imagen::buena().bytes();
    assert_eq!(revisar(&b, 700).err().unwrap(), Falta::ImagenIncompleta);
}

#[test]
fn demasiados_anexos() {
    let mut img = Imagen::buena();
    for i in 0..15u32 {
        img.anexos.push((0x20 + i as u8, 0x200 + i * 16, 16));
    }
    assert_eq!(falta_de(&img), Falta::DemasiadasSecciones);
}

#[test]
fn tabla_que_no_cabe_en_lo_leido_no_es_tabla_invalida() {
    let mut img = Imagen::buena();
    for i in 0..10u32 {
        img.anexos.push((0x20 + i as u8, 0x200 + i * 16, 16));
    }
    let b = img.bytes();
    // 64 + 12 * 16 = 256 > 200: se puede leer mas y volver.
    assert_eq!(revisar(&b[..200], b.len()).err().unwrap(), Falta::TablaFueraDeLoLeido);
    assert!(revisar(&b, b.len()).is_ok());
}

#[test]
fn ningun_campo_trucado_puede_reventar_al_lector() {
    let base = Imagen::buena().bytes();
    for campo in (4..64).step_by(4) {
        for valor in [u32::MAX, 0x8000_0000, 1, 0] {
            let mut b = base.clone();
            b[campo..campo + 4].copy_from_slice(&valor.to_le_bytes());
            // Solo tiene que NO reventar. Que conteste no nos importa aqui.
            let _ = revisar(&b, 768);
            let _ = revisar(&b, usize::MAX);
            let _ = revisar(&b[..50], 768);
        }
    }
    // Y la tabla de anexos entera, byte a byte.
    for i in 64..96 {
        let mut b = base.clone();
        b[i] = 0xFF;
        let _ = revisar(&b, 768);
    }
}

/// `hasta_donde_hace_falta` no cuenta lo que el cargador no toca. Es el escalon 2
/// de `LA_RAM.md` en una funcion: un paquete con un WAD dentro mide seis megas y
/// lo que hay que leer para ejecutarlo son ochocientos kilos.
#[test]
fn los_recursos_no_cuentan_para_lo_que_hay_que_leer() {
    let mut img = Imagen::buena();
    img.anexos.push((bef2::ANEXO_RECURSOS, 768, 1_000_000));
    img.total = 1_000_768;
    let b = img.bytes();
    let rev = revisar(&b, img.total as usize).expect("tiene que pasar");
    assert_eq!(
        rev.hasta_donde_hace_falta(),
        0x178,
        "el millon de bytes de recursos NO hay que traerlos para ejecutar"
    );
}

// -- ** `reloc_cabe`: la regla que el cargador no tenia (2026-08-25) ---------
//
// Cinco casos, y el que importa es el tercero: **no se sale de la imagen, se
// mete en la seccion de al lado.** Ese es el que el cargador dejaba pasar,
// porque su unica comprobacion era "cae en la pagina que estoy parcheando", y
// caia.

/// El caso bueno, y el borde exacto: ocho bytes que acaban justo en el final de
/// la seccion SI caben.
///
/// # *** ESTE BORDE NO ES TEORICO: ES DOOM, Y NO SOBRA NI UN BYTE
///
/// Se midieron las 1.285 relocations de `doom.bex` contra esta regla antes de
/// cablearla, para saber si rechazaba algo que hoy funciona. Ninguna. Pero la
/// mas ajustada sale asi:
///
/// ```text
///    .data de DOOM        151.560 bytes = 0x25008
///    la reloc #706        offset 0x25000, ocho bytes, acaba en 0x25008
///    holgura              CERO
/// ```
///
/// > **Un `<` en vez de un `<=` no habria fallado una prueba: habria dejado de
/// > cargar DOOM.** Y el sintoma no seria "relocation invalida", seria que el
/// > programa mas grande del arbol deja de arrancar por un byte.
///
/// El codegen pone punteros al final de `.data` porque es donde caen; que la
/// ultima acabe justo en el limite no es casualidad, es lo normal.
#[test]
fn una_reloc_que_acaba_justo_en_el_borde_cabe() {
    assert!(super::reloc_cabe(0x3F8, 8, 0x400, 0x400), "0x3F8 + 8 = 0x400, y la seccion son 0x400");
    assert!(super::reloc_cabe(0, 8, 0x400, 0x400));
    // Los numeros de verdad de `doom.bex`, reloc #706. Si esto se pone rojo,
    // DOOM no arranca.
    assert!(
        super::reloc_cabe(0x25000, 8, 0x25008, 0x25008),
        "la reloc mas ajustada de DOOM: holgura CERO y es legal"
    );
}

/// Un byte mas alla del borde NO cabe. Es el reverso del de arriba y va al lado
/// a proposito: los dos juntos fijan el `<=` y ninguno de los dos solo lo hace.
#[test]
fn un_solo_byte_de_mas_no_cabe() {
    assert!(!super::reloc_cabe(0x3F9, 8, 0x400, 0x400), "acabaria en 0x401");
}

/// *** EL CASO QUE ESTO EXISTE PARA CAZAR.
///
/// Una `.data` de 0x400 con una reloc en 0x9000. No se sale de la imagen: las
/// secciones van seguidas, asi que **cae dentro de otra**. El cargador
/// comprobaba que el destino estuviera en la pagina que estaba parcheando --lo
/// estaba-- y escribia ocho bytes en la seccion del vecino.
#[test]
fn una_reloc_que_apunta_a_la_seccion_de_al_lado_no_cabe() {
    assert!(!super::reloc_cabe(0x9000, 8, 0x400, 0x400));
}

/// Una `.bss` no ocupa en el fichero y si en memoria, y se parchea sobre lo que
/// hay EN MEMORIA. Con el tope puesto en `file_size` esto diria que no, y
/// rechazaria programas correctos.
#[test]
fn manda_el_tamano_en_memoria_y_no_el_del_fichero() {
    assert!(super::reloc_cabe(0x100, 8, 0, 0x1000), "una .bss: 0 en fichero, 0x1000 en memoria");
}

/// **El desbordamiento es un NO, no un panico.** `offset` viene del fichero, o
/// sea de fuera: `u64::MAX` es un valor que alguien puede escribir a mano, y un
/// `+` normal en `release` daria la vuelta y contestaria que SI cabe.
#[test]
fn un_offset_imposible_no_da_la_vuelta() {
    assert!(!super::reloc_cabe(u64::MAX, 8, 0x400, 0x400));
    assert!(!super::reloc_cabe(u64::MAX - 3, 8, u64::MAX, u64::MAX));
}

/// ** HOSTILE PASS (2026-09-17): the gate reads the prologue of every `.bex`
/// before anything else in the kernel trusts it. A good image and a two-section
/// one, mutated, and the file size told the truth, a byte short and absurd.
/// Checked: nothing panics.
#[test]
fn hostile_prologues_never_panic() {
    let one = Imagen::buena().bytes();
    let mut two = Imagen::buena();
    two.anexos.push((bef2::ANEXO_RECURSOS, 0x200, 256));
    two.total = 1024;
    let two = two.bytes();
    bmo_hostile::attack("bex gate", bmo_hostile::DEFAULT_SEED, 30_000, &[&one, &two], 256, |x| {
        for size in [x.len(), x.len().saturating_sub(1), 768, 1024, usize::MAX] {
            if let Ok(r) = revisar(x, size) {
                let _ = (r.entry_offset(), r.cuantas(), r.hasta_donde_hace_falta());
                for i in 0..r.cuantas() + 2 {
                    let _ = r.seccion(i);
                }
                let _ = r.secciones().count();
                let _ = (r.buscar(CODE), r.buscar(DATA), r.buscar(0xFF));
            }
        }
    });
}
