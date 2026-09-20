//! **LAS DOS COPIAS DE UNA MISMA REGLA, ATADAS POR UNA PRUEBA.**
//!
//! # Por que hay dos copias, y por que NO se arreglo juntandolas
//!
//! El juez del formato vive en `bmo_abi::bef2::lector`, que es lo que llama
//! el toolchain. El cargador del kernel no lo puede llamar --este crate usa
//! `alloc`--, asi que la puerta (`bmo-bex-gate`, cero dependencias, la enlaza
//! Ring 0) repite las mismas reglas. Lo obvio seria que el lector delegara en
//! la puerta. **No se hace, y el motivo esta escrito en el `Cargo.toml` de
//! este crate:**
//!
//! > *"Solo para las PRUEBAS: los offsets compartidos con el cargador del
//! > kernel. No es dependencia de la libreria -- el contrato no depende de la
//! > puerta."*
//!
//! Delegar habria invertido esa flecha en silencio: el CONTRATO pasaria a
//! depender de la PUERTA. Y esa flecha no es un detalle de empaquetado -- es lo
//! que permite que exista mas de una puerta sin tocar el contrato.
//!
//! # Entonces que impide que se separen
//!
//! Esto. `bmo-bex-gate` es dev-dependency, o sea que **en las pruebas si esta**,
//! y aqui se le pregunta lo mismo a los dos y se exige la misma respuesta.
//!
//! > Dos copias de una decision son dos decisiones esperando a separarse.
//! > Cuando la arquitectura no deja juntarlas, lo que queda no es confiar:
//! > es **atarlas por fuera**.
//!
//! ** Las filas de BEF1 (`reloc_cabe` contra el validador, las versiones del
//! ABI, el enlazado dinamico, "lo que no es x86-64") se fueron con BEF1 el
//! 2026-09-19. Lo que aqui se ata es BEF2, y solo BEF2.

// =======================================================================
//  BEF2: la puerta del kernel y el lector del contrato, sobre el formato
//  nuevo (2026-09-19, B3 de docs/plan/PLAN_BEF_NATIVO.md)
// =======================================================================
//
// ** La puerta NO puede importar `bmo-abi` (cero dependencias: la enlaza Ring
// 0), asi que vuelve a haber dos lectores del mismo formato. Es la misma
// situacion que con BEF1 y se paga igual: estas filas le preguntan lo mismo a
// los dos y exigen la misma respuesta.

fn bef2_buena() -> Vec<u8> {
    let mut e = bmo_abi::bef2::Escritor::ejecutable();
    e.codigo(vec![0xC3; 64])
        .constantes(b"hola\0".to_vec())
        .datos(vec![0u8; 8])
        .ceros(4096)
        .reloc(bmo_abi::bef2::Reloc {
            donde: bmo_abi::bef2::Region::Datos,
            destino: bmo_abi::bef2::Region::Constantes,
            offset: 0,
            addend: 0,
        });
    e.construir().unwrap()
}

/// La puerta ve las REGIONES como secciones, con el tipo que el kernel ya
/// sabia mapear y el indice con el que la firma las nombra.
#[test]
fn la_puerta_ve_un_bef2_como_las_secciones_que_el_kernel_espera() {
    let img = bef2_buena();
    bmo_abi::bef2::leer(&img).expect("el lector la acepta");
    let rev = bmo_bex_gate::revisar(&img, img.len()).expect("la puerta la acepta");
    assert_eq!(rev.entry_offset(), 0);

    let kinds: Vec<u8> = rev.secciones().map(|s| s.kind).collect();
    assert!(kinds.contains(&bmo_bex_gate::CODE));
    assert!(kinds.contains(&bmo_bex_gate::RODATA));
    assert!(kinds.contains(&bmo_bex_gate::DATA));
    assert!(kinds.contains(&bmo_bex_gate::BSS));
    assert!(kinds.contains(&bmo_bex_gate::RELOCS));
    assert!(kinds.contains(&bmo_bex_gate::SIGNATURE));
    assert!(kinds.contains(&bmo_bex_gate::REQUISITOS));

    // El codigo es lo unico ejecutable, y los ceros no ocupan fichero.
    for s in rev.secciones() {
        assert_eq!(
            s.flags & bmo_bex_gate::SECCION_FLAG_EXEC != 0,
            s.kind == bmo_bex_gate::CODE,
            "kind {:#04x}",
            s.kind
        );
        if s.kind == bmo_bex_gate::BSS {
            assert_eq!(s.file_size, 0);
            assert_eq!(s.mem_size, 4096);
        }
    }
    // Y la firma nombra a las regiones por su numero: 0 codigo, 1 constantes,
    // 2 datos. Es el mismo byte que escribe el escritor.
    let codigo = rev.secciones().find(|s| s.kind == bmo_bex_gate::CODE).unwrap();
    assert_eq!(codigo.indice, 0);
}

/// Lo que hay que traer del disco cubre todo lo que el kernel lee.
#[test]
fn la_puerta_dice_cuanto_hay_que_traer_de_un_bef2() {
    let img = bef2_buena();
    let rev = bmo_bex_gate::revisar(&img, img.len()).unwrap();
    let hasta = rev.hasta_donde_hace_falta();
    for s in rev.secciones() {
        if s.kind == bmo_bex_gate::BSS || !bmo_bex_gate::se_lee(s.kind) {
            continue;
        }
        assert!(
            s.file_offset + s.file_size <= hasta,
            "la seccion {:#04x} se queda fuera de lo que se trae",
            s.kind
        );
    }
    assert!(hasta <= img.len() as u64);
}

/// **Las dos copias contestan lo mismo.** Con la puerta de ayer --que no sabia
/// leer BEF2-- todas estas filas fallan en la primera linea.
#[test]
fn los_dos_rechazan_las_mismas_mentiras_de_un_bef2() {
    let cambios: [(&str, fn(&mut Vec<u8>)); 10] = [
        ("otro abi", |i| i[4] = 9),
        ("bandera inventada", |i| i[5] |= 1 << 6),
        ("reservado sucio", |i| i[6] = 1),
        ("pide AVX", |i| i[8..16].copy_from_slice(&0b111u64.to_le_bytes())),
        ("sin codigo", |i| i[28..32].copy_from_slice(&0u32.to_le_bytes())),
        ("entrada fuera", |i| i[16..20].copy_from_slice(&9999u32.to_le_bytes())),
        ("demasiados anexos", |i| i[20..24].copy_from_slice(&99u32.to_le_bytes())),
        ("codigo fuera del fichero", |i| {
            i[28..32].copy_from_slice(&0xFFFF_0000u32.to_le_bytes())
        }),
        // ** Las dos las encontro la pasada hostil (19-09): una region VACIA
        // con el offset fuera del fichero pasaba al lector (no ocupa sitio) y
        // luego rebanarla panicaba; y un anexo vacio lo tragaba el lector y lo
        // rechazaba la puerta. Los dos jueces contestan lo mismo, y es NO.
        ("region vacia fuera del fichero", |i| {
            i[40..44].copy_from_slice(&0xFFFF_0000u32.to_le_bytes()); // datos: offset
            i[44..48].copy_from_slice(&0u32.to_le_bytes()); // datos: 0 bytes
        }),
        ("anexo vacio", |i| i[64 + 8..64 + 12].copy_from_slice(&0u32.to_le_bytes())),
    ];
    for (que, cambio) in cambios {
        let mut img = bef2_buena();
        cambio(&mut img);
        assert!(
            bmo_abi::bef2::leer(&img).is_err(),
            "el lector traga '{que}'"
        );
        assert!(
            bmo_bex_gate::revisar(&img, img.len()).is_err(),
            "la puerta traga '{que}'"
        );
    }
}

/// Un OBJETO es una imagen valida y NO se carga: la puerta manda al enlazador.
#[test]
fn un_objeto_bef2_es_valido_y_la_puerta_no_lo_carga() {
    let mut e = bmo_abi::bef2::Escritor::objeto();
    e.codigo(vec![0xC3; 16]);
    let img = e.construir().unwrap();
    assert!(bmo_abi::bef2::leer(&img).is_ok());
    assert_eq!(
        bmo_bex_gate::revisar(&img, img.len()).err(),
        Some(bmo_bex_gate::Falta::EsUnObjetoSinEnlazar)
    );
}

/// Sin firma no se carga: el kernel aplica relocs que vienen del mismo
/// fichero, y sin hashes no hay con que comprobar que llegaron enteros.
#[test]
fn un_bef2_sin_firma_no_pasa_la_puerta() {
    let mut img = bef2_buena();
    // Se le quita el anexo de firma de la tabla (es el ultimo).
    let cuantos = u32::from_le_bytes(img[20..24].try_into().unwrap()) as usize;
    img[20..24].copy_from_slice(&((cuantos - 1) as u32).to_le_bytes());
    assert!(bmo_bex_gate::revisar(&img, img.len()).is_err());
    assert!(bmo_abi::bef2::leer(&img).is_err());
}

/// **Los relocs de un BEF2, leidos COMO LOS LEE EL KERNEL.**
///
/// *** ESTA FILA EXISTE PORQUE B3 MINTIO. Decia "Ring 0 no cambia una linea",
/// y `task/bex.rs::leer_reloc` seguia descodificando el registro de 24 bytes
/// de BEF1 sobre el anexo de 16 de BEF2: en el Ryzen, todo programa con un
/// puntero en sus datos habria muerto en "relocation fuera de su seccion". Se
/// cazo leyendo, en B6. Aqui se descodifica con el codigo del kernel copiado
/// (`RELOC_SIZE` y `gate::reloc::*`) y se exige lo mismo que dice el juez.
#[test]
fn el_kernel_lee_los_relocs_de_un_bef2() {
    use bmo_bex_gate::{reloc, RELOC_SIZE};
    let img = bef2_buena();
    let v = bmo_abi::bef2::leer(&img).unwrap();
    let rev = bmo_bex_gate::revisar(&img, img.len()).unwrap();
    let tabla = rev
        .secciones()
        .find(|s| s.kind == bmo_bex_gate::RELOCS)
        .expect("trae relocs");
    let bytes = &img[tabla.file_offset as usize..(tabla.file_offset + tabla.file_size) as usize];
    assert_eq!(RELOC_SIZE, bmo_abi::bef2::RELOC);
    assert_eq!(bytes.len() % RELOC_SIZE, 0);

    // -- Copiado de `bex::leer_reloc` ---------------------------------------
    let cuantos = bytes.len() / RELOC_SIZE;
    let leer = |n: usize| -> (u8, u8, u64, i64) {
        let b = n * RELOC_SIZE;
        assert_eq!(u16::from_le_bytes(bytes[b + reloc::RELLENO..b + reloc::RELLENO + 2].try_into().unwrap()), 0);
        (
            bytes[b + reloc::DONDE],
            bytes[b + reloc::DESTINO],
            u32::from_le_bytes(bytes[b + reloc::OFFSET..b + reloc::OFFSET + 4].try_into().unwrap()) as u64,
            u64::from_le_bytes(bytes[b + reloc::ADDEND..b + reloc::ADDEND + 8].try_into().unwrap()) as i64,
        )
    };
    // -- Copiado de `admitir.rs::seccion_por_codigo_reloc` -------------------
    let kind_de = |region: u8| match region {
        0 => bmo_bex_gate::CODE,
        1 => bmo_bex_gate::RODATA,
        2 => bmo_bex_gate::DATA,
        3 => bmo_bex_gate::BSS,
        _ => panic!("region {region} que el kernel no conoce"),
    };

    let del_juez: Vec<_> = v.relocs().collect();
    assert_eq!(cuantos, del_juez.len());
    for (n, r) in del_juez.iter().enumerate() {
        let (donde, destino, off, add) = leer(n);
        assert_eq!((donde, destino, off, add), (r.donde as u8, r.destino as u8, r.offset as u64, r.addend as i64));
        // Y la region que el kernel busca por ese numero es la que la puerta
        // presento con ese mismo numero de hash.
        let s_donde = rev.secciones().find(|s| s.kind == kind_de(donde)).expect("la region existe");
        assert_eq!(s_donde.indice, donde as usize);
        let s_dest = rev.secciones().find(|s| s.kind == kind_de(destino)).expect("la region destino existe");
        assert_eq!(s_dest.indice, destino as usize);
    }
    // El de `bef2_buena`: datos[0] <- constantes+0.
    assert_eq!(leer(0), (2, 1, 0, 0));
}

/// **La firma de un BEF2, leida COMO LA LEE EL KERNEL.**
///
/// `task/landing.rs::Firmas` lee cada entrada como `section_index: u16` +
/// relleno + digest, y BEF2 escribe `que: u8` + cero + relleno. Son los mismos
/// bytes mientras el indice quepa en uno, y en BEF2 cabe siempre. Esta fila lo
/// comprueba con el codigo del kernel copiado a mano: si algun dia dejan de
/// coincidir, se pone roja aqui y no en el Ryzen.
#[test]
fn el_kernel_encuentra_los_hashes_de_un_bef2() {
    const CAB: usize = 8;
    const ENTRADA: usize = 40;
    const DIGEST: usize = 32;

    let img = bef2_buena();
    let rev = bmo_bex_gate::revisar(&img, img.len()).unwrap();
    let firma = rev
        .secciones()
        .find(|s| s.kind == bmo_bex_gate::SIGNATURE)
        .expect("trae firma");
    let bytes = &img[firma.file_offset as usize..(firma.file_offset + firma.file_size) as usize];

    // -- Copiado de `Firmas::abrir` / `Firmas::digest_de` --------------------
    let cuantos = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    assert!(CAB + cuantos * ENTRADA <= bytes.len(), "la tabla no cuadra");
    let digest_de = |idx: usize| -> Option<[u8; DIGEST]> {
        for k in 0..cuantos {
            let e = CAB + k * ENTRADA;
            let quien = u16::from_le_bytes(bytes[e..e + 2].try_into().unwrap()) as usize;
            if quien != idx {
                continue;
            }
            let mut d = [0u8; DIGEST];
            d.copy_from_slice(&bytes[e + 8..e + 8 + DIGEST]);
            return Some(d);
        }
        None
    };

    // El codigo es el indice 0, y su digest tiene que cuadrar con sus bytes.
    let codigo = rev
        .secciones()
        .find(|s| s.kind == bmo_bex_gate::CODE)
        .unwrap();
    let suyos = &img[codigo.file_offset as usize..(codigo.file_offset + codigo.file_size) as usize];
    assert_eq!(
        digest_de(codigo.indice).expect("el codigo tiene digest"),
        bmo_abi::bef::blake3::blake3_256(suyos)
    );
    // Y los RELOCS, que el kernel aplica, tambien: se nombran `0x80 | n`.
    let relocs = rev
        .secciones()
        .find(|s| s.kind == bmo_bex_gate::RELOCS)
        .unwrap();
    let suyos = &img[relocs.file_offset as usize..(relocs.file_offset + relocs.file_size) as usize];
    assert_eq!(
        digest_de(relocs.indice).expect("los relocs tienen digest"),
        bmo_abi::bef::blake3::blake3_256(suyos)
    );
}
