//! Las pruebas de `largo.rs` (12-10): nombres largos, carpetas y agregar.
//! Salieron de `pruebas.rs` por L6a moviendo texto: ni una linea cambia, y
//! usan el disco de mentira de alli (`super`).

use super::*;

// == NOMBRES LARGOS, CARPETAS Y AGREGAR (12-10, `largo.rs`) ==================

/// Lee entero lo que `hallar` encuentra.
fn leer_largo(v: &mut FatVolume, dir: u32, n: &str) -> Option<Vec<u8>> {
    let h = v.hallar(dir, n.as_bytes())?;
    let mut d = vec![0u8; h.entrada.size as usize + 512];
    let k = v.read_file(h.entrada.first_cluster, h.entrada.size, &mut d);
    d.truncate(k.min(h.entrada.size as usize));
    Some(d)
}

#[test]
fn una_carpeta_de_nombre_largo_se_crea_y_se_encuentra() {
    let (_turno, mut v) = volumen();
    let c = v.crear_carpeta(2, b"datos-componentes").expect("debe crear");
    // Por el largo, en cualquier caja, y por el alias de Windows.
    for n in ["datos-componentes", "DATOS-COMPONENTES", "DATOS-~1"] {
        let h = v.hallar(2, n.as_bytes()).unwrap_or_else(|| panic!("no encuentra {n}"));
        assert!(h.es_dir);
        assert_eq!(h.entrada.first_cluster, c);
    }
    // Crear otra vez devuelve la misma (mkdir -p), no otra.
    assert_eq!(v.crear_carpeta(2, b"Datos-Componentes").unwrap(), c);
    // Y dentro estan `.` y `..` (la raiz se apunta como 0).
    let (n0, d0, _) = v.entry_at(c, 0).unwrap();
    let (n1, d1, _) = v.entry_at(c, 1).unwrap();
    assert_eq!((&n0, d0, &n1, d1), (b".          ", true, b"..         ", true));
}

#[test]
fn carpetas_anidadas_y_un_fichero_largo_dentro() {
    let (_turno, mut v) = volumen();
    let inf = v.crear_carpeta(2, b"informe").unwrap();
    let gpu = v.crear_carpeta(inf, b"gpu-relojes").unwrap();
    v.guardar_como(gpu, b"informe-maestro.txt", b"todo bien").unwrap();
    assert_eq!(leer_largo(&mut v, gpu, "informe-maestro.txt").unwrap(), b"todo bien");
    // El listado da el nombre LARGO (y el 8.3 a quien no lo tiene).
    let mut n = [0u8; 255];
    let mut nombres = vec![];
    let mut k = 0;
    while let Some((l, _, _)) = v.entrada_larga(gpu, k, &mut n) {
        nombres.push(String::from_utf8_lossy(&n[..l]).to_string());
        k += 1;
    }
    assert_eq!(nombres, [".", "..", "informe-maestro.txt"]);
    // Un 8.3 de toda la vida se sigue escribiendo SIN entradas largas.
    v.guardar_como(inf, b"salida.txt", b"x").unwrap();
    assert!(v.find_file_in(&name("SALIDA  TXT"), inf).is_some());
}

#[test]
fn dos_nombres_largos_parecidos_tienen_alias_distintos() {
    let (_turno, mut v) = volumen();
    v.guardar_como(2, b"captura-de-la-gpu.txt", b"uno").unwrap();
    v.guardar_como(2, b"captura-de-la-cpu.txt", b"dos").unwrap();
    assert_eq!(leer_largo(&mut v, 2, "captura-de-la-gpu.txt").unwrap(), b"uno");
    assert_eq!(leer_largo(&mut v, 2, "captura-de-la-cpu.txt").unwrap(), b"dos");
    assert!(v.hallar(2, b"CAPTUR~1.TXT").is_some());
    assert!(v.hallar(2, b"CAPTUR~2.TXT").is_some());
}

/// Con un cluster de un sector, la raiz llena son 16 entradas. Antes el
/// diecisiete era `DirFull`; ahora el directorio crece.
#[test]
fn el_directorio_crece_cuando_se_llena() {
    let (_turno, mut v) = volumen();
    for i in 0..40 {
        v.guardar_como(2, format!("fichero-numero-{i}.txt").as_bytes(), format!("{i}").as_bytes()).unwrap();
    }
    for i in 0..40 {
        let d = leer_largo(&mut v, 2, &format!("fichero-numero-{i}.txt")).unwrap_or_else(|| panic!("falta {i}"));
        assert_eq!(d, format!("{i}").as_bytes());
    }
}

#[test]
fn agregar_al_final_sin_fin() {
    let (_turno, mut v) = volumen();
    let inf = v.crear_carpeta(2, b"informe").unwrap();
    let mut todo = Vec::new();
    // Trozos que cruzan sectores y clusters (spc = 1), de tallas raras.
    for (k, t) in [1usize, 7, 300, 511, 512, 513, 2000, 3].into_iter().enumerate() {
        let trozo: Vec<u8> = (0..t).map(|i| (i * 7 + k * 13) as u8).collect();
        let m = v.agregar(inf, b"vivo-sin-final.log", &trozo).unwrap();
        todo.extend_from_slice(&trozo);
        assert_eq!(m as usize, todo.len());
        assert_eq!(leer_largo(&mut v, inf, "vivo-sin-final.log").unwrap(), todo, "tras el trozo {k}");
    }
    // Ni un cluster de mas: los que pide la medida, la carpeta y la raiz.
    let clusters = todo.len().div_ceil(512);
    assert_eq!(ocupados(&mut v), clusters + 2);
}

#[test]
fn agregar_a_uno_vacio_y_reemplazar_despues() {
    let (_turno, mut v) = volumen();
    v.guardar_como(2, b"vacio.txt", b"").unwrap();
    v.agregar(2, b"vacio.txt", b"hola").unwrap();
    assert_eq!(leer_largo(&mut v, 2, "vacio.txt").unwrap(), b"hola");
    v.guardar_como(2, b"vacio.txt", b"otra cosa").unwrap();
    assert_eq!(leer_largo(&mut v, 2, "vacio.txt").unwrap(), b"otra cosa");
}

#[test]
fn nombres_que_fat_no_guarda_se_rechazan() {
    let (_turno, mut v) = volumen();
    for n in [&b""[..], b"a/b", b"que?", b"fin ", b"..", b"...", b"\xC3\xB1"] {
        assert!(matches!(v.guardar_como(2, n, b"x"), Err(WriteError::Nombre)), "{n:?}");
    }
    // Un fichero no se convierte en carpeta.
    v.guardar_como(2, b"nota.txt", b"x").unwrap();
    assert!(matches!(v.crear_carpeta(2, b"nota.txt"), Err(WriteError::Exists)));
}

#[test]
fn el_tope_de_fat32_se_dice() {
    let (_turno, mut v) = volumen();
    v.guardar_como(2, b"grande.bin", b"abc").unwrap();
    // Se le miente la medida a la entrada: 4 GiB - 2.
    let h = v.hallar(2, b"grande.bin").unwrap();
    let mut s = [0u8; 512];
    assert!(read(h.entrada.lba, 1, &mut s));
    s[h.entrada.offset + 28..h.entrada.offset + 32].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
    assert!(write(h.entrada.lba, 1, &s));
    assert!(matches!(v.agregar(2, b"grande.bin", b"xy"), Err(WriteError::Grande)));
}

/// **Contra un FAT32 de VERDAD** (12-10): `BMO_FAT32_IMAGEN=img cargo test
/// contra_una_imagen -- --ignored` sobre una imagen de `mkfs.fat -F 32`; y
/// despues `fsck.fat -n img` y `mdir -i img -/ ::` tienen que leerla limpia,
/// como la leeria Windows. Sin la variable no hace nada.
#[test]
#[ignore]
fn contra_una_imagen() {
    use std::io::{Read, Seek, SeekFrom, Write};
    struct Imagen(std::sync::Mutex<std::fs::File>, u64);
    impl bmo_block::BlockDevice for Imagen {
        fn identity(&self) -> bmo_block::DeviceId {
            let mut id = bmo_block::DeviceId::EMPTY;
            id.blocks = self.1;
            id
        }
        fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, bmo_block::BlockError> {
            let mut f = self.0.lock().unwrap();
            f.seek(SeekFrom::Start(lba * 512)).map_err(|_| bmo_block::BlockError::Device)?;
            f.read_exact(&mut buf[..count as usize * 512]).map_err(|_| bmo_block::BlockError::Device)?;
            Ok(count)
        }
        fn write(&self, lba: u64, count: u16, data: &[u8]) -> Result<u16, bmo_block::BlockError> {
            let mut f = self.0.lock().unwrap();
            f.seek(SeekFrom::Start(lba * 512)).map_err(|_| bmo_block::BlockError::Device)?;
            f.write_all(&data[..count as usize * 512]).map_err(|_| bmo_block::BlockError::Device)?;
            Ok(count)
        }
        fn flush(&self) -> Result<(), bmo_block::BlockError> { Ok(()) }
        fn writable(&self) -> bool { true }
    }
    // Al reves (12-10): una imagen que escribio OTRO (mtools/Linux), y BMO-X
    // la lee: `BMO_FAT32_AJENA=img`, con lo que deja el script de la prueba.
    if let Ok(ruta) = std::env::var("BMO_FAT32_AJENA") {
        let f = std::fs::OpenOptions::new().read(true).write(true).open(&ruta).unwrap();
        let largo = f.metadata().unwrap().len() / 512;
        let dev: &'static Imagen = Box::leak(Box::new(Imagen(std::sync::Mutex::new(f), largo)));
        let mut v = mount(dev, true, 0).expect("monta");
        let raiz = v.root_cluster();
        let c = v.hallar(raiz, b"Datos-Componentes").expect("la carpeta de mtools").entrada.first_cluster;
        let h = v.hallar(c, b"un nombre largo con espacios.txt").expect("el fichero de mtools");
        let mut d = vec![0u8; h.entrada.size as usize];
        v.read_file(h.entrada.first_cluster, h.entrada.size, &mut d);
        assert_eq!(d, b"escrito por mtools\n");
        let mut n = [0u8; 255];
        let mut vistos = vec![];
        let mut k = 0;
        while let Some((l, _, _)) = v.entrada_larga(c, k, &mut n) {
            vistos.push(String::from_utf8_lossy(&n[..l]).to_string());
            k += 1;
        }
        assert!(vistos.contains(&"un nombre largo con espacios.txt".to_string()), "{vistos:?}");
        assert!(vistos.contains(&"minus.txt".to_string()), "{vistos:?}");
        // Y BMO-X escribe al lado sin romper nada (luego lo juzga fsck).
        v.agregar(c, b"un nombre largo con espacios.txt", b"y bmo-x agrega\n").unwrap();
        return;
    }
    let Ok(ruta) = std::env::var("BMO_FAT32_IMAGEN") else { return };
    let f = std::fs::OpenOptions::new().read(true).write(true).open(&ruta).unwrap();
    let largo = f.metadata().unwrap().len() / 512;
    let dev: &'static Imagen = Box::leak(Box::new(Imagen(std::sync::Mutex::new(f), largo)));
    let mut v = mount(dev, true, 0).expect("monta");
    v.poner_reloj(Some((2026, 10, 12, 15, 6, 30)));
    let raiz = v.root_cluster();
    let inf = v.crear_carpeta(raiz, b"informe").unwrap();
    let comp = v.crear_carpeta(raiz, b"datos-componentes").unwrap();
    let gpu = v.crear_carpeta(inf, b"gpu-relojes").unwrap();
    v.guardar_como(comp, b"mov.txt", b"1050\n2075\n").unwrap();
    v.guardar_como(inf, b"Salida-Mixta.txt", b"caja mezclada\n").unwrap();
    v.guardar_como(inf, b"LEEME.TXT", b"mayusculas\n").unwrap();
    v.guardar_como(inf, b"Nota.Md", b"8.3 con caja mezclada\n").unwrap();
    for i in 0..300 {
        v.guardar_como(gpu, format!("captura-numero-{i:03}.txt").as_bytes(), format!("captura {i}\n").as_bytes()).unwrap();
    }
    let mut todo = Vec::new();
    for k in 0..200usize {
        let t: Vec<u8> = format!("linea {k} del informe que no se acaba nunca\n").into_bytes().repeat(k % 37 + 1);
        v.agregar(inf, b"informe-sin-final.log", &t).unwrap();
        todo.extend_from_slice(&t);
    }
    std::fs::write(format!("{ruta}.esperado"), &todo).unwrap();
}
