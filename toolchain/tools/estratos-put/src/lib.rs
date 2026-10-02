//! Publica la compilacion nueva de PROTON-X en ESTRATOS sin reformatear.
//!
//! Solo sustituye `apps/proton-x.bex`: el contenido y las ramas nuevas se
//! escriben al final del log, se vacian al disco y entonces se publica el
//! superbloque alterno. Un corte antes del commit conserva la version anterior.

use std::collections::BTreeMap;
use std::io::{self, Read, Seek, SeekFrom, Write};

use bmo_estratos as es;
use bmo_estratos::carpeta::{self, Cambio, Veredicto};
use bmo_estratos::flujo::{plan_de, Arbol};
use bmo_estratos::objects::{
    Attr, BlockPtr, Nodo, Tipo, ATTR_ENTRADAS, ATTR_FIRMA, BLOQUE, NIVELES_MAX,
};
use bmo_estratos::read::Fuente;
// SUPER_LEN vive en la raiz de la crate, no en `objects`.
use bmo_estratos::{Autor, Estrato, Superblock, Transaccion, SUPER_LEN};

const NOMBRE_APLICACION: &str = "proton-x.bex";
const NOMBRE_CARPETA: &str = "apps";

/// E/S de un archivo o volumen ya abierto. La barrera se ejecuta antes del
/// commit; en un `File` de Windows, `sync_all` vacia tambien la cache del disco.
pub trait Almacen: Read + Write + Seek {
    fn barrera(&mut self) -> io::Result<()>;
}

impl Almacen for std::fs::File {
    fn barrera(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resultado {
    pub generacion: u64,
    pub bytes: u64,
    pub bloques_nuevos: u64,
    pub raiz: BlockPtr,
}

#[derive(Default)]
struct Cache {
    bloques: BTreeMap<u64, [u8; BLOQUE]>,
}

impl Fuente for Cache {
    fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
        match self.bloques.get(&lba) {
            Some(b) => {
                dst.copy_from_slice(b);
                true
            }
            None => false,
        }
    }
}

struct Capturar<'a, R> {
    reader: &'a mut R,
    cache: &'a mut Cache,
}

impl<R: Read + Seek> Fuente for Capturar<'_, R> {
    fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
        let offset = match lba.checked_mul(BLOQUE as u64) {
            Some(n) => n,
            None => return false,
        };
        if self.reader.seek(SeekFrom::Start(offset)).is_err()
            || self.reader.read_exact(dst).is_err()
        {
            return false;
        }
        self.cache.bloques.insert(lba, *dst);
        true
    }
}

fn leer_bloque<R: Read + Seek>(reader: &mut R, lba: u64) -> Result<[u8; BLOQUE], String> {
    let offset = lba
        .checked_mul(BLOQUE as u64)
        .ok_or_else(|| "LBA fuera de rango".to_string())?;
    reader
        .seek(SeekFrom::Start(offset))
        .and_then(|_| {
            let mut bloque = [0u8; BLOQUE];
            reader.read_exact(&mut bloque)?;
            Ok(bloque)
        })
        .map_err(|e| format!("leyendo el bloque {lba}: {e}"))
}

fn escribir_bloque<W: Write + Seek>(
    writer: &mut W,
    lba: u64,
    datos: &[u8],
) -> io::Result<()> {
    if datos.len() > BLOQUE {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bloque demasiado grande"));
    }
    let offset = lba
        .checked_mul(BLOQUE as u64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "LBA fuera de rango"))?;
    let mut bloque = [0u8; BLOQUE];
    bloque[..datos.len()].copy_from_slice(datos);
    writer.seek(SeekFrom::Start(offset))?;
    writer.write_all(&bloque)
}

fn escribir_objeto<W: Write + Seek>(
    writer: &mut W,
    lba: u64,
    datos: &[u8],
) -> Result<BlockPtr, String> {
    if datos.len() > BLOQUE {
        return Err("objeto ESTRATOS mayor que un bloque".into());
    }
    escribir_bloque(writer, lba, datos).map_err(|e| format!("escribiendo objeto: {e}"))?;
    Ok(BlockPtr::nuevo(lba, 0, datos))
}

fn leer_objeto<R: Read + Seek>(reader: &mut R, ptr: &BlockPtr) -> Result<Vec<u8>, String> {
    let bloque = leer_bloque(reader, ptr.lba)?;
    let inicio = ptr.off as usize;
    let fin = inicio
        .checked_add(ptr.len as usize)
        .filter(|&fin| fin <= BLOQUE)
        .ok_or_else(|| "puntero de objeto fuera del bloque".to_string())?;
    let datos = bloque[inicio..fin].to_vec();
    if !ptr.verifica(&datos) {
        return Err(format!("no cuadra la suma del bloque {}", ptr.lba));
    }
    Ok(datos)
}

fn leer_nodo<R: Read + Seek>(reader: &mut R, ptr: &BlockPtr) -> Result<Nodo, String> {
    Nodo::decode(&leer_objeto(reader, ptr)?).map_err(|e| e.name().into())
}

fn leer_flujo<R: Read + Seek>(
    reader: &mut R,
    cache: &mut Cache,
    attr: &Attr,
) -> Result<Vec<u8>, String> {
    if let Some(bytes) = attr.datos_residentes() {
        return Ok(bytes.to_vec());
    }
    let raiz = attr.raiz().ok_or("atributo sin raiz")?;
    let mut salida = Vec::new();
    let mut scratch = [[0u8; BLOQUE]; NIVELES_MAX + 1];
    let mut captura = Capturar { reader, cache };
    es::descender(
        &mut captura,
        &raiz,
        attr.levels,
        &mut scratch[..attr.levels as usize + 1],
        &mut |trozo| {
            salida.extend_from_slice(trozo);
            true
        },
    )
    .map_err(|e| e.name().to_string())?;
    let largo = usize::try_from(attr.size).map_err(|_| "flujo demasiado grande")?;
    salida.truncate(largo);
    if salida.len() != largo {
        return Err("el flujo no mide lo que declara el atributo".into());
    }
    Ok(salida)
}

struct Preparado {
    sb: Superblock,
    superbloque_en_uso: u64,
    estrato: Estrato,
    raiz: Nodo,
    raiz_entradas: Option<Attr>,
    apps: Nodo,
    apps_entradas: Option<Attr>,
    apps_veredicto: Veredicto,
    raiz_veredicto: Veredicto,
    cache: Cache,
}

fn preparar<R: Read + Seek>(
    reader: &mut R,
    disk_id: [u8; 32],
    generacion_esperada: u64,
    bloques_esperados: u64,
) -> Result<Preparado, String> {
    let a = leer_bloque(reader, es::SUPER_A_BLOCK)?;
    let b = leer_bloque(reader, es::SUPER_B_BLOCK)?;
    let (sb, cual) = es::pick_superblock(&a[..SUPER_LEN], &b[..SUPER_LEN])
        .map_err(|e| format!("superbloque: {}", e.name()))?;
    if sb.disk_id != disk_id {
        return Err("F: no coincide con el disco BMO-X esperado; no se escribe".into());
    }
    if sb.generation != generacion_esperada {
        return Err(format!(
            "generacion inesperada: se esperaba {generacion_esperada}, se encontro {}",
            sb.generation
        ));
    }
    if sb.total_blocks != bloques_esperados {
        return Err(format!(
            "medida inesperada: se esperaban {bloques_esperados} bloques, se encontraron {}",
            sb.total_blocks
        ));
    }

    let estrato = Estrato::decode(&leer_objeto(reader, &sb.estrato)?)
        .map_err(|e| format!("estrato: {}", e.name()))?;
    let raiz = leer_nodo(reader, &estrato.raiz)?;
    if raiz.tipo != Tipo::Directorio {
        return Err("la raiz de ESTRATOS no es un directorio".into());
    }
    let raiz_entradas = raiz.attr(ATTR_ENTRADAS).copied();

    let mut cache = Cache::default();
    if let Some(attr) = raiz_entradas.as_ref() {
        let _ = leer_flujo(reader, &mut cache, attr)?;
    }
    let mut scratch = [[0u8; BLOQUE]; NIVELES_MAX + 1];
    let apps_entrada = carpeta::buscar(
        &mut cache,
        raiz_entradas.as_ref(),
        NOMBRE_CARPETA,
        &mut scratch,
    )
    .map_err(|e| format!("leyendo la raiz: {}", e.name()))?
    .ok_or("no existe la carpeta apps/ de ESTRATOS")?;
    let apps = leer_nodo(reader, &apps_entrada.nodo)?;
    if apps.tipo != Tipo::Directorio {
        return Err("apps/ no es un directorio".into());
    }
    let apps_entradas = apps.attr(ATTR_ENTRADAS).copied();
    if let Some(attr) = apps_entradas.as_ref() {
        let _ = leer_flujo(reader, &mut cache, attr)?;
    }

    let mut scratch_raiz = [[0u8; BLOQUE]; NIVELES_MAX + 1];
    let apps_veredicto = carpeta::examinar(
        &mut cache,
        apps_entradas.as_ref(),
        &Cambio::Guardar { nombre: NOMBRE_APLICACION, nodo: BlockPtr::NULO },
        &mut scratch_raiz,
    )
    .map_err(|e| format!("apps/: {e:?}"))?;
    let mut scratch_apps = [[0u8; BLOQUE]; NIVELES_MAX + 1];
    let raiz_veredicto = carpeta::examinar(
        &mut cache,
        raiz_entradas.as_ref(),
        &Cambio::Guardar { nombre: NOMBRE_CARPETA, nodo: BlockPtr::NULO },
        &mut scratch_apps,
    )
    .map_err(|e| format!("raiz: {e:?}"))?;

    Ok(Preparado {
        sb,
        superbloque_en_uso: cual,
        estrato,
        raiz,
        raiz_entradas,
        apps,
        apps_entradas,
        apps_veredicto,
        raiz_veredicto,
        cache,
    })
}

fn grabar_arbol<W: Write + Seek>(
    writer: &mut W,
    datos: &[u8],
    base: u64,
) -> Result<(BlockPtr, u64), String> {
    let plan = plan_de(datos.len() as u64).ok_or("el BEX esta vacio o es demasiado grande")?;
    let mut indice = [[0u8; BLOQUE]; NIVELES_MAX];
    let mut arbol = Arbol::nuevo(plan, base, &mut indice[..plan.niveles as usize])
        .map_err(|e| e.name().to_string())?;
    let mut poner = |lba: u64, trozo: &[u8]| escribir_bloque(writer, lba, trozo).is_ok();
    for trozo in datos.chunks(BLOQUE) {
        arbol.empujar(trozo, &mut poner).map_err(|e| e.name().to_string())?;
    }
    let raiz = arbol.cerrar(&mut poner).map_err(|e| e.name().to_string())?;
    Ok((raiz, plan.total))
}

fn grabar_lista<W: Write + Seek>(
    writer: &mut W,
    cache: &mut Cache,
    previo: Option<&Attr>,
    cambio: &Cambio<'_>,
    veredicto: &Veredicto,
    base: u64,
) -> Result<(Option<(BlockPtr, u8)>, u64), String> {
    let mut scratch = [[0u8; BLOQUE]; NIVELES_MAX + 1];
    let mut indice = [[0u8; BLOQUE]; NIVELES_MAX];
    let mut paso = [0u8; BLOQUE];
    let mut poner = |lba: u64, datos: &[u8]| escribir_bloque(writer, lba, datos).is_ok();
    let nueva = carpeta::reescribir(
        cache,
        previo,
        cambio,
        veredicto,
        base,
        &mut scratch,
        &mut indice,
        &mut paso,
        &mut poner,
    )
    .map_err(|e| e.name().to_string())?;
    Ok((nueva, veredicto.bloques()))
}

/// Instala o actualiza solo `apps/proton-x.bex` en un volumen existente.
///
/// `disk_id`, generacion y medida son comprobaciones obligatorias de identidad
/// y estado; se hacen antes de reservar o escribir cualquier bloque.
pub fn instalar<W: Almacen>(
    disco: &mut W,
    bytes: &[u8],
    disk_id: [u8; 32],
    generacion_esperada: u64,
    bloques_esperados: u64,
) -> Result<Resultado, String> {
    let mut preparado = preparar(disco, disk_id, generacion_esperada, bloques_esperados)?;
    let datos_plan = plan_de(bytes.len() as u64).ok_or("el BEX esta vacio o es demasiado grande")?;
    let bloques = datos_plan.total
        + 1 // nodo del fichero
        + preparado.apps_veredicto.bloques()
        + 1 // nodo apps/
        + preparado.raiz_veredicto.bloques()
        + 1 // nodo raiz
        + 1; // estrato
    let mut transaccion = Transaccion::open(
        &preparado.sb,
        preparado.superbloque_en_uso,
        true,
    )
    .map_err(|e| e.name().to_string())?;
    let base = transaccion.reserve(bloques).map_err(|e| e.name().to_string())?;
    let mut cursor = base;

    let (datos_raiz, usados) = grabar_arbol(disco, bytes, cursor)?;
    if usados != datos_plan.total {
        return Err("el arbol de datos no uso lo que dijo el plan".into());
    }
    cursor += usados;
    let nodo_datos = bmo_estratos::escritura::nodo_de_fichero_grande(
        bytes.len() as u64,
        datos_plan.niveles,
        datos_raiz,
    )
    .map_err(|e| e.name().to_string())?;
    let firma = es::blake3(bytes);
    let archivo = Nodo::decode(&nodo_datos)
        .map_err(|e| e.name().to_string())?
        .con(Attr::residente(ATTR_FIRMA, &firma).map_err(|e| e.name().to_string())?)
        .map_err(|e| e.name().to_string())?
        .encode();
    let archivo_ptr = escribir_objeto(disco, cursor, &archivo)?;
    cursor += 1;

    let cambio_apps = Cambio::Guardar { nombre: NOMBRE_APLICACION, nodo: archivo_ptr };
    let (apps_lista, usados) = grabar_lista(
        disco,
        &mut preparado.cache,
        preparado.apps.attr(ATTR_ENTRADAS),
        &cambio_apps,
        &preparado.apps_veredicto,
        cursor,
    )?;
    cursor += usados;
    let apps_bytes = carpeta::nodo_de(apps_lista, &preparado.apps_veredicto)
        .map_err(|e| e.name().to_string())?;
    let apps_ptr = escribir_objeto(disco, cursor, &apps_bytes)?;
    cursor += 1;

    let cambio_raiz = Cambio::Guardar { nombre: NOMBRE_CARPETA, nodo: apps_ptr };
    let (raiz_lista, usados) = grabar_lista(
        disco,
        &mut preparado.cache,
        preparado.raiz.attr(ATTR_ENTRADAS),
        &cambio_raiz,
        &preparado.raiz_veredicto,
        cursor,
    )?;
    cursor += usados;
    let raiz_bytes = carpeta::nodo_de(raiz_lista, &preparado.raiz_veredicto)
        .map_err(|e| e.name().to_string())?;
    let raiz_ptr = escribir_objeto(disco, cursor, &raiz_bytes)?;
    cursor += 1;

    let nuevo_estrato = Estrato::new(
        raiz_ptr,
        preparado.sb.estrato,
        0,
        Autor::Herramienta,
        "",
    );
    let estrato_ptr = escribir_objeto(disco, cursor, &nuevo_estrato.encode())?;
    cursor += 1;
    if cursor != base + bloques {
        return Err("la transaccion no escribio todos los bloques reservados".into());
    }

    transaccion.cerrar_datos().map_err(|e| e.name().to_string())?;
    disco.barrera().map_err(|e| format!("vaciando los datos antes del commit: {e}"))?;
    transaccion.barrera_hecha().map_err(|e| e.name().to_string())?;
    let (destino, nuevo_sb) = transaccion
        .commit(estrato_ptr)
        .map_err(|e| e.name().to_string())?;
    let offset = destino
        .checked_mul(BLOQUE as u64)
        .ok_or_else(|| "offset del superbloque fuera de rango".to_string())?;
    disco
        .seek(SeekFrom::Start(offset))
        .and_then(|_| disco.write_all(&nuevo_sb.encode()))
        .map_err(|e| format!("publicando el superbloque alterno: {e}"))?;
    disco
        .barrera()
        .map_err(|e| format!("vaciando el commit: {e}"))?;

    verificar_objetivo(disco, &firma, bytes)?;
    Ok(Resultado {
        generacion: nuevo_sb.generation,
        bytes: bytes.len() as u64,
        bloques_nuevos: bloques,
        raiz: nuevo_estrato.raiz,
    })
}

fn verificar_objetivo<R: Read + Seek>(reader: &mut R, firma: &[u8; 32], esperado: &[u8]) -> Result<(), String> {
    let a = leer_bloque(reader, es::SUPER_A_BLOCK)?;
    let b = leer_bloque(reader, es::SUPER_B_BLOCK)?;
    let (sb, _) = es::pick_superblock(&a[..SUPER_LEN], &b[..SUPER_LEN])
        .map_err(|e| e.name().to_string())?;
    let estrato = Estrato::decode(&leer_objeto(reader, &sb.estrato)?)
        .map_err(|e| e.name().to_string())?;
    let raiz = leer_nodo(reader, &estrato.raiz)?;
    let mut cache = Cache::default();
    let raiz_attr = raiz.attr(ATTR_ENTRADAS).copied();
    if let Some(attr) = raiz_attr.as_ref() {
        let _ = leer_flujo(reader, &mut cache, attr)?;
    }
    let mut scratch = [[0u8; BLOQUE]; NIVELES_MAX + 1];
    let apps = carpeta::buscar(&mut cache, raiz_attr.as_ref(), NOMBRE_CARPETA, &mut scratch)
        .map_err(|e| e.name().to_string())?
        .ok_or("readback: falta apps/")?;
    let apps_node = leer_nodo(reader, &apps.nodo)?;
    let apps_attr = apps_node.attr(ATTR_ENTRADAS).copied();
    if let Some(attr) = apps_attr.as_ref() {
        let _ = leer_flujo(reader, &mut cache, attr)?;
    }
    let archivo = carpeta::buscar(&mut cache, apps_attr.as_ref(), NOMBRE_APLICACION, &mut scratch)
        .map_err(|e| e.name().to_string())?
        .ok_or("readback: falta apps/proton-x.bex")?;
    let archivo_node = leer_nodo(reader, &archivo.nodo)?;
    if archivo_node.attr(ATTR_FIRMA).and_then(Attr::datos_residentes) != Some(firma.as_slice()) {
        return Err("readback: la firma de ESTRATOS no coincide".into());
    }
    let datos = archivo_node.attr(":datos").ok_or("readback: BEX sin :datos")?;
    let mut cache_datos = Cache::default();
    let recibidos = leer_flujo(reader, &mut cache_datos, datos)?;
    if recibidos != esperado {
        return Err("readback: los bytes publicados no coinciden con el BEX".into());
    }
    Ok(())
}

/// Crea un fichero con firma en memoria de disco para las pruebas de la transaccion.
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File, OpenOptions};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_BLOCKS: u64 = 2048;
    const TEST_ID: [u8; 32] = es::disk_id(b"TEST DISK", b"TEST-SERIAL", 4096);

    fn put_block(file: &mut File, lba: u64, data: &[u8]) {
        escribir_bloque(file, lba, data).unwrap();
    }

    fn seed(file: &mut File) -> BlockPtr {
        file.set_len(TEST_BLOCKS * BLOQUE as u64).unwrap();
        let old = b"previous app";
        let old_hash = es::blake3(old);
        let old_data = Nodo::decode(&bmo_estratos::escritura::nodo_de_fichero(old).unwrap())
            .unwrap()
            .con(Attr::residente(ATTR_FIRMA, &old_hash).unwrap())
            .unwrap()
            .encode();
        let old_ptr = BlockPtr::nuevo(2, 0, &old_data);
        put_block(file, 2, &old_data);

        let old_entry = Entrada::nueva("hola.bex", old_ptr).unwrap();
        let app_entries = old_entry.encode();
        let app_entries_ptr = BlockPtr::nuevo(3, 0, &app_entries);
        put_block(file, 3, &app_entries);
        let app_dir = bmo_estratos::escritura::nodo_de_directorio(
            app_entries_ptr,
            app_entries.len() as u64,
        )
        .unwrap();
        let app_dir_ptr = BlockPtr::nuevo(4, 0, &app_dir);
        put_block(file, 4, &app_dir);

        let root_entry = Entrada::nueva("apps", app_dir_ptr).unwrap();
        let root_entries = root_entry.encode();
        let root_entries_ptr = BlockPtr::nuevo(5, 0, &root_entries);
        put_block(file, 5, &root_entries);
        let root_node = bmo_estratos::escritura::nodo_de_directorio(
            root_entries_ptr,
            root_entries.len() as u64,
        )
        .unwrap();
        let root_ptr = BlockPtr::nuevo(6, 0, &root_node);
        put_block(file, 6, &root_node);

        let estrato = Estrato::new(root_ptr, BlockPtr::NULO, 0, Autor::Herramienta, "");
        let estrato_ptr = BlockPtr::nuevo(7, 0, &estrato.encode());
        put_block(file, 7, &estrato.encode());
        let mut sb = Superblock::new(TEST_ID, TEST_BLOCKS);
        sb.generation = 30;
        sb.log_head = 8;
        sb.estrato = estrato_ptr;
        for lba in [es::SUPER_A_BLOCK, es::SUPER_B_BLOCK] {
            file.seek(SeekFrom::Start(lba * BLOQUE as u64)).unwrap();
            file.write_all(&sb.encode()).unwrap();
        }
        file.sync_all().unwrap();
        estrato_ptr
    }

    fn read_target<R: Read + Seek>(
        reader: &mut R,
        estrato_ptr: BlockPtr,
        name: &str,
    ) -> Vec<u8> {
        let estrato = Estrato::decode(&leer_objeto(reader, &estrato_ptr).unwrap()).unwrap();
        let root = leer_nodo(reader, &estrato.raiz).unwrap();
        let mut cache = Cache::default();
        let root_attr = root.attr(ATTR_ENTRADAS).copied();
        let mut scratch = [[0u8; BLOQUE]; NIVELES_MAX + 1];
        let app = {
            if let Some(attr) = root_attr.as_ref() {
                let _ = leer_flujo(reader, &mut cache, attr).unwrap();
            }
            carpeta::buscar(&mut cache, root_attr.as_ref(), "apps", &mut scratch)
                .unwrap()
                .unwrap()
        };
        let app_dir = leer_nodo(reader, &app.nodo).unwrap();
        let app_attr = app_dir.attr(ATTR_ENTRADAS).copied();
        if let Some(attr) = app_attr.as_ref() {
            let _ = leer_flujo(reader, &mut cache, attr).unwrap();
        }
        let file = carpeta::buscar(&mut cache, app_attr.as_ref(), name, &mut scratch)
            .unwrap()
            .unwrap();
        let node = leer_nodo(reader, &file.nodo).unwrap();
        let mut content_cache = Cache::default();
        leer_flujo(reader, &mut content_cache, node.attr(":datos").unwrap()).unwrap()
    }

    #[test]
    fn publica_bex_sin_perder_la_version_ni_el_estrato_anterior() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bmo-estratos-put-{}-{nonce}.img",
            std::process::id()
        ));
        let mut disk = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let anterior = seed(&mut disk);
        let primero = vec![0x42; BLOQUE * 2 + 37];
        let r1 = instalar(&mut disk, &primero, TEST_ID, 30, TEST_BLOCKS).unwrap();
        assert_eq!(r1.generacion, 31);
        assert_eq!(read_target(&mut disk, r1.raiz, NOMBRE_APLICACION), primero);
        assert_eq!(read_target(&mut disk, anterior, "hola.bex"), b"previous app");

        let segundo = b"updated BMO-X Proton-X";
        let r2 = instalar(&mut disk, segundo, TEST_ID, 31, TEST_BLOCKS).unwrap();
        assert_eq!(r2.generacion, 32);
        assert_eq!(read_target(&mut disk, r2.raiz, NOMBRE_APLICACION), segundo);
        assert_eq!(read_target(&mut disk, r1.raiz, NOMBRE_APLICACION), primero);
        drop(disk);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn identidad_generacion_o_medida_incorrectas_no_escriben() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bmo-estratos-put-gate-{}-{nonce}.img",
            std::process::id()
        ));
        let mut disk = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let _ = seed(&mut disk);
        let before = fs::read(&path).unwrap();
        assert!(instalar(&mut disk, b"bex", [0; 32], 30, TEST_BLOCKS).is_err());
        assert!(instalar(&mut disk, b"bex", TEST_ID, 29, TEST_BLOCKS).is_err());
        assert!(instalar(&mut disk, b"bex", TEST_ID, 30, TEST_BLOCKS - 1).is_err());
        drop(disk);
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(path).unwrap();
    }
}
