use super::*;

static BLOQUES_MEMORIA: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

fn memoria_para_mapeo(bytes: usize) -> Option<u64> {
    let layout = std::alloc::Layout::from_size_align(bytes, 64 * 1024).ok()?;
    // SAFETY: el bloque se conserva hasta que la prueba vacia el monton.
    let base = unsafe { std::alloc::alloc_zeroed(layout) };
    if base.is_null() {
        return None;
    }
    BLOQUES_MEMORIA.lock().unwrap().push((base as usize, bytes));
    Some(base as u64)
}

fn soltar_bloques_de_prueba() {
    crate::memoria::reiniciar();
    for (base, bytes) in core::mem::take(&mut *BLOQUES_MEMORIA.lock().unwrap()) {
        let layout = std::alloc::Layout::from_size_align(bytes, 64 * 1024).unwrap();
        // SAFETY: cada bloque viene de `alloc_zeroed` con este layout.
        unsafe { std::alloc::dealloc(base as *mut u8, layout) };
    }
}

fn plataforma_prueba_mapeo() -> crate::Plataforma {
    let mut p = plataforma_prueba_trozos();
    p.memoria = memoria_para_mapeo;
    p
}

#[test]
fn mapview_solo_lectura_de_un_archive_grande_trae_el_rango_pedido() {
    let _una = UNA_A_LA_VEZ.lock().unwrap();
    {
        let mut v = VOLUMEN.lock().unwrap();
        *v = Volumen::default();
        for d in [
            "",
            "d:",
            "d:Cyberpunk 2077",
            "d:Cyberpunk 2077/archive",
            "d:Cyberpunk 2077/archive/pc",
            "d:Cyberpunk 2077/archive/pc/content",
        ] {
            v.carpetas.insert(String::from(d));
        }
        v.ficheros
            .insert(String::from(ARCHIVO_GIGANTE), Vec::new());
    }
    // SAFETY: prueba serializada; la vista y el fichero usan el banco.
    unsafe { crate::empezar(plataforma_prueba_mapeo()) };
    crate::ficheros::poner_directorio("d:Cyberpunk 2077/bin/x64");
    crate::ficheros::poner_capa(None);

    let nombre: Vec<u16> =
        "D:\\Cyberpunk 2077\\archive\\pc\\content\\basegame_4_gamedata.archive"
            .encode_utf16()
            .chain([0])
            .collect();
    let h = create_file_dentro(nombre.as_ptr(), GENERIC_READ, 0, 0, OPEN_EXISTING, 0, 0);
    assert_ne!(h, NO_VALE);
    let m = crate::kernel32_mapeo::crear_mapeo_solo_lectura_para_prueba(h);
    assert_ne!(m, 0);

    // La vista empieza despues de 4 GiB, mide 64 KiB y no reserva los 5 GiB.
    let desde = 1u64 << 32;
    let vista = crate::kernel32_mapeo::mapear_rango_para_prueba(m, desde, 64 * 1024);
    assert_ne!(vista, 0);
    // SAFETY: MapViewOfFile acaba de reservar y llenar al menos 32 bytes.
    let bytes = unsafe { core::slice::from_raw_parts(vista as *const u8, 32) };
    for (k, byte) in bytes.iter().enumerate() {
        assert_eq!(*byte, desde.wrapping_add(k as u64) as u8);
    }

    assert_eq!(crate::kernel32_mapeo::desmapear_para_prueba(vista), 1);
    assert_eq!(crate::kernel32_mapeo::cerrar(m), 1);
    assert_eq!(cerrar(h), 1);
    soltar_bloques_de_prueba();
}
