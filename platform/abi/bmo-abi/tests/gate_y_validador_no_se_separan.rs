//! **LAS DOS COPIAS DE UNA MISMA REGLA, ATADAS POR UNA PRUEBA.**
//!
//! # Por que hay dos copias, y por que NO se arreglo juntandolas
//!
//! La regla es *"una relocation tiene que caber dentro de la seccion que dice
//! parchear"*, y el 2026-08-25 se descubrio que vivia en **un solo sitio**:
//! `bef::validator`, que es quien llama el toolchain. El cargador del kernel no
//! lo llama --no puede: este crate usa `alloc`-- asi que un `.bex` copiado a
//! mano al FAT32 entraba con sus relocations sin mirar.
//!
//! La regla se escribio en `bmo-bex-gate`, que es el juez sin `alloc` al que el
//! kernel SI llama. Lo obvio despues era que este validador delegara en el.
//! **No se hizo, y el motivo esta escrito en el `Cargo.toml` de este crate:**
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
//! [!] Y si algun dia el contrato deja de tener enforcement dentro --que seria
//! lo correcto-- esta prueba se borra con el. Mientras exista, existe.

/// La regla **tal y como la escribe el validador**, copiada de
/// `validate_reloc_section` a proposito.
///
/// Si alguien cambia alla y no aqui, esta funcion deja de representarlo y la
/// prueba de abajo se vuelve mentira. Es el limite de este metodo y se dice:
/// **ata las dos implementaciones, no vigila que esta copia siga siendo fiel.**
/// La linea de alla lleva un comentario que manda aqui.
fn como_lo_dice_el_validador(offset: u64, parche: usize, file_size: u64, mem_size: u64) -> bool {
    let end = offset as usize + parche;
    // El validador ERRA cuando se pasa de las dos; o sea que "cabe" es lo
    // contrario de eso.
    !(end > file_size as usize && end > mem_size as usize)
}

/// Los dos jueces, la misma pregunta, la misma respuesta.
///
/// Se barren los bordes de verdad --el ultimo byte que cabe, el primero que no,
/// la `.bss` que no ocupa en fichero-- y no una nube de numeros al azar: un
/// desacuerdo vive en un borde, no en el medio.
#[test]
fn reloc_cabe_dice_lo_mismo_que_este_validador() {
    let casos: &[(u64, usize, u64, u64)] = &[
        // (offset, parche, file_size, mem_size)
        (0, 8, 0x400, 0x400),        // el principio
        (0x3F8, 8, 0x400, 0x400),    // el ultimo que cabe, justo
        (0x3F9, 8, 0x400, 0x400),    // el primero que no
        (0x400, 8, 0x400, 0x400),    // el borde exacto por arriba
        (0x9000, 8, 0x400, 0x400),   // *** el que cae en la seccion de al lado
        (0x100, 8, 0, 0x1000),       // una .bss: manda `mem`
        (0xFFC, 4, 0x1000, 0x1000),  // un parche de 4, que existe en el formato
        (0xFFD, 4, 0x1000, 0x1000),
        (0, 8, 0, 0),                // una seccion vacia no admite ninguna
    ];
    for &(off, parche, fs, ms) in casos {
        let gate = bmo_bex_gate::reloc_cabe(off, parche as u64, fs, ms);
        let val = como_lo_dice_el_validador(off, parche, fs, ms);
        assert_eq!(
            gate, val,
            "los dos jueces discrepan en offset={:#x} parche={} file={} mem={}: \
             gate dice {} y el validador {}",
            off, parche, fs, ms, gate, val
        );
    }
}

/// **El desbordamiento es lo unico donde NO coinciden, y el gate es el bueno.**
///
/// El validador hace `rel.offset as usize + patch_size` con un `+` normal. En
/// `debug` eso entra en panico y en `release` da la vuelta y contesta que SI
/// cabe -- con un `offset` que viene del fichero, o sea de fuera.
///
/// *** No se arregla alla porque este crate no se ejecuta en la maquina: corre
/// en el anfitrion, dentro del compilador. Donde importa es en el cargador, y
/// ahi manda el gate, que usa `checked_add`. **Se deja escrito para que el dia
/// que alguien toque esa linea sepa que hay un caso que no comparten.**
#[test]
fn en_el_desbordamiento_el_gate_es_mas_estricto_y_es_a_proposito() {
    assert!(
        !bmo_bex_gate::reloc_cabe(u64::MAX, 8, 0x400, 0x400),
        "un offset imposible no puede caber"
    );
}

/// **LA VERSION DEL ABI, QUE ES LA OTRA COPIA -- y la que tenia la grieta.**
///
/// # Lo que se encontro el 2026-08-26
///
/// `bmo-abi` declara la regla en la misma frase que la define:
///
/// > *"Major versions are incompatible; minor versions are additive."*
///
/// Y el cargador --que es quien de verdad decide, porque el kernel llama al
/// gate y no a este crate-- la tenia escrita a mano y **de otra forma**:
///
/// ```text
///    if !((abi_mayor == 1 || abi_mayor == 2) && abi_menor == 0)
/// ```
///
/// *** `abi_menor == 0` no es aditivo: es exacto. El dia que el ABI subiera a
/// `2.1` --o sea el primer dia que se "mejorase" de la forma que el contrato
/// declara segura-- un `.bex` de `2.1` habria sido rechazado por el cargador
/// mientras el contrato decia que tenia que entrar.
///
/// No habia hecho dano porque nadie ha subido el menor nunca. Eso no es que
/// estuviera bien: es que todavia no se habia cobrado.
///
/// ** Esta prueba barre TODO el espacio pequeno de versiones en vez de mirar
/// las de hoy. Comprobar `(2,0)` no habria encontrado nada -- ahi las dos
/// copias coincidian. Lo que separa dos reglas no es el caso que se usa: es el
/// que todavia no.
#[test]
fn el_gate_y_el_contrato_admiten_las_mismas_versiones_del_abi() {
    for mayor in 0u8..=4 {
        for menor in 0u8..=4 {
            let gate = bmo_bex_gate::abi_admisible(mayor, menor);
            let contrato = bmo_abi::supports_abi((mayor, menor));
            assert_eq!(
                gate, contrato,
                "el cargador y el contrato discrepan en el ABI {}.{}: \
                 el gate dice {} y `supports_abi` {}",
                mayor, menor, gate, contrato
            );
        }
    }
}

/// **Y que la regla sea DE VERDAD aditiva en el menor**, no solo igual en los
/// dos sitios. Dos copias equivocadas de la misma forma tambien coinciden.
#[test]
fn el_menor_es_aditivo_en_los_dos() {
    let (mayor, menor) = bmo_abi::BMO_ABI_VERSION;
    for m in 0..=menor {
        assert!(
            bmo_abi::supports_abi((mayor, m)),
            "el contrato tendria que admitir {}.{}", mayor, m
        );
        assert!(
            bmo_bex_gate::abi_admisible(mayor, m),
            "el cargador tendria que admitir {}.{}", mayor, m
        );
    }
    assert!(
        !bmo_bex_gate::abi_admisible(mayor, menor + 1),
        "un binario que pide mas menor del que hay NO puede entrar: pide algo que \
         este sistema no implementa"
    );
}

/// **LA PRUEBA QUE DE VERDAD HABRIA CAZADO LA GRIETA.**
///
/// Las dos de arriba comparan las dos copias entre si, y eso **no basta**: con
/// el menor de hoy en cero, `menor <= 0` y `menor == 0` contestan lo mismo en
/// todas las versiones que existen. Las dos copias podian estar de acuerdo *y
/// las dos equivocadas*.
///
/// Aqui se le pregunta a la REGLA, con unos limites inventados, la unica
/// pregunta que la distingue: **si este sistema fuera el 2.2, entraria un
/// binario compilado contra el 2.1?** El contrato dice que si --el menor es
/// aditivo-- y una comprobacion de igualdad diria que no.
#[test]
fn el_menor_es_aditivo_de_verdad_y_no_solo_por_casualidad() {
    // Un sistema hipotetico 2.2.
    let admite = |mayor, menor| bmo_bex_gate::admisible_con(mayor, menor, 2, 2);

    assert!(admite(2, 0), "2.0 tiene que entrar en un sistema 2.2");
    assert!(admite(2, 1), "*** 2.1 tiene que entrar en un sistema 2.2: EL MENOR ES ADITIVO");
    assert!(admite(2, 2), "2.2 es el de casa");
    assert!(!admite(2, 3), "2.3 pide algo que este sistema no implementa");
    assert!(!admite(3, 0), "un mayor distinto es incompatible por definicion");
    assert!(!admite(1, 0), "el 1.0 ya no entra: solo sabia llamar a la tabla v1");
}

/// ** EL ENLAZADO DINAMICO: los dos jueces dicen que NO (2026-09-19).
///
/// Hasta hoy la puerta SALTABA una seccion de imports, exports o TLS como
/// "data para otro", y el validador la validaba. BMO-X enlaza estatico y no
/// tiene TLS: un binario que las trae cuenta con que alguien resuelva sus
/// llamadas al cargar, y no hay nadie. Y las dos BANDERAS que las anunciaban
/// (HAS_TLS, HAS_SHADERS) tambien se rechazan en los dos sitios.
#[test]
fn los_dos_rechazan_el_enlazado_dinamico() {
    assert_eq!(bmo_bex_gate::PIDEN_ENLAZADO_DINAMICO, bmo_abi::bef::validator::PIDEN_ENLAZADO_DINAMICO);
    let mut b = bmo_abi::bef::BefBuilder::new();
    b.add_section(bmo_abi::bef::BefSection::code(vec![0xC3; 16]));
    b.add_section(bmo_abi::bef::BefSection::rodata(vec![1; 16]));
    let buena = b.build().unwrap();
    assert!(bmo_abi::bef::validate(&buena).is_valid);
    assert!(bmo_bex_gate::revisar(&buena, buena.len()).is_ok());

    // La segunda entrada de la tabla (rodata) pasa a ser 0x05, 0x06, 0x0C.
    // Cabecera: `section_table_offset` en 32..40, `section_count` en 40..44.
    let tabla = u64::from_le_bytes(buena[32..40].try_into().unwrap()) as usize;
    let cuantas = u32::from_le_bytes(buena[40..44].try_into().unwrap()) as usize;
    for kind in bmo_bex_gate::PIDEN_ENLAZADO_DINAMICO {
        let mut img = buena.clone();
        let i = (0..cuantas).find(|&i| img[tabla + i * 48] == 0x02).expect("la rodata");
        img[tabla + i * 48] = kind;
        assert!(!bmo_abi::bef::validate(&img).is_valid, "el validador admite {kind:#04x}");
        assert_eq!(
            bmo_bex_gate::revisar(&img, img.len()).err(),
            Some(bmo_bex_gate::Falta::EnlazadoDinamico),
            "la puerta admite {kind:#04x}"
        );
    }
    for bandera in [bmo_bex_gate::FLAG_TLS, bmo_bex_gate::FLAG_SHADERS] {
        let mut img = buena.clone();
        let f = u32::from_le_bytes(img[8..12].try_into().unwrap()) | bandera;
        img[8..12].copy_from_slice(&f.to_le_bytes());
        assert!(!bmo_abi::bef::validate(&img).is_valid, "el validador admite la bandera {bandera:#x}");
        assert!(bmo_bex_gate::revisar(&img, img.len()).is_err(), "la puerta admite la bandera {bandera:#x}");
    }
}

/// **La arquitectura: los dos jueces dicen que NO a lo que no es x86-64.**
///
/// ** Hasta el 2026-09-18 no coincidian: la puerta rechazaba un `.bex` de ARM,
/// de RISC-V o sin arquitectura, y el validador lo daba por VALIDO con un
/// aviso. Ese dia el repositorio paso a ser SOLO x86-64 (`toolchain/tools/isa`)
/// y el validador dice lo mismo que la puerta. Esto impide que vuelvan a
/// separarse.
#[test]
fn los_dos_rechazan_lo_que_no_es_x86_64() {
    let mut b = bmo_abi::bef::BefBuilder::new();
    b.add_section(bmo_abi::bef::BefSection::code(vec![0xC3; 16]));
    let buena = b.build().unwrap();
    for arch in [0x00u8, 0x01, 0x02, 0x03, 0x7F] {
        let mut img = buena.clone();
        img[12] = arch;
        let val = bmo_abi::bef::validate(&img).is_valid;
        let gate = bmo_bex_gate::revisar(&img, img.len())
            .err()
            .map_or(true, |f| f != bmo_bex_gate::Falta::OtraArquitectura);
        assert_eq!(val, arch == bmo_bex_gate::ARCH_X86_64, "validador, arch {arch:#04x}");
        assert_eq!(gate, val, "la puerta y el validador discrepan en arch {arch:#04x}");
    }
}

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
    assert!(rev.es_bef2());
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
        bmo_abi::bef::signing::blake3_256(suyos)
    );
    // Y los RELOCS, que el kernel aplica, tambien: se nombran `0x80 | n`.
    let relocs = rev
        .secciones()
        .find(|s| s.kind == bmo_bex_gate::RELOCS)
        .unwrap();
    let suyos = &img[relocs.file_offset as usize..(relocs.file_offset + relocs.file_size) as usize];
    assert_eq!(
        digest_de(relocs.indice).expect("los relocs tienen digest"),
        bmo_abi::bef::signing::blake3_256(suyos)
    );
}
