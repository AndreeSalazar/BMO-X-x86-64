//! **Los pasos de `save mode`**, en el orden en que hay que darlos: la tabla
//! que recorre `verificar.rs`, y lo poco que la tabla necesita para decidir
//! si un paso esta hecho.
//!
//! [consumo] NADA      es una tabla; la recorre `verificar.rs`
//!
//! Partido de `verificar.rs` el 24-09: la tabla crecio con cada escalon de
//! la 3060 (M5, 47 pasos) y el fichero paso de las 1000 lineas de codigo que
//! permite el censo modular. Es TEXTO MOVIDO: los mismos pasos, en el mismo
//! orden.

use bmo_userland as bmo;

/// Un paso de la verificacion.
pub(crate) struct Paso {
    /// El nombre con el que se quita: `-iommu`, `-gpu`.
    pub(crate) nombre: &'static [u8],
    /// Que hace, en una linea.
    pub(crate) que: &'static [u8],
    /// Ya esta hecho (no se repite).
    pub(crate) hecho: fn() -> bool,
    /// Darlo. `Ok` con el valor del kernel, o el motivo del NO.
    pub(crate) dar: fn() -> Result<u64, u32>,
    /// El paso que tiene que estar hecho antes, si lo hay.
    pub(crate) pide: Option<&'static [u8]>,
    /// Lo que mirar despues si salio bien.
    pub(crate) consejo: &'static [u8],
    /// Tras darlo, repintar el escritorio entero: `init` cambia BAR1 y lo
    /// que habia en pantalla era de antes (L0c4b3a).
    pub(crate) repinta: bool,
}

fn iommu_encendida() -> bool {
    bmo::info(bmo::INFO_IOMMU_VIVA) & bmo::IOMMU_VIVA_ENCENDIDA != 0
}

/// CIEGA o TRADUCIDA: las dos le quitan la RAM. Sin lo segundo, `save mode`
/// volveria a cegar una 3060 ya traducida y desharia M0d.
fn gpu_ciega() -> bool {
    bmo::info(bmo::INFO_IOMMU_GPU) & (bmo::IOMMU_GPU_CIEGA | bmo::IOMMU_GPU_TRADUCIDA) != 0
}

fn gpu_traducida() -> bool {
    bmo::info(bmo::INFO_IOMMU_GPU) & bmo::IOMMU_GPU_TRADUCIDA != 0
}

fn traducir_gpu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_TRADUCIR_GPU)
}

fn prueba_prestada() -> bool {
    bmo::info(bmo::INFO_GPU_PRUEBA) & bmo::GPU_PRUEBA_PRESTADA != 0
}

fn prestar_prueba() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_PRESTAR_PRUEBA)
}

fn fuego_hecho() -> bool {
    bmo::info(bmo::INFO_GPU_FUEGO) & bmo::FUEGO_HECHO != 0
}

/// La prueba de fuego cuenta solo con las 1024 palabras: 1023 es un NO.
fn fuego() -> Result<u64, u32> {
    let v = bmo::iommu_orden(bmo::IOMMU_OP_GPU_FUEGO)?;
    if !fuego_hecho() {
        return Err(super::gpu::NO_FUEGO_A_MEDIAS);
    }
    Ok(v)
}

fn frontera_hecha() -> bool {
    bmo::info(bmo::INFO_GPU_FRONTERA) & bmo::FUEGO_HECHO != 0
}

fn frontera() -> Result<u64, u32> {
    let v = bmo::iommu_orden(bmo::IOMMU_OP_GPU_FRONTERA)?;
    if v == 0 {
        return Err(super::gpu::NO_SIN_FRONTERA);
    }
    Ok(v)
}

fn encender_iommu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_ENCENDER)
}

fn cegar_gpu() -> Result<u64, u32> {
    bmo::iommu_orden(bmo::IOMMU_OP_CEGAR_GPU)
}

/// E2 cuenta como hecho si esta ARMADO y ya llego al menos un VBLANK: un
/// aviso encendido que no avisa no es un paso dado.
fn e2_avisa() -> bool {
    let v = bmo::info(bmo::INFO_GPU_VBLANK);
    v & bmo::E2_ARMADO != 0 && v & 0xFFFF_FFFF > 0
}

/// Encender E2 y ESCUCHAR 300 ms: a 60 Hz son ~18 avisos. Si no llega
/// ninguno, el paso no salio -- aunque el kernel dijera que si.
fn encender_e2() -> Result<u64, u32> {
    let v = bmo::iommu_orden(bmo::IOMMU_OP_E2_ENCENDER)?;
    let (n, _) = super::gpu::contar_vblanks(300);
    if n == 0 {
        return Err(super::gpu::NO_E2_MUDO);
    }
    Ok(v)
}

/// **Los pasos, en el orden en que hay que darlos.** Ver
/// `docs/plan/PLAN_LA_3060.md`: M0c, M0e, E2, M0d2 y M0d3.
pub(crate) const PASOS: &[Paso] = &[
    Paso {
        nombre: b"iommu",
        que: b"encender la IOMMU, todo de paso (M0c)",
        hecho: iommu_encendida,
        dar: encender_iommu,
        pide: None,
        consejo: b"usa la maquina un par de minutos (DOOM, el disco, el raton) y teclea `iommu`: los eventos tienen que seguir en 0",
        repinta: false,
    },
    Paso {
        nombre: b"gpu",
        que: b"cegar la 3060: su DMA no alcanza la RAM (M0e)",
        hecho: gpu_ciega,
        dar: cegar_gpu,
        pide: Some(b"iommu"),
        consejo: b"la 3060 ya no puede tocar tu RAM ni con el Bus Master encendido; lo siguiente es E2, su VBLANK por MSI",
        repinta: false,
    },
    Paso {
        nombre: b"e2",
        que: b"la 3060 AVISA del VBLANK por MSI, con el Bus Master tras el candado (E2)",
        hecho: e2_avisa,
        dar: encender_e2,
        pide: Some(b"gpu"),
        consejo: b"teclea `gpu`: la fila `e2` tiene que subir ~60 por segundo y la escalera en `+`; `iommu` con los eventos en 0",
        repinta: false,
    },
    Paso {
        nombre: b"traducir",
        que: b"la 3060 TRADUCIDA por su dominio: ve SOLO lo prestado, hoy nada (M0d2)",
        hecho: gpu_traducida,
        dar: traducir_gpu,
        pide: Some(b"gpu"),
        consejo: b"`iommu`: la fila 3060 dice TRADUCIDA, sin fila `event`; y `gpu`: la fila `e2` sigue subiendo (el MSI pasa la entrada traducida)",
        repinta: false,
    },
    Paso {
        nombre: b"prestar",
        que: b"prestarle la pagina de prueba, solo lectura, en 0x10000000 (M0d2)",
        hecho: prueba_prestada,
        dar: prestar_prueba,
        pide: Some(b"traducir"),
        consejo: b"`iommu`: la fila `domain` dice 1 pagina prestada; lo siguiente es M0d3, que un falcon de la 3060 la LEA por DMA",
        repinta: false,
    },
    Paso {
        nombre: b"fuego",
        que: b"LA PRUEBA DE FUEGO: el DMA del falcon del GSP lee la pagina prestada (M0d3)",
        hecho: fuego_hecho,
        dar: fuego,
        pide: Some(b"prestar"),
        consejo: b"`gpu`: la fila `fuego` dice 1024 de 1024 -- la 3060 leyo tu RAM, y solo lo prestado",
        repinta: false,
    },
    Paso {
        nombre: b"frontera",
        que: b"LA FRONTERA: una direccion NO prestada sale como fallo de pagina de la 3060 (M0d3)",
        hecho: frontera_hecha,
        dar: frontera,
        pide: Some(b"fuego"),
        consejo: b"`iommu`: la fila `event` dice FALLO de pagina, BDF 29:00.0, direccion 0x20000000 -- la venda existe",
        repinta: false,
    },
    Paso {
        nombre: b"vbios",
        que: b"leer la VBIOS, hallar FWSEC y su firma para este fusible; solo lectura (L0a)",
        hecho: super::vbios::lista,
        dar: super::vbios::paso,
        pide: None,
        consejo: b"`gpu`: filas `vbios`, `fwsec`, `fusible`, `vram` y `wpr2`; la ROM queda en datos/vbios.rom -- lo siguiente es L0b, correr FWSEC-FRTS",
        repinta: false,
    },
    Paso {
        nombre: b"fwsec",
        que: b"CORRER FWSEC-FRTS: el firmware firmado de la VBIOS monta la WPR2 (L0b)",
        hecho: super::vbios::hay_wpr2,
        dar: super::vbios::correr_fwsec,
        pide: Some(b"vbios"),
        consejo: b"`gpu`: la fila `wpr2` dice YA montada donde se pidio, y `frts` CORRIO -- la puerta del booter y del GSP",
        repinta: false,
    },
    Paso {
        nombre: b"gsp",
        que: b"leer el firmware del GSP de fw/gsp/, la firma del booter para el fusible del SEC2 y el reparto de la VRAM; solo lectura (L0c1)",
        hecho: super::gsp::hecho,
        dar: super::gsp::leer,
        pide: None,
        consejo: b"`gpu`: filas `booter`, `fusible`, `bootldr`, `gsp-rm`, `mapa` y `cuadra` -- lo siguiente es L0c2, prestar el GSP-RM por la radix3",
        repinta: false,
    },
    Paso {
        nombre: b"radix",
        que: b"PRESTAR el GSP-RM a la 3060 por su radix3 y releerlo entero por la IOMMU, con su BLAKE3; no arranca nada (L0c2)",
        hecho: super::gsp::radix_hecho,
        dar: super::gsp::radix,
        pide: Some(b"gsp"),
        consejo: b"`gpu`: la fila `radix` dice PRESTADO y la 570.144 entera, blake3 e7856ee2b387917b; `iommu`: `domain` ~15.600 paginas y sin eventos nuevos -- lo siguiente es L0c3, el booter en el SEC2",
        repinta: false,
    },
    Paso {
        nombre: b"libos",
        que: b"PRESTAR para escribir lo que el GSP escribe (LIBOS, logs, rmargs, colas) y seguir cada puntero por la IOMMU; no arranca nada (L0c3a)",
        hecho: super::gsp::libos_hecho,
        dar: super::gsp::libos,
        pide: Some(b"radix"),
        consejo: b"`gpu`: la fila `libos` dice PRESTADO para escribir y cada puntero lleva a lo suyo; `iommu`: `domain` 180 paginas mas y sin eventos nuevos -- lo siguiente es `sistema`, escribirle SetSystemInfo y SetRegistry antes de despertarlo",
        repinta: false,
    },
    Paso {
        nombre: b"sistema",
        que: b"ESCRIBIRLE AL GSP por primera vez: SetSystemInfo y SetRegistry en la cola de la CPU, antes de despertarlo (L0c4b2a)",
        hecho: super::gspsistema::mandado,
        dar: super::gspsistema::mandar,
        pide: Some(b"libos"),
        consejo: b"`gpu`: la fila `sistema` dice los dos con suma 0, `sysinfo` las BAR y el PCI de tu 3060, y `leyo` que el GSP LOS LEYO (su puntero en 2) tras `despertar`",
        repinta: false,
    },
    Paso {
        nombre: b"despertar",
        que: b"DESPERTAR EL GSP: sus argumentos en el buzon, el booter firmado en el SEC2, y su RISC-V encendido (L0c3b)",
        hecho: super::gsp::despierto,
        dar: super::gsp::despertar,
        pide: Some(b"sistema"),
        consejo: b"`gpu`: la fila `despierto` dice el RISC-V ACTIVO y `gsplog` que el GSP ESCRIBIO; sus logs en datos/gsplog.bin -- lo siguiente es `cola`, leer lo que dijo",
        repinta: false,
    },
    Paso {
        nombre: b"cola",
        que: b"LEER lo que el GSP ya dijo: los mensajes de su cola, sin contestar ni mover un puntero (L0c4a)",
        hecho: super::gspcola::leida,
        dar: super::gspcola::leer,
        pide: Some(b"despertar"),
        consejo: b"`gpu`: la fila `cola` dice cuantos mensajes, todos con firma y suma; `dijo` que tipos; la cola cruda en datos/gspcola.bin -- lo siguiente es `vaciar`",
        repinta: false,
    },
    Paso {
        nombre: b"vaciar",
        que: b"CONSUMIR lo que el GSP solo cuenta (NOCAT, LIBOS_PRINT) moviendo el puntero de lectura de la CPU; lo que pide algo se queda (L0c4b1)",
        hecho: super::gspvaciar::vaciada,
        dar: super::gspvaciar::vaciar,
        pide: Some(b"cola"),
        consejo: b"`gpu`: la fila `vacia` dice cuantos consumidos, `nocat` lo que traian en claro y `pide` el primero que espera respuesta; crudos en datos/gspnocat.bin -- lo siguiente es `secuenciador`, leer lo que pide",
        repinta: false,
    },
    Paso {
        nombre: b"secuenciador",
        que: b"LEER el secuenciador que pide el GSP, orden a orden, SIN correr ninguna (L0c4b2b)",
        hecho: super::gspsecuencia::leido,
        dar: super::gspsecuencia::leer,
        pide: Some(b"vaciar"),
        consejo: b"`gpu`: la fila `secuen` dice cuantas ordenes y de que tipo, y cada `orden` que registro toca y con que; crudo en datos/gspsec.bin -- lo siguiente es `init`",
        repinta: false,
    },
    // ** `init` VUELVE a save mode (24-09): salio el 09:54 porque tras
    // GSP_INIT_DONE la pantalla se quedaba quieta, y era BAR1. Desde L0c4b3a
    // (VISTO 10:13) `correr` se la devuelve al GOP, y aqui se repinta todo.
    Paso {
        nombre: b"init",
        que: b"CORRER el secuenciador hasta GSP_INIT_DONE: el GSP-RM de la 570.144 ARRANCA, y BAR1 vuelve a la pantalla (L0c4b2c)",
        hecho: super::gspinit::listo,
        dar: super::gspinit::correr,
        pide: Some(b"secuenciador"),
        consejo: b"`gpu`: `corrio` dice las ordenes CORRIDAS, `listo` GSP_INIT_DONE y `bar1` que se le DEVOLVIO la del GOP; el panel, `gsp LISTO`",
        repinta: true,
    },
    Paso {
        nombre: b"estatica",
        que: b"LA PRIMERA RPC: GET_GSP_STATIC_INFO, lo que el GSP-RM dice de la 3060 (L1a)",
        hecho: super::gsprpc::contestada,
        dar: super::gsprpc::preguntar,
        pide: Some(b"init"),
        consejo: b"`gpu`: `rpc` CONTESTADA con rpc_result 0, `nombre` tu RTX 3060, `memoria` 12288 MiB de GDDR6 y 192 bits",
        repinta: false,
    },
    Paso {
        nombre: b"objetos",
        que: b"NUESTROS objetos en el RM por GSP_RM_ALLOC: cliente, dispositivo y subdispositivo (L1b)",
        hecho: super::gspobjeto::listos,
        dar: super::gspobjeto::paso,
        pide: Some(b"estatica"),
        consejo: b"`gpu`: las filas `obj cli`, `obj disp` y `obj sub` en NV_OK -- lo siguiente, su primera orden de control",
        repinta: false,
    },
    Paso {
        nombre: b"salud",
        que: b"la primera ORDEN DE CONTROL sobre nuestro subdispositivo: el P-state (GSP_RM_CONTROL); y la temperatura y el PCIe (L1b)",
        hecho: super::gspsalud::hecho,
        dar: super::gspsalud::preguntar,
        pide: Some(b"objetos"),
        consejo: b"`gpu`: `pstate` dice P0..P15 con NV_OK, `temp` los grados del sensor y `pcie` el enlace; el panel, lo mismo -- lo siguiente es `espacio`",
        repinta: false,
    },
    Paso {
        nombre: b"espacio",
        que: b"el ESPACIO DE DIRECCIONES de la GPU, nuestro: FERMI_VASPACE_A 'de fuera' (sus tablas, nuestras) (L1c1)",
        hecho: super::gspobjeto::espacio_listo,
        dar: super::gspobjeto::paso_espacio,
        pide: Some(b"objetos"),
        consejo: b"`gpu`: la fila `obj esp` en NV_OK -- lo siguiente es `vram`, que la CPU escriba en la VRAM",
        repinta: false,
    },
    Paso {
        nombre: b"vram",
        que: b"LA CPU ESCRIBE EN LA VRAM por la ventana PRAMIN: una pagina en 64 MiB, guardada y devuelta (L1c2)",
        hecho: super::gspvram::hecha,
        dar: super::gspvram::probar,
        pide: Some(b"estatica"),
        consejo: b"`gpu`: la fila `vram` dice 1024 de 1024 y devueltas 1024 con la ventana devuelta -- lo siguiente es `directorio`",
        repinta: false,
    },
    Paso {
        nombre: b"directorio",
        que: b"la RAIZ de nuestro espacio de direcciones: una pagina de VRAM a cero y SET_PAGE_DIRECTORY (L1c3)",
        hecho: super::gspvram::directorio_puesto,
        dar: super::gspvram::poner_directorio,
        pide: Some(b"espacio"),
        consejo: b"`gpu`: la fila `pd` dice la raiz PD3 en 0x004100000 con NV_OK, y `raiz` sus 4 entradas -- lo siguiente es `tramo`",
        repinta: false,
    },
    Paso {
        nombre: b"tramo",
        que: b"MAPEAR 16 paginas de VRAM propia en nuestro espacio: la GPU las ve en la VA de 8 GiB (L1d1)",
        hecho: super::gspvram::tramo_puesto,
        dar: super::gspvram::mapear_tramo,
        pide: Some(b"directorio"),
        consejo: b"`gpu`: la fila `tramo` dice 20 de 20 entradas releidas -- lo siguiente es `motores`",
        repinta: false,
    },
    Paso {
        nombre: b"motores",
        que: b"QUE MOTORES tiene la 3060 y cuanto mide el bufer de metodos de un canal: dos preguntas, sin cambiar nada (L1d2a)",
        hecho: super::gspmotores::hecho,
        dar: super::gspmotores::preguntar,
        pide: Some(b"objetos"),
        consejo: b"`gpu`: la fila `motores` dice GR0, varios COPY y COPY2 como el de copia del canal; `metodos`, 20480 B -- lo siguiente es `canal`",
        repinta: false,
    },
    Paso {
        nombre: b"canal",
        que: b"PEDIR EL CANAL GPFIFO: su instancia, USERD y GPFIFO en el tramo, y su bufer de metodos prestado a la 3060 para escribir (L1d2b)",
        hecho: super::gspcanal::pedido,
        dar: super::gspcanal::pedir,
        pide: Some(b"motores"),
        consejo: b"`gpu`: la fila `canal` dice NV_OK y `memoria` donde vive; `iommu`: `domain` 5 paginas mas y sin eventos nuevos -- lo siguiente es `encender`",
        repinta: false,
    },
    Paso {
        nombre: b"encender",
        que: b"ENCENDER EL CANAL: BIND a COPY2 y GPFIFO_SCHEDULE, en ese orden (L1d2c)",
        hecho: super::gspcanal::encendido,
        dar: super::gspcanal::encender,
        pide: Some(b"canal"),
        consejo: b"`gpu`: las filas `atado` (COPY2) y `en lista` dicen NV_OK -- lo siguiente es `ficha`",
        repinta: false,
    },
    Paso {
        nombre: b"ficha",
        que: b"LA FICHA DEL TIMBRE: GET_WORK_SUBMIT_TOKEN sobre el canal, una pregunta (L1d2d)",
        hecho: super::gspcanal::ficha_leida,
        dar: super::gspcanal::preguntar_ficha,
        pide: Some(b"encender"),
        consejo: b"`gpu`: la fila `ficha` dice el numero que se escribira en el timbre -- lo siguiente es `copiador`",
        repinta: false,
    },
    Paso {
        nombre: b"copiador",
        que: b"EL COPIADOR: AMPERE_DMA_COPY_B sobre COPY2, colgado del canal (L1d3)",
        hecho: super::gspcanal::copiador_listo,
        dar: super::gspcanal::pedir_copiador,
        pide: Some(b"ficha"),
        consejo: b"`gpu`: la fila `copiador` dice NV_OK -- lo siguiente es `copia`",
        repinta: false,
    },
    Paso {
        nombre: b"copia",
        que: b"LA PRIMERA COPIA: GP_PUT, el timbre, y la 3060 copia 4 KiB de VRAM a VRAM por su canal y paga el semaforo (L1d2d y L1d3)",
        hecho: super::gspcanal::copia_hecha,
        dar: super::gspcanal::copiar,
        pide: Some(b"copiador"),
        consejo: b"`gpu`: la fila `copia` dice LA 3060 COPIO 1024 de 1024 y semaforo PAGADO; `iommu` sin eventos nuevos -- lo siguiente es `gr`, el motor grafico",
        repinta: false,
    },
    Paso {
        nombre: b"gr",
        que: b"QUE PIDE EL MOTOR GRAFICO: los buferes de su contexto de oro, una pregunta al cliente interno del RM (M5 G0)",
        hecho: super::gspgr::hecho,
        dar: super::gspgr::preguntar,
        pide: Some(b"estatica"),
        consejo: b"`gpu`: la fila `gr` dice 9 buferes y cuanto ocupan, y cada `gr bufer` su medida -- lo siguiente es `canalgr`",
        repinta: false,
    },
    Paso {
        nombre: b"canalgr",
        que: b"EL CANAL DE GR0: pedido con la receta del de copia, chid 2 en la lista 0, y su bufer de metodos (M5 G1)",
        hecho: super::gspcanalgr::pedido,
        dar: super::gspcanalgr::pedir,
        pide: Some(b"canal"),
        consejo: b"`gpu`: la fila `canal gr` dice NV_OK; `iommu`: `domain` 5 paginas mas -- lo siguiente es `encendergr`",
        repinta: false,
    },
    Paso {
        nombre: b"encendergr",
        que: b"ENCENDER EL CANAL DE GR0: BIND a GR0 y GPFIFO_SCHEDULE (M5 G1)",
        hecho: super::gspcanalgr::encendido,
        dar: super::gspcanalgr::encender,
        pide: Some(b"canalgr"),
        consejo: b"`gpu`: `atado gr` y `en lista gr` dicen NV_OK -- lo siguiente es `grmem`, los buferes de `gr` en tu VRAM",
        repinta: false,
    },
    Paso {
        nombre: b"grmem",
        que: b"LOS BUFERES DE GR EN TU VRAM: los que llena el RM a cero, todo mapeado en la VA de 12 GiB y la MMU invalidada (M5 G2)",
        hecho: super::gspgr::mapeado,
        dar: super::gspgr::mapear,
        pide: Some(b"gr"),
        consejo: b"`gpu`: `gr memoria` dice todas las entradas releidas y `gr donde` cada bufer; `iommu` sin eventos nuevos -- lo siguiente es `promover`",
        repinta: false,
    },
    Paso {
        nombre: b"promover",
        que: b"DARLE AL RM LOS BUFERES DE GR: PROMOTE_CTX con los 9 de G2, cada direccion dentro de su region (M5 G3)",
        hecho: super::gspgr::promovido,
        dar: super::gspgr::promover,
        pide: Some(b"grmem"),
        consejo: b"`gpu`: la fila `gr oro` dice PROMOTE_CTX NV_OK -- lo siguiente es `oro`, AMPERE_B",
        repinta: false,
    },
    Paso {
        nombre: b"oro",
        que: b"EL CONTEXTO DE ORO: AMPERE_B (0xC797) en el canal de GR0; el RM lo corre con nuestros buferes (M5 G4)",
        hecho: super::gspgr::de_oro,
        dar: super::gspgr::tresde,
        pide: Some(b"promover"),
        consejo: b"`gpu`: la segunda `gr oro` dice AMPERE_B NV_OK; `iommu` sin eventos nuevos -- lo siguiente es `computo`",
        repinta: false,
    },
    Paso {
        nombre: b"computo",
        que: b"LA CLASE DE COMPUTO: AMPERE_COMPUTE_B (0xC7C0) en el canal de GR0, sin parametros (M5d S1)",
        hecho: super::gspcomputo::pedido,
        dar: super::gspcomputo::pedir,
        pide: Some(b"oro"),
        consejo: b"`gpu`: la fila `computo` dice NV_OK -- lo siguiente es `fichagr`",
        repinta: false,
    },
    Paso {
        nombre: b"fichagr",
        que: b"LA FICHA DEL CANAL DE GR0 y la lista de GR0 en la tabla de aparatos (M5d S2)",
        hecho: super::gspcomputo::ficha_leida,
        dar: super::gspcomputo::ficha,
        pide: Some(b"canalgr"),
        consejo: b"`gpu`: la fila `ficha gr` dice la ficha y la lista de GR0 -- lo siguiente es `trabajogr`",
        repinta: false,
    },
    Paso {
        nombre: b"trabajogr",
        que: b"EL PRIMER TRABAJO DEL MOTOR GRAFICO: SET_OBJECT del computo y un semaforo de INFORME, que solo paga el GR (M5d S3)",
        hecho: super::gspcomputo::trabajado,
        dar: super::gspcomputo::trabajar,
        pide: Some(b"computo"),
        consejo: b"`gpu`: `gr trabajo` dice PAGADO; `iommu` sin eventos nuevos -- lo siguiente es `sombreo`, el primer sombreador",
        repinta: false,
    },
    Paso {
        nombre: b"sombreo",
        que: b"EL PRIMER SOMBREADOR: 32 hilos de SASS de SM86 (de ptxas) lanzados con un QMD; cada uno escribe su palabra (M5d S4..S6)",
        hecho: super::gspcomputo::sombreado,
        dar: super::gspcomputo::sombrear,
        pide: Some(b"trabajogr"),
        consejo: b"`gpu`: `sombreo` dice 32 de 32 y los dos semaforos PAGADOS; `iommu` sin eventos nuevos -- lo siguiente es `lienzo`",
        repinta: false,
    },
    Paso {
        nombre: b"lienzo",
        que: b"LA 3060 PINTA EN LA RAM DEL PC: 128 x 128 hilos, un degradado en 64 KiB prestados por la IOMMU (M5d L)",
        hecho: super::gspcomputo::pintado,
        dar: super::gspcomputo::pintar,
        pide: Some(b"sombreo"),
        consejo: b"`gpu`: `lienzo` dice 16384 de 16384; `iommu` sin eventos nuevos -- teclea `gpu lienzo` y VELO; lo siguiente es `blur`",
        repinta: false,
    },
    Paso {
        nombre: b"blur",
        que: b"EL BLUR: la media de 7 x 7 de cada pixel del lienzo, por la 3060, igual pixel a pixel que la cuenta de la CPU (M5d B)",
        hecho: super::gspcomputo::desenfocado,
        dar: super::gspcomputo::desenfocar,
        pide: Some(b"lienzo"),
        consejo: b"`gpu`: `blur` dice 16384 de 16384; teclea `gpu blur` y mira un trozo de tu pantalla DESENFOCADO por la 3060 -- lo siguiente es `fractal`",
        repinta: false,
    },
    Paso {
        nombre: b"fractal",
        que: b"LA FUERZA DE LA 3060: Mandelbrot de 512 x 512 (262144 hilos), cronometrado contra la CPU y comparado bit a bit (M5d F)",
        hecho: super::gspcomputo::fractal_hecho,
        dar: super::gspcomputo::calcular_fractal,
        pide: Some(b"blur"),
        consejo: b"`gpu`: `fractal` dice 262144 de 262144 y cuantas veces mas rapida; teclea `gpu fractal` para el panel a pantalla completa -- lo siguiente es `triangulo`",
        repinta: false,
    },
    Paso {
        nombre: b"triangulo",
        que: b"EL PRIMER TRIANGULO DE LA 3060: las tres funciones de arista en 262144 hilos, sus colores mezclados, comparado bit a bit (M5d T0)",
        hecho: super::gspcomputo::triangulo_hecho,
        dar: super::gspcomputo::dibujar_triangulo,
        pide: Some(b"fractal"),
        consejo: b"`gpu`: `triangulo` dice 262144 de 262144; teclea `gpu triangulo` y VELO a pantalla completa -- lo siguiente es `limpio3d`",
        repinta: false,
    },
    Paso {
        nombre: b"limpio3d",
        que: b"LA CLASE 3D ESCRIBE PIXELES: AMPERE_B limpia un destino de 512 x 512 con su ROP, sin programas (M5 T1a)",
        hecho: super::gspcomputo::limpio_3d,
        dar: super::gspcomputo::limpiar_3d,
        pide: Some(b"triangulo"),
        consejo: b"`gpu`: la fila `3d` dice 262144 de 262144 y el semaforo PAGADO; teclea `gpu 3d` -- lo siguiente es `escena`",
        repinta: false,
    },
    Paso {
        nombre: b"escena",
        que: b"LA ESCENA 3D CON LUZ: esfera iluminada, suelo con sombra y cielo, dibujada por la 3060 y comparada bit a bit (M5d E)",
        hecho: super::gspcomputo::escena_hecha,
        dar: super::gspcomputo::dibujar_escena,
        pide: Some(b"triangulo"),
        consejo: b"`gpu`: `escena` dice 262144 de 262144; teclea `gpu escena` y VELA a pantalla completa -- lo siguiente es `giro`",
        repinta: false,
    },
    Paso {
        nombre: b"giro",
        que: b"MOVIMIENTO: una esfera que gira y bota, 32 fotogramas dibujados por la 3060 (computo) y comprobados uno a uno por la CPU (M5d G)",
        hecho: super::gspcomputo::giro_hecho,
        dar: super::gspcomputo::dibujar_giro,
        pide: Some(b"escena"),
        consejo: b"`gpu`: `giro` dice 32 de 32; teclea `gpu giro` y VELA moverse a pantalla completa -- lo siguiente es `raster`",
        repinta: false,
    },
    Paso {
        nombre: b"raster",
        que: b"EL TRIANGULO POR EL RASTERIZADOR: programa de vertice, rasterizador, programa de pixel y ROP de la 3060, con juez (M5 T1b + T1c)",
        hecho: super::gspcomputo::raster_hecho,
        dar: super::gspcomputo::dibujar_raster,
        pide: Some(b"limpio3d"),
        consejo: b"`gpu`: `raster` dice 262144 de 262144 y el semaforo PAGADO; teclea `gpu raster` y VELO -- lo siguiente es `color`",
        repinta: false,
    },
    Paso {
        nombre: b"color",
        que: b"TRES COLORES MEZCLADOS POR EL RASTERIZADOR: cada vertice con su color, la 3060 los mezcla en cada pixel con IPA (M5 T2a)",
        hecho: super::gspcomputo::color3d_hecho,
        dar: super::gspcomputo::dibujar_color3d,
        pide: Some(b"raster"),
        consejo: b"`gpu`: `color` dice 262144 de 262144; teclea `gpu color` y VELO -- lo siguiente es T2b, la profundidad (dos triangulos que se cruzan)",
        repinta: false,
    },
    Paso {
        nombre: b"pantalla",
        que: b"LA 3060 TOMA LA PANTALLA: 8 fotogramas a la resolucion del monitor, escritos por la 3060 directamente en el framebuffer del GOP; la CPU comprueba 1024 pixeles de cada uno (M5d P)",
        hecho: super::gspcomputo::pantalla_hecha,
        dar: super::gspcomputo::dibujar_pantalla,
        pide: Some(b"escena"),
        consejo: b"`gpu`: la fila `pantalla` dice 8 de 8 y los fps; teclea `gpu pantalla` y VELA tomar el monitor entero",
        repinta: true,
    },
    // ** EL ULTIMO, SIEMPRE (L0c5, 25-09): tras el, el GSP-RM ya no contesta.
    // Pide solo `despertar`: aunque algo de en medio falle, se apaga igual,
    // y el siguiente arranque (tambien con el boton de reset) sale limpio.
    Paso {
        nombre: b"apagado",
        que: b"EL GSP APAGADO EN ORDEN: la despedida al GSP-RM, FWSEC-SB y el booter de descarga baja la WPR2 -- el booter del arranque siguiente no sale con 0x15 (L0c5)",
        hecho: super::gspapagar::hecho,
        dar: super::gspapagar::apagar,
        pide: Some(b"despertar"),
        consejo: b"`gpu`: la fila `apagado` dice EL GSP APAGADO EN ORDEN y `wpr2-abajo`; ya puedes reiniciar -- para volver a usar la 3060, arranca otra vez",
        repinta: false,
    },
];
