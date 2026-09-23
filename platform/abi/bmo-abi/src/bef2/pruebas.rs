//! El banco de BEF2: una imagen buena, y **una mutacion por cada falta**.
//!
//! ** El criterio es el de `gate_y_validador_no_se_separan`: una prueba que
//! solo mira el caso bueno no distingue un juez de un `Ok(())`.

use super::*;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Un programa minimo: codigo, constantes, datos, ceros y un reloc que pone en
/// los datos la direccion de una cadena de las constantes.
fn buena() -> Vec<u8> {
    let mut e = Escritor::ejecutable();
    e.codigo(vec![0xC3; 64])
        .constantes(b"hola\0".to_vec())
        .datos(vec![0u8; 8])
        .ceros(4096)
        .entrada(0)
        .reloc(Reloc {
            donde: Region::Datos,
            destino: Region::Constantes,
            offset: 0,
            addend: 0,
        });
    e.construir().expect("la imagen buena tiene que salir")
}

/// Donde empieza una region, leyendo la cabecera como la lee el kernel.
fn tramo(img: &[u8], o: usize) -> (usize, usize) {
    (
        u32_en(img, o).unwrap() as usize,
        u32_en(img, o + 4).unwrap() as usize,
    )
}

#[test]
fn una_imagen_buena_pasa_y_dice_lo_que_lleva() {
    let img = buena();
    let v = leer(&img).expect("tiene que pasar");
    assert!(v.es_ejecutable() && !v.es_objeto() && !v.quiere_pantalla());
    assert_eq!(v.region(Region::Codigo).len(), 64);
    assert_eq!(v.region(Region::Constantes), b"hola\0");
    assert_eq!(v.region(Region::Datos).len(), 8);
    assert_eq!(v.ceros, 4096);
    assert_eq!(v.memoria(), 64 + 5 + 8 + 4096);
    assert_eq!(v.relocs().count(), 1);
    assert!(v.anexo(ANEXO_REQUISITOS).is_some());
    assert!(v.anexo(ANEXO_FIRMA).is_some());
    // Lo que el kernel lee para mapear cabe en la cabecera: 64 B.
    assert_eq!(CABECERA, 64);
}

/// ** La cuenta que motiva el formato: BEF1 gastaba 48 B de cabecera mas 48
/// por cada seccion (siete en un programa asi: code, rodata, data, bss,
/// relocs, requisitos, firma) = 384 B de prologo. BEF2 son 64 + 16 por anexo.
#[test]
fn el_prologo_es_mas_chico_que_el_de_bef1() {
    let img = buena();
    let cuantos = u32_en(&img, 20).unwrap() as usize;
    let prologo = CABECERA + cuantos * ANEXO;
    assert!(prologo <= 112, "prologo de {prologo} B");
    assert!(prologo < 48 + 7 * 48, "BEF1 gastaba 384 B en lo mismo");
}

#[test]
fn las_regiones_no_se_pisan_y_los_anexos_tampoco() {
    let img = buena();
    let v = leer(&img).unwrap();
    let mut trozos: Vec<(usize, usize)> = vec![(0, CABECERA + v.cuantos_anexos() * ANEXO)];
    for o in [24usize, 32, 40] {
        let (off, len) = tramo(&img, o);
        if len > 0 {
            trozos.push((off, off + len));
        }
    }
    for a in v.anexos() {
        trozos.push((
            a.tramo.offset as usize,
            a.tramo.offset as usize + a.tramo.bytes as usize,
        ));
    }
    for i in 0..trozos.len() {
        for j in i + 1..trozos.len() {
            let (a, b) = (trozos[i], trozos[j]);
            assert!(a.0 >= b.1 || b.0 >= a.1, "{a:?} pisa a {b:?}");
        }
    }
}

#[test]
fn con_paginas_cada_region_empieza_en_una() {
    let mut e = Escritor::ejecutable();
    e.codigo(vec![0xC3; 64])
        .constantes(b"hola\0".to_vec())
        .datos(vec![1u8; 8])
        .alinear_a_pagina(true);
    let img = e.construir().unwrap();
    leer(&img).expect("tiene que seguir valiendo");
    for o in [24usize, 32, 40] {
        let (off, len) = tramo(&img, o);
        if len > 0 {
            assert_eq!(off % 4096, 0, "la region en {o} no empieza en pagina");
        }
    }
}

// -- Una mutacion por cada falta -------------------------------------------

fn falta_de(cambio: impl FnOnce(&mut Vec<u8>)) -> Falta {
    let mut img = buena();
    cambio(&mut img);
    leer(&img).err().expect("esto NO puede pasar por bueno")
}

#[test]
fn el_juez_caza_cada_mentira_de_la_cabecera() {
    assert_eq!(leer(&[]).err(), Some(Falta::Corta));
    assert_eq!(falta_de(|i| i[0] = b'X'), Falta::OtroFormato);
    assert_eq!(falta_de(|i| i[4] = 3), Falta::OtroAbi);
    assert_eq!(falta_de(|i| i[5] |= 1 << 7), Falta::BanderaDesconocida);
    assert_eq!(falta_de(|i| i[5] = EJECUTABLE | OBJETO), Falta::NiEjecutableNiObjeto);
    assert_eq!(falta_de(|i| i[5] = 0), Falta::NiEjecutableNiObjeto);
    assert_eq!(falta_de(|i| i[6] = 1), Falta::ReservadoNoEsCero);
    assert_eq!(falta_de(|i| i[56] = 1), Falta::ReservadoNoEsCero);
    // AVX: el kernel no guarda los ymm en un cambio de contexto.
    assert_eq!(
        falta_de(|i| i[8..16].copy_from_slice(&(XCR0_X87 | XCR0_SSE | XCR0_AVX).to_le_bytes())),
        Falta::EstadoDeCpuQueNoSePreserva
    );
    assert_eq!(falta_de(|i| i[52] = 0), Falta::TotalNoCuadra);
    assert_eq!(
        falta_de(|i| i[16..20].copy_from_slice(&9999u32.to_le_bytes())),
        Falta::EntradaFueraDelCodigo
    );
    assert_eq!(
        falta_de(|i| i[20..24].copy_from_slice(&99u32.to_le_bytes())),
        Falta::DemasiadosAnexos
    );
}

#[test]
fn el_juez_caza_una_region_imposible() {
    // El codigo dice medir mas que el fichero.
    assert_eq!(
        falta_de(|i| i[28..32].copy_from_slice(&0xFFFF_0000u32.to_le_bytes())),
        Falta::SeSaleDelFichero
    );
    // Las constantes se mudan encima del codigo.
    assert_eq!(
        falta_de(|i| {
            let (codigo, _) = tramo(i, 24);
            i[32..36].copy_from_slice(&(codigo as u32).to_le_bytes());
        }),
        Falta::SeSolapan
    );
    // Un ejecutable sin codigo.
    assert_eq!(
        falta_de(|i| i[28..32].copy_from_slice(&0u32.to_le_bytes())),
        Falta::SinCodigo
    );
    // ** Una region VACIA con el offset fuera del fichero. La encontro la
    // pasada hostil: no ocupaba sitio, el juez la saltaba, y `region()` la
    // rebanaba y panicaba. Ahora es una falta con nombre, ANTES de la firma.
    assert_eq!(
        falta_de(|i| {
            i[40..44].copy_from_slice(&0xFFFF_0000u32.to_le_bytes());
            i[44..48].copy_from_slice(&0u32.to_le_bytes());
        }),
        Falta::SeSaleDelFichero
    );
    // Y un anexo vacio, que la puerta del kernel ya rechazaba.
    assert_eq!(
        falta_de(|i| i[CABECERA + 8..CABECERA + 12].copy_from_slice(&0u32.to_le_bytes())),
        Falta::AnexoQueNoVaAqui
    );
}

#[test]
fn el_juez_caza_un_reloc_que_apunta_fuera() {
    let img = buena();
    let v = leer(&img).unwrap();
    let a = v.anexos().find(|a| a.tipo == ANEXO_RELOCS).unwrap();
    let o = a.tramo.offset as usize;
    // Escribir en los CEROS: no tienen bytes donde escribir.
    assert_eq!(falta_de(|i| i[o] = Region::Ceros as u8), Falta::RelocMal);
    // Un destino que no es una region.
    assert_eq!(falta_de(|i| i[o + 1] = 9), Falta::RelocMal);
    // Ocho bytes que no caben en la region donde se escriben.
    assert_eq!(
        falta_de(|i| i[o + 4..o + 8].copy_from_slice(&7u32.to_le_bytes())),
        Falta::RelocMal
    );
    // Un addend mas alla del final de su region.
    assert_eq!(
        falta_de(|i| i[o + 8..o + 16].copy_from_slice(&999u64.to_le_bytes())),
        Falta::RelocMal
    );
}

#[test]
fn un_byte_cambiado_en_cualquier_sitio_rompe_su_hash() {
    let img = buena();
    let v = leer(&img).unwrap();
    // En el codigo.
    let (codigo, _) = tramo(&img, 24);
    assert_eq!(falta_de(|i| i[codigo] ^= 0xFF), Falta::NoCuadraElHash);
    // En las constantes.
    let (cte, _) = tramo(&img, 32);
    assert_eq!(falta_de(|i| i[cte] ^= 0xFF), Falta::NoCuadraElHash);
    // Y en los RELOCS, que el kernel aplica: si no estuvieran firmados, quien
    // tocara uno haria que el kernel escribiera donde el quisiera.
    //
    // ** El addend se mueve a 1, que SIGUE SIENDO VALIDO (cabe en las
    // constantes): asi lo que lo caza es la firma y no la comprobacion de
    // limites. Un cambio invalido saldria por `RelocMal` y esta fila no
    // probaria lo que dice probar.
    let a = v.anexos().find(|a| a.tipo == ANEXO_RELOCS).unwrap();
    let o = a.tramo.offset as usize;
    assert_eq!(falta_de(|i| i[o + 8] = 1), Falta::NoCuadraElHash);
}

/// Se le quita el ULTIMO hash a la firma: el anexo sigue bien formado y
/// cubre menos. En la imagen buena el ultimo es el de los requisitos (el
/// orden es indice, regiones, anexos en el orden de la tabla).
#[test]
fn una_firma_que_no_cubre_un_anexo_no_vale() {
    let img = buena();
    let v = leer(&img).unwrap();
    let f = v.anexos().find(|a| a.tipo == ANEXO_FIRMA).unwrap().tramo.offset as usize;
    let cuantos = u32_en(&img, f).unwrap() as usize;
    assert_eq!(
        falta_de(|i| {
            i[f..f + 4].copy_from_slice(&((cuantos - 1) as u32).to_le_bytes());
            let fin = f + FIRMA_CABECERA + (cuantos - 1) * FIRMA_HASH;
            let total = i.len();
            i.drain(fin..total);
            let n = i.len() as u32;
            i[52..56].copy_from_slice(&n.to_le_bytes());
            let anexo = CABECERA + (v.cuantos_anexos() - 1) * ANEXO;
            let nuevo = (FIRMA_CABECERA + (cuantos - 1) * FIRMA_HASH) as u32;
            i[anexo + 8..anexo + 12].copy_from_slice(&nuevo.to_le_bytes());
        }),
        Falta::FirmaIncompleta
    );
}

/// *** LA FIRMA ES DEL INDICE (2026-09-20). Hasta hoy un `.bex` firmado
/// admitia que le cambiaran la entrada o los ceros: el hash de cada region
/// cuadraba igual, porque la region no habia cambiado. Habia cambiado a DONDE
/// saltaba el kernel.
#[test]
fn la_firma_cubre_el_indice_y_tocar_la_cabecera_la_rompe() {
    let img = buena();
    let v = leer(&img).unwrap();
    let firma = v.anexo(ANEXO_FIRMA).unwrap();
    // La primera entrada es el indice, y su hash es el de cabecera + tabla.
    assert_eq!(firma[FIRMA_CABECERA], FIRMA_INDICE);
    let esperado = crate::bef::blake3::blake3_256(v.indice());
    assert_eq!(&firma[FIRMA_CABECERA + 8..FIRMA_CABECERA + FIRMA_HASH], &esperado[..]);
    assert_eq!(v.indice().len(), CABECERA + v.cuantos_anexos() * ANEXO);
    // Otra entrada VALIDA (el codigo mide 64): ninguna region cambia, y aun
    // asi no pasa. Es exactamente el ataque que antes no se veia.
    assert_eq!(falta_de(|i| i[16] = 1), Falta::NoCuadraElHash);
    // Mas ceros: la memoria que pide cambia sin tocar un byte firmado.
    assert_eq!(
        falta_de(|i| i[48..52].copy_from_slice(&8192u32.to_le_bytes())),
        Falta::NoCuadraElHash
    );
}

/// Una firma SIN la entrada del indice no vale, aunque cubra todo lo demas.
#[test]
fn una_firma_sin_el_indice_no_vale() {
    let img = buena();
    let v = leer(&img).unwrap();
    let f = v.anexos().find(|a| a.tipo == ANEXO_FIRMA).unwrap().tramo.offset as usize;
    let cuantos = u32_en(&img, f).unwrap() as usize;
    let idx_firma = v.cuantos_anexos() - 1;
    assert_eq!(
        falta_de(|i| {
            // Se quita la PRIMERA entrada corriendo las demas hacia arriba;
            // el anexo mide 40 menos y lo que sobra al final queda sin propietario.
            let ini = f + FIRMA_CABECERA;
            i.copy_within(ini + FIRMA_HASH..ini + cuantos * FIRMA_HASH, ini);
            i[f..f + 4].copy_from_slice(&((cuantos - 1) as u32).to_le_bytes());
            let anexo = CABECERA + idx_firma * ANEXO;
            let nuevo = (FIRMA_CABECERA + (cuantos - 1) * FIRMA_HASH) as u32;
            i[anexo + 8..anexo + 12].copy_from_slice(&nuevo.to_le_bytes());
        }),
        Falta::FirmaIncompleta
    );
}

/// ** LA FIRMA CUBRE TODOS LOS ANEXOS, tambien los que el kernel no lee: los
/// recursos de una app tienen su hash en la misma tabla, y la firma de autor
/// responde por ellos. Antes (19-09) solo cubria relocs y requisitos, y un
/// icono o un WAD dentro del paquete se podia cambiar sin que nadie lo viera.
#[test]
fn la_firma_cubre_los_anexos_que_el_kernel_no_lee() {
    let mut e = Escritor::de_imagen(&buena()).unwrap();
    e.anexo(ANEXO_RECURSOS, vec![7u8; 300]);
    let img = e.construir().unwrap();
    let v = leer(&img).expect("con recursos tambien vale");
    let firma = v.anexo(ANEXO_FIRMA).unwrap();
    let cuantos = u32_en(firma, 0).unwrap() as usize;
    // indice + 3 regiones + relocs + requisitos + recursos.
    assert_eq!(cuantos, 7);
    let rec = v.anexos().position(|a| a.tipo == ANEXO_RECURSOS).unwrap() as u8;
    let ult = FIRMA_CABECERA + (cuantos - 1) * FIRMA_HASH;
    assert_eq!(firma[ult], FIRMA_ANEXO | rec);
    // Un byte del icono cambiado ya no pasa.
    let o = v.anexos().find(|a| a.tipo == ANEXO_RECURSOS).unwrap().tramo.offset as usize;
    let mut rota = img.clone();
    rota[o] ^= 0xFF;
    assert_eq!(leer(&rota).err(), Some(Falta::NoCuadraElHash));
    // Y una firma que no lo cubra tampoco.
    let f = v.anexos().find(|a| a.tipo == ANEXO_FIRMA).unwrap().tramo.offset as usize;
    let idx_firma = v.anexos().position(|a| a.tipo == ANEXO_FIRMA).unwrap();
    let mut sin = img.clone();
    sin[f..f + 4].copy_from_slice(&((cuantos - 1) as u32).to_le_bytes());
    let anexo = CABECERA + idx_firma * ANEXO;
    let nuevo = (FIRMA_CABECERA + (cuantos - 1) * FIRMA_HASH) as u32;
    sin[anexo + 8..anexo + 12].copy_from_slice(&nuevo.to_le_bytes());
    assert_eq!(leer(&sin).err(), Some(Falta::FirmaIncompleta));
}

/// ** Los SIMBOLOS viajan tambien en un ejecutable, y esta fila existe porque
/// el primer esquema los prohibia: el DIRECTOR los lee para anotar una autopsia
/// (`services/director/src/simbolos.rs`), asi que prohibirlos habria roto esa
/// anotacion sin que nadie se enterara hasta el siguiente fallo en el Ryzen.
#[test]
fn un_ejecutable_lleva_sus_simbolos_para_la_autopsia() {
    let mut e = Escritor::ejecutable();
    e.codigo(vec![0xC3; 16]).anexo(ANEXO_SIMBOLOS, vec![7u8; 8]);
    let img = e.construir().unwrap();
    let v = leer(&img).expect("los simbolos son data para OTRO, no un error");
    assert_eq!(v.anexo(ANEXO_SIMBOLOS).unwrap(), &[7u8; 8]);
    // Y el kernel no los abre.
    assert!(!lo_lee_el_kernel(ANEXO_SIMBOLOS));
}

#[test]
fn un_objeto_si_los_lleva_y_no_pide_firma() {
    let mut e = Escritor::objeto();
    e.codigo(vec![0xC3; 16]).anexo(ANEXO_SIMBOLOS, vec![9u8; 8]);
    let img = e.construir().unwrap();
    let v = leer(&img).expect("un objeto es una imagen valida");
    assert!(v.es_objeto() && !v.es_ejecutable());
    assert_eq!(v.anexo(ANEXO_SIMBOLOS).unwrap(), &[9u8; 8]);
}

#[test]
fn los_requisitos_dicen_la_memoria_de_la_imagen() {
    let img = buena();
    let v = leer(&img).unwrap();
    let t = crate::bef::requisitos::Tabla::abrir(v.anexo(ANEXO_REQUISITOS).unwrap())
        .expect("la tabla se tiene que abrir");
    assert_eq!(
        t.total_de(crate::bef::requisitos::CLASE_MEMORIA),
        v.memoria()
    );
}

#[test]
fn lo_que_declara_el_programa_viaja_con_lo_que_mide() {
    let mut e = Escritor::ejecutable();
    e.codigo(vec![0xC3; 16]).requerir(Requisito {
        clase: crate::bef::requisitos::CLASE_PANTALLA,
        unidad: crate::bef::requisitos::UNIDAD_UNIDADES,
        obligatorio: true,
        cantidad: 1,
        motivo: String::from("dibuja"),
    });
    let img = e.construir().unwrap();
    let v = leer(&img).unwrap();
    let t = crate::bef::requisitos::Tabla::abrir(v.anexo(ANEXO_REQUISITOS).unwrap()).unwrap();
    assert_eq!(t.total_de(crate::bef::requisitos::CLASE_PANTALLA), 1);
}

#[test]
fn el_escritor_no_deja_poner_los_anexos_que_fabrica_el() {
    let mut e = Escritor::ejecutable();
    e.codigo(vec![0xC3; 16]).anexo(ANEXO_FIRMA, vec![0u8; 8]);
    assert!(e.construir().is_err());
}

#[test]
fn un_anexo_desconocido_se_lleva_y_no_estorba() {
    // La unica tolerancia del formato: data para OTRO.
    let mut e = Escritor::ejecutable();
    e.codigo(vec![0xC3; 16]).anexo(0x40, b"para otro".to_vec());
    let img = e.construir().unwrap();
    let v = leer(&img).expect("un anexo desconocido no es un error");
    assert_eq!(v.anexo(0x40).unwrap(), b"para otro");
    assert!(!lo_lee_el_kernel(0x40));
}

/// **Reabrir una imagen, cambiarla y reescribirla**: lo que hacen `bmo-pack`
/// (agregar recursos) y `bmo-firmar` (poner la firma de autor).
#[test]
fn una_imagen_se_reabre_se_le_agrega_y_sigue_valiendo() {
    let antes = buena();
    let mut e = Escritor::de_imagen(&antes).expect("se reabre");
    e.anexo(ANEXO_RECURSOS, b"un wad".to_vec());
    let despues = e.construir().unwrap();

    let a = leer(&antes).unwrap();
    let d = leer(&despues).expect("la reescrita tambien vale");
    // Lo que el programa ES no cambia.
    assert_eq!(a.region(Region::Codigo), d.region(Region::Codigo));
    assert_eq!(a.region(Region::Constantes), d.region(Region::Constantes));
    assert_eq!(a.ceros, d.ceros);
    assert_eq!(a.entrada, d.entrada);
    assert_eq!(a.relocs().count(), d.relocs().count());
    // Y lo nuevo viaja.
    assert_eq!(d.anexo(ANEXO_RECURSOS).unwrap(), b"un wad");
    // La firma se rehace: cubre tambien lo que se anadio, si el kernel lo lee.
    assert!(d.anexo(ANEXO_FIRMA).is_some());
}

/// Lo que el programa DECLARO (pantalla, audio) sobrevive a reabrir la imagen
/// -- que es lo que le pasa a todo `.ibx` con icono al pasar por `bmo-pack` --
/// y la linea de memoria no se duplica: se rehace.
#[test]
fn lo_declarado_sobrevive_a_reabrir() {
    use crate::bef::requisitos::{Tabla, CLASE_MEMORIA, CLASE_PANTALLA, UNIDAD_UNIDADES};
    let mut e = Escritor::ejecutable();
    e.codigo(vec![0xC3; 16]).requerir(Requisito {
        clase: CLASE_PANTALLA,
        unidad: UNIDAD_UNIDADES,
        obligatorio: true,
        cantidad: 1,
        motivo: String::from("dibuja"),
    });
    let antes = e.construir().unwrap();
    let mut r = Escritor::de_imagen(&antes).unwrap();
    r.anexo(ANEXO_RECURSOS, b"icono".to_vec());
    let despues = r.construir().unwrap();
    let v = leer(&despues).unwrap();
    let t = Tabla::abrir(v.anexo(ANEXO_REQUISITOS).unwrap()).unwrap();
    assert_eq!(t.total_de(CLASE_PANTALLA), 1, "la pantalla declarada se perdio al reabrir");
    assert_eq!(t.cuantos(), 2, "una linea de memoria y una de pantalla, ni una mas");
    let mem = t.iter().find(|r| r.clase == CLASE_MEMORIA).unwrap();
    assert_eq!(mem.cantidad, 16, "la memoria se rehace con la medida de ahora");
    assert_eq!(t.iter().find(|r| r.clase == CLASE_PANTALLA).map(|r| t.motivo(&r)), Some("dibuja"));
}

/// **Empaquetar dos veces REEMPLAZA.** Un `.bex` no puede llevar dos anexos
/// del mismo tipo (la puerta del kernel lo rechaza), asi que `anexo` sobre un
/// tipo que ya viene de `de_imagen` tiene que quitar el viejo. Antes lo
/// agregaba: `empaquetar` prometia "mete (o reemplaza)" y un `.bex`
/// reempaquetado --el build de cada dia con `sombra.bex` y su BSF-- salia
/// con dos y no arrancaba.
#[test]
fn un_anexo_repetido_reemplaza_al_viejo() {
    let mut e = Escritor::de_imagen(&buena()).unwrap();
    e.anexo(ANEXO_RECURSOS, b"viejo".to_vec());
    let una = e.construir().unwrap();
    let mut e = Escritor::de_imagen(&una).unwrap();
    e.anexo(ANEXO_RECURSOS, b"nuevo".to_vec());
    let dos = e.construir().unwrap();
    let v = leer(&dos).expect("reempaquetada tiene que pasar");
    assert_eq!(v.anexos().filter(|a| a.tipo == ANEXO_RECURSOS).count(), 1);
    assert_eq!(v.anexo(ANEXO_RECURSOS), Some(&b"nuevo"[..]));
    // Y por `empaquetar`, dos veces seguidas, lo mismo.
    let p1 = paquete::empaquetar(&buena(), &[("a", b"1")]).unwrap();
    let p2 = paquete::empaquetar(&p1, &[("a", b"2")]).unwrap();
    let v = leer(&p2).unwrap();
    assert_eq!(v.anexos().filter(|a| a.tipo == ANEXO_RECURSOS).count(), 1);
    assert_eq!(paquete::directorio(&p2).unwrap().len(), 1);
}

/// La firma de AUTOR: 64 de firma y 32 de clave detras de los hashes.
#[test]
fn una_firma_ed25519_viaja_en_el_anexo() {
    let mut e = Escritor::de_imagen(&buena()).unwrap();
    e.ed25519([7u8; 64], [9u8; 32]);
    let img = e.construir().unwrap();
    let v = leer(&img).expect("firmada tambien vale");
    let firma = v.anexo(ANEXO_FIRMA).unwrap();
    let cuantos = u32_en(firma, 0).unwrap() as usize;
    assert_eq!(u32_en(firma, 4).unwrap(), ALGO_ED25519);
    let s = FIRMA_CABECERA + cuantos * FIRMA_HASH;
    assert_eq!(&firma[s..s + 64], &[7u8; 64]);
    assert_eq!(&firma[s + 64..s + 96], &[9u8; 32]);
    // Y hay una cadena que firmar: un solo numero que resume la imagen.
    assert!(v.cadena_de_hashes().is_some());
}

/// Un algoritmo de firma que este sistema no conoce NO se ignora.
#[test]
fn un_algoritmo_de_firma_desconocido_no_pasa() {
    let img = buena();
    let v = leer(&img).unwrap();
    let f = v.anexos().find(|a| a.tipo == ANEXO_FIRMA).unwrap().tramo.offset as usize;
    assert_eq!(falta_de(|i| i[f + 4] = 9), Falta::FirmaMal);
}
