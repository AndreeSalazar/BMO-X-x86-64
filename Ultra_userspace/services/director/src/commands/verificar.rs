//! **`save mode`: LA VERIFICACION TOTAL** -- todos los pasos arriesgados de la
//! GPU, en orden, con un `save` antes de cada uno, y al final notas y consejos.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea
//!
//! # Por que existe (2026-09-24)
//!
//! Peticion del propietario, para el dia que BMO-X ejecute la RTX 3060 por
//! primera vez: *"verificacion total"*. Cada paso nuevo hacia la GPU es una
//! orden que escribe en el hardware, y cada una puede tumbar la maquina. Hacerlos
//! a mano, uno por uno, es acordarse del orden, del `save` de antes y de que
//! mirar despues. Esto lo hace la maquina:
//!
//! ```text
//!    save mode              TODOS los pasos, en orden
//!    save mode -gpu         todos menos ese (y cualquier otro con `-nombre`)
//! ```
//!
//! Antes de CADA paso, el informe maestro se guarda -- siempre, este en
//! `save auto` o en `save manual`: esta orden ES un save. Un paso que ya esta
//! hecho no se repite; uno que pide otro que no esta, no se intenta, y se dice.
//! Si un `save` no se puede escribir, se para todo: sin red no se salta.
//!
//! Los pasos viven en [`PASOS`], en el orden en que hay que darlos. E2 (el
//! VBLANK por interrupcion) es la tercera fila desde el 2026-09-24.
//!
//! Y queda ARMADO en `datos/modo.txt`: se repite solo en cada arranque, sin el
//! paso que tumbo la maquina si lo hubo (ver `EL MODO ARMADO`, mas abajo). El
//! CONSEJERO dice lo siguiente recomendado cada vez que Ctrl+Alt abre la caja.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};
use crate::DEFAULT_DUMP;

/// Un paso de la verificacion.
struct Paso {
    /// El nombre con el que se quita: `-iommu`, `-gpu`.
    nombre: &'static [u8],
    /// Que hace, en una linea.
    que: &'static [u8],
    /// Ya esta hecho (no se repite).
    hecho: fn() -> bool,
    /// Darlo. `Ok` con el valor del kernel, o el motivo del NO.
    dar: fn() -> Result<u64, u32>,
    /// El paso que tiene que estar hecho antes, si lo hay.
    pide: Option<&'static [u8]>,
    /// Lo que mirar despues si salio bien.
    consejo: &'static [u8],
    /// Tras darlo, repintar el escritorio entero: `init` cambia BAR1 y lo
    /// que habia en pantalla era de antes (L0c4b3a).
    repinta: bool,
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
const PASOS: &[Paso] = &[
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
        consejo: b"`gpu`: `blur` dice 16384 de 16384; teclea `gpu blur` y mira un trozo de tu pantalla DESENFOCADO por la 3060 -- lo siguiente es el triangulo",
        repinta: false,
    },
];

/// Cuantos pasos caben. Eran 8 y `vbios` hizo el octavo (24-09): con L0 en
/// camino, se deja sitio -- y la prueba de abajo dice NO si se pasa. Eran 16
/// y `sistema` hizo el decimosexto (L0c4b2a, 24-09): L0c4b2b y L0c4b2c vienen.
/// Y eran 24 y L1c los llevo a 23 (24-09): con L1d y M5 detras, 32. Y `gr`
/// (M5 G0, 24-09) hizo el trigesimo segundo: con G1..G4 y el triangulo, 48.
const MAX_PASOS: usize = 48;
const _: () = assert!(PASOS.len() <= MAX_PASOS, "save mode: mas pasos que MAX_PASOS");

/// Que salio de cada paso.
#[derive(Clone, Copy, PartialEq)]
enum Salio {
    Quitado,
    /// Un save de antes no se pudo escribir: de ahi en adelante, nada.
    Parado,
    YaEstaba,
    FaltaOtro,
    Bien,
    No(u32),
}

// == EL MODO ARMADO: SOBREVIVE A UN REINICIO Y A UN FALLO DEL KERNEL ==========
//
// Peticion del propietario (24-09): *"save mode es para automatizar en caso que
// la PC se reinicie o kernel fault"*. Asi que `save mode` no solo corre: queda
// ARMADO en `datos/modo.txt`, y en cada arranque el escritorio lo repite solo.
//
// ** Y con MEMORIA, porque repetir a ciegas es un bucle de caidas: si un paso
// tumba la maquina, el arranque siguiente lo volveria a dar, y el otro, y el
// otro. Antes de CADA paso se escribe `en curso: <paso>` en el fichero (y el
// kernel hace FLUSH del disco antes de tocar la IOMMU, asi que llega); al
// acabar, se borra. Si un arranque encuentra un `en curso`, ese paso TUMBO la
// maquina la vez anterior: se QUITA solo (`-paso`), se apunta `tumbo: paso` y
// el consejero lo dice. Se repite todo menos lo que tumbo.
//
// ```text
//    save mode -gpu        lo corre YA y lo deja ARMADO con esos `-`
//    save mode off         lo desarma: al arrancar ya no se repite
// ```

/// Donde vive el modo armado. 8.3, como todo en FAT32.
const MODO: &[u8] = b"datos/modo.txt";

/// El modo leido del disco.
struct Modo {
    /// Los argumentos tal cual (`-gpu ...`), sin el `save mode`.
    args: [u8; 64],
    n: usize,
    /// El paso que estaba EN CURSO cuando se apago la maquina.
    en_curso: Option<usize>,
    /// El paso que ya se sabe que tumbo la maquina.
    tumbo: Option<usize>,
}

impl Modo {
    fn args(&self) -> &[u8] {
        &self.args[..self.n]
    }
}

fn paso_por_nombre(nombre: &[u8]) -> Option<usize> {
    PASOS.iter().position(|p| p.nombre == nombre)
}

/// **Lee `datos/modo.txt`.** `None` = no esta armado.
fn leer_modo() -> Option<Modo> {
    let a = bmo::Archivo::leer_de(MODO).ok()?;
    let mut buf = [0u8; 256];
    let n = a.read(&mut buf);
    a.close();
    let mut m = Modo { args: [0; 64], n: 0, en_curso: None, tumbo: None };
    let mut armado = false;
    for linea in buf[..n].split(|&b| b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)) {
        if let Some(r) = linea.strip_prefix(b"save mode") {
            let r = r.strip_prefix(b" ").unwrap_or(r);
            let k = r.len().min(m.args.len());
            m.args[..k].copy_from_slice(&r[..k]);
            m.n = k;
            armado = true;
        } else if let Some(r) = linea.strip_prefix(b"en curso: ") {
            m.en_curso = paso_por_nombre(r);
        } else if let Some(r) = linea.strip_prefix(b"tumbo: ") {
            m.tumbo = paso_por_nombre(r);
        }
    }
    armado.then_some(m)
}

/// **Escribe el modo**: armado con `args`, el paso en curso y el que tumbo.
/// `false` si no se pudo escribir.
fn escribir_modo(args: &[u8], en_curso: Option<usize>, tumbo: Option<usize>) -> bool {
    let Ok(a) = bmo::Archivo::create(MODO) else { return false };
    a.write(b"save mode");
    if !args.is_empty() {
        a.write(b" ");
        a.write(args);
    }
    a.write(b"\n");
    if let Some(i) = en_curso {
        a.write(b"en curso: ");
        a.write(PASOS[i].nombre);
        a.write(b"\n");
    }
    if let Some(i) = tumbo {
        a.write(b"tumbo: ");
        a.write(PASOS[i].nombre);
        a.write(b"\n");
    }
    a.close()
}

/// **Desarma**: el fichero queda, pero sin `save mode` dentro.
fn desarmar() -> bool {
    let Ok(a) = bmo::Archivo::create(MODO) else { return false };
    a.write(b"apagado\n");
    a.close()
}

/// Los `-paso` de unos argumentos. `Err(t)` con el que no es un paso.
fn quitados_de(args: &[u8]) -> Result<[bool; MAX_PASOS], &[u8]> {
    let mut quitados = [false; MAX_PASOS];
    for t in args.split(|&b| b == b' ').filter(|t| !t.is_empty()) {
        match t.strip_prefix(b"-").and_then(paso_por_nombre) {
            Some(i) => quitados[i] = true,
            None => return Err(t),
        }
    }
    Ok(quitados)
}

/// **`save mode [-nombre ...]` / `save mode off`**. `None` si `arg` no es
/// `mode ...`.
pub(crate) fn save_mode(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> Option<After> {
    let resto = if arg == b"mode" {
        &b""[..]
    } else if let Some(r) = arg.strip_prefix(b"mode ") {
        r
    } else {
        return None;
    };
    if resto == b"off" || resto == b"apagar" {
        let ok = desarmar();
        let g = &mut dsk.out.grid;
        g.with_ink(if ok { INK_GOOD } else { INK_ERR });
        g.text(if ok {
            b"  save mode DESARMADO: al arrancar ya no se repite\n" as &[u8]
        } else {
            b"  save mode: no se pudo escribir datos/modo.txt\n"
        });
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return Some(After::Settle);
    }
    // Un nombre que no es un paso se dice y no se corre nada: una
    // verificacion con una errata no es una verificacion.
    let quitados = match quitados_de(resto) {
        Ok(q) => q,
        Err(t) => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            g.text(b"  save mode: `");
            g.text(t);
            g.text(b"` no es un paso. Los pasos se quitan con `-`:");
            for p in PASOS {
                g.text(b" -");
                g.text(p.nombre);
            }
            g.text(b"   (y `save mode off` lo desarma)\n");
            g.with_ink(INK_PLAIN);
            dsk.field.n = 0;
            return Some(After::Settle);
        }
    };
    let armado = escribir_modo(resto, None, None);
    {
        let g = &mut dsk.out.grid;
        g.with_ink(if armado { INK_GOOD } else { INK_ERR });
        g.text(if armado {
            b"  save mode ARMADO en datos/modo.txt: al arrancar se repite solo\n" as &[u8]
        } else {
            b"  save mode: NO se pudo armar (datos/modo.txt): corre solo esta vez\n"
        });
        g.with_ink(INK_PLAIN);
    }
    correr(dsk, p, &quitados, resto, None);
    dsk.field.n = 0;
    Some(After::Settle)
}

/// **Al arrancar**: si el modo esta armado, se repite -- sin el paso que
/// tumbo la maquina, si lo hubo. Lo llama el arranque del escritorio.
pub(crate) fn al_arrancar(dsk: &mut Desktop, p: &bmo::Pantalla) {
    let Some(m) = leer_modo() else {
        consejero(&mut dsk.out.grid);
        return;
    };
    let mut args = [0u8; 64];
    let mut n = m.n;
    args[..n].copy_from_slice(m.args());
    // El paso en curso cuando se cayo: se QUITA, y se apunta que tumbo.
    let tumbo = m.en_curso.or(m.tumbo);
    if let Some(i) = m.en_curso {
        let nombre = PASOS[i].nombre;
        if n + 2 + nombre.len() <= args.len() {
            args[n] = b' ';
            args[n + 1] = b'-';
            args[n + 2..n + 2 + nombre.len()].copy_from_slice(nombre);
            n += 2 + nombre.len();
        }
        escribir_modo(&args[..n], None, tumbo);
    }
    let Ok(quitados) = quitados_de(&args[..n]) else {
        consejero(&mut dsk.out.grid);
        return;
    };
    {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_GOOD);
        g.text(b"  save mode ARMADO: se repite al arrancar (`save mode off` lo desarma)\n");
        if let Some(i) = m.en_curso {
            g.with_ink(INK_ERR);
            g.text(b"  el paso `");
            g.text(PASOS[i].nombre);
            g.text(b"` estaba EN CURSO cuando se apago la maquina: TUMBO, y queda quitado\n");
        }
        g.with_ink(INK_PLAIN);
    }
    let copia = args;
    correr(dsk, p, &quitados, &copia[..n], tumbo);
}

/// **Los pasos, en orden**, con un save antes de cada uno y la marca `en
/// curso` alrededor. Lo comparten la orden y el arranque.
fn correr(dsk: &mut Desktop, p: &bmo::Pantalla, quitados: &[bool; MAX_PASOS], args: &[u8], tumbo: Option<usize>) {
    {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_GOOD);
        g.text(b"  VERIFICACION TOTAL: ");
        g.dec(PASOS.len() as u64);
        g.text(b" paso(s) en orden, con un save antes de cada uno\n");
        g.with_ink(INK_PLAIN);
    }
    let armado = leer_modo().is_some();
    // ** EL SAVE DE EMERGENCIA, ANTES DE TODO (24-09): aunque todos los pasos
    // esten hechos y no se arriesgue nada, lo que la maquina es AHORA queda en
    // el disco (datos/) desde el primer instante. Los de antes de cada paso
    // vienen despues.
    {
        let ok = super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo()).is_ok();
        let g = &mut dsk.out.grid;
        g.with_ink(if ok { INK_GOOD } else { INK_ERR });
        g.text(if ok {
            b"  save de emergencia ESCRITO antes de todo: el informe y los datos ya estan en datos/\n" as &[u8]
        } else {
            b"  el save de emergencia NO se pudo escribir: mira `disco` (los pasos lo intentan otra vez)\n"
        });
        g.with_ink(INK_PLAIN);
    }
    let mut salio = [Salio::Quitado; MAX_PASOS];
    let mut tiempo = [0u64; MAX_PASOS];
    let mut intentos = [0u8; MAX_PASOS];
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let mut parado = false;
    for (i, paso) in PASOS.iter().enumerate() {
        salio[i] = if parado {
            Salio::Parado
        } else if quitados[i] {
            Salio::Quitado
        } else if (paso.hecho)() {
            Salio::YaEstaba
        } else if paso.pide.map_or(false, |otro| !PASOS.iter().any(|p| p.nombre == otro && (p.hecho)())) {
            Salio::FaltaOtro
        } else {
            // El save de antes: si no se puede, se para TODO.
            if super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo()).is_err() {
                let g = &mut dsk.out.grid;
                g.with_ink(INK_ERR);
                g.text(b"  el save antes de `");
                g.text(paso.nombre);
                g.text(b"` no se pudo escribir: la verificacion se PARA aqui\n");
                g.with_ink(INK_PLAIN);
                parado = true;
                salio[i] = Salio::Parado;
                fila(dsk, paso, Salio::Parado, 0, 0);
                continue;
            }
            // La marca: si la maquina cae AHORA, el arranque siguiente lo sabe.
            if armado {
                escribir_modo(args, Some(i), tumbo);
            }
            // Por donde va, en la linea de estado: un paso de varios segundos
            // sin decir cual es parece una maquina colgada.
            crate::scene::sugerir::pista(p, &dsk.run_box, b"save mode", paso.nombre);
            let desde = bmo::ciclos();
            let mut r = (paso.dar)();
            intentos[i] = 1;
            // ** EL REINTENTO (24-09): un paso que solo PREGUNTA y no sale se
            // da otra vez, una sola: una respuesta que tarda o un mensaje
            // del GSP por medio no deberian tumbar todo lo que va detras.
            if r.is_err() && reintentable(paso.nombre) {
                r = (paso.dar)();
                intentos[i] = 2;
            }
            tiempo[i] = (bmo::ciclos() - desde) * 1_000_000 / hz;
            if armado {
                escribir_modo(args, None, tumbo);
            }
            if paso.repinta && r.is_ok() {
                crate::repintar_escritorio(p, dsk, "save mode");
            }
            match r {
                Ok(_) => Salio::Bien,
                Err(m) => Salio::No(m),
            }
        };
        fila(dsk, paso, salio[i], tiempo[i], intentos[i]);
    }
    // Y el save de despues: lo que quedo, tambien en el disco.
    let _ = super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo());
    let escrito = escribir_pasos(&salio, &tiempo, &intentos, tumbo);
    resumen(dsk, &salio, tumbo, escrito);
    notas(dsk, &salio, armado);
    super::iommu::report_iommu(&mut dsk.out.grid);
    consejero(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "verificacion total", INK_DIM);
}

/// Los pasos que solo PREGUNTAN al GSP-RM (o leen): darlos dos veces no
/// cambia nada en la 3060, asi que un fallo se reintenta una vez.
const REINTENTABLES: &[&[u8]] = &[b"estatica", b"objetos", b"salud", b"espacio", b"motores", b"ficha", b"gr", b"fichagr"];

fn reintentable(nombre: &[u8]) -> bool {
    REINTENTABLES.contains(&nombre)
}

fn fila(dsk: &mut Desktop, paso: &Paso, s: Salio, us: u64, intentos: u8) {
    let g = &mut dsk.out.grid;
    g.text(b"    ");
    g.text(paso.nombre);
    g.text(b"  ");
    match s {
        Salio::Quitado => {
            g.with_ink(INK_ECHO);
            g.text(b"QUITADO (-");
            g.text(paso.nombre);
            g.text(b")");
        }
        Salio::YaEstaba => {
            g.with_ink(INK_GOOD);
            g.text(b"ya estaba hecho");
        }
        Salio::Parado => {
            g.with_ink(INK_ERR);
            g.text(b"PARADO: el save de antes no se pudo escribir");
        }
        Salio::FaltaOtro => {
            g.with_ink(INK_ERR);
            g.text(b"NO SE INTENTA: pide `");
            g.text(paso.pide.unwrap_or(b""));
            g.text(b"` antes");
        }
        Salio::Bien => {
            g.with_ink(INK_GOOD);
            g.text(b"HECHO");
        }
        Salio::No(_) => {
            g.with_ink(INK_ERR);
            g.text(b"NO");
        }
    }
    if matches!(s, Salio::Bien | Salio::No(_)) {
        g.with_ink(INK_ECHO);
        g.text(b" en ");
        if us >= 10_000 {
            g.dec(us / 1000);
            g.text(b" ms");
        } else {
            g.dec(us);
            g.text(b" us");
        }
        if intentos > 1 {
            g.text(b" (al 2o intento)");
        }
    }
    match s {
        Salio::Quitado | Salio::YaEstaba | Salio::Parado | Salio::FaltaOtro | Salio::Bien => {}
        Salio::No(m) => {
            g.with_ink(INK_ERR);
            g.text(b": ");
            g.text(super::iommu::motivo(m));
        }
    }
    g.with_ink(INK_ECHO);
    g.text(b"   -- ");
    g.text(paso.que);
    g.with_ink(INK_PLAIN);
    g.byte(b'\n');
}

/// **EL RESUMEN** (24-09): una linea con la cuenta, y el primer paso que no
/// salio con su motivo -- lo primero que hay que leer.
fn resumen(dsk: &mut Desktop, salio: &[Salio; MAX_PASOS], tumbo: Option<usize>, escrito: bool) {
    let n = PASOS.len();
    let cuenta = |f: fn(&Salio) -> bool| salio[..n].iter().filter(|s| f(s)).count() as u64;
    let bien = cuenta(|s| matches!(s, Salio::Bien | Salio::YaEstaba));
    let g = &mut dsk.out.grid;
    g.separar();
    g.with_ink(if bien as usize == n { INK_GOOD } else { INK_PLAIN });
    g.text(b"  RESUMEN  ");
    g.dec(bien);
    g.text(b" de ");
    g.dec(n as u64);
    g.text(b" pasos bien (");
    g.dec(cuenta(|s| matches!(s, Salio::Bien)));
    g.text(b" hechos ahora, ");
    g.dec(cuenta(|s| matches!(s, Salio::YaEstaba)));
    g.text(b" ya estaban)");
    let quitados = cuenta(|s| matches!(s, Salio::Quitado));
    if quitados > 0 {
        g.text(b", ");
        g.dec(quitados);
        g.text(b" quitados");
    }
    g.with_ink(INK_PLAIN);
    g.byte(b'\n');
    if let Some(i) = salio[..n].iter().position(|s| matches!(s, Salio::No(_) | Salio::Parado)) {
        g.with_ink(INK_ERR);
        g.text(b"           se paro en `");
        g.text(PASOS[i].nombre);
        g.text(b"`: ");
        g.text(match salio[i] {
            Salio::No(m) => super::iommu::motivo(m),
            _ => b"el save de antes no se pudo escribir",
        });
        g.with_ink(INK_PLAIN);
        g.byte(b'\n');
    }
    if let Some(i) = tumbo {
        g.with_ink(INK_ERR);
        g.text(b"           `");
        g.text(PASOS[i].nombre);
        g.text(b"` TUMBO la maquina una vez: sigue quitado\n");
        g.with_ink(INK_PLAIN);
    }
    g.with_ink(INK_ECHO);
    g.text(if escrito {
        b"           una linea por paso en datos/pasos.txt: pega ESE fichero, no el informe entero\n" as &[u8]
    } else {
        b"           datos/pasos.txt no se pudo escribir\n"
    });
    g.with_ink(INK_PLAIN);
}

/// **`datos/pasos.txt`** (24-09): una linea por paso -- nombre, que salio,
/// cuanto tardo y el motivo --, para pegarlo entero en vez del informe. Lo
/// que no cabe en una linea esta en el informe maestro de al lado.
fn escribir_pasos(salio: &[Salio; MAX_PASOS], tiempo: &[u64; MAX_PASOS], intentos: &[u8; MAX_PASOS], tumbo: Option<usize>) -> bool {
    let Ok(a) = bmo::Archivo::create(b"datos/pasos.txt") else { return false };
    a.write(b"save mode -- una linea por paso (el detalle, en el informe maestro)\n");
    for (i, paso) in PASOS.iter().enumerate() {
        let mut t = [0u8; 160];
        let estado: &[u8] = match salio[i] {
            Salio::Quitado => b"QUITADO",
            Salio::Parado => b"PARADO",
            Salio::YaEstaba => b"ya estaba",
            Salio::FaltaOtro => b"no se intento",
            Salio::Bien => b"HECHO",
            Salio::No(_) => b"NO",
        };
        let mut us = [0u8; 20];
        let nu = decimal(&mut us, tiempo[i]);
        let mut n = juntar(&mut t, &[paso.nombre, b": ", estado]);
        if matches!(salio[i], Salio::Bien | Salio::No(_)) {
            n += juntar(&mut t[n..], &[b" en ", &us[..nu], b" us"]);
            if intentos[i] > 1 {
                n += juntar(&mut t[n..], &[b" (2o intento)"]);
            }
        }
        if let Salio::No(m) = salio[i] {
            n += juntar(&mut t[n..], &[b" -- ", super::iommu::motivo(m)]);
        }
        if tumbo == Some(i) {
            n += juntar(&mut t[n..], &[b" -- TUMBO la maquina una vez"]);
        }
        a.write(&t[..n]);
        a.write(b"\n");
    }
    a.close()
}

/// `v` en decimal en `t`. Devuelve cuantos bytes.
fn decimal(t: &mut [u8; 20], mut v: u64) -> usize {
    let mut k = t.len();
    loop {
        k -= 1;
        t[k] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let n = t.len() - k;
    t.copy_within(k.., 0);
    n
}

/// Escribe `partes` en `t` hasta donde quepa. Devuelve cuanto escribio.
fn juntar(t: &mut [u8], partes: &[&[u8]]) -> usize {
    let mut n = 0;
    for p in partes {
        let k = p.len().min(t.len() - n);
        t[n..n + k].copy_from_slice(&p[..k]);
        n += k;
    }
    n
}

/// **LA PISTA** (24-09): el consejero en UNA linea, para la linea de estado
/// de la caja. `(etiqueta, cuanto de t)`. Ver `desktop::paint::pista_consejero`.
pub(crate) fn pista(t: &mut [u8]) -> (&'static [u8], usize) {
    let modo = leer_modo();
    let armado: &[u8] = if modo.is_some() { b"  (armado)" } else { b"" };
    match (modo.as_ref().and_then(|m| m.tumbo), PASOS.iter().position(|p| !(p.hecho)())) {
        (Some(i), _) => (b"cuidado", juntar(t, &[b"`", PASOS[i].nombre, b"` tumbo la maquina: quitado de save mode, a mano y con save"])),
        (None, Some(i)) => (b"siguiente", juntar(t, &[b"save mode -> ", PASOS[i].nombre, b": ", PASOS[i].que, armado])),
        (None, None) => (b"verificado", juntar(t, &[b"los ", paso_n(), b" pasos; `gpu` lo muestra todo", armado])),
    }
}

/// Cuantos pasos, en texto (hasta 99).
fn paso_n() -> &'static [u8] {
    const N: [u8; 2] = [b'0' + (PASOS.len() / 10) as u8, b'0' + (PASOS.len() % 10) as u8];
    if PASOS.len() < 10 {
        &N[1..]
    } else {
        &N
    }
}

/// **EL CONSEJERO** (24-09): lo que la caja recomienda AHORA, mirando la
/// maquina y no un texto fijo. Sale al arrancar y al acabar `save mode`; al
/// invocar la caja (Ctrl+Alt) sale solo su [`pista`], en la linea de estado:
/// agregarlo a la salida cada vez la mezclaba (el propietario, 24-09). Dos
/// lineas, alineadas: lo siguiente, y el modo.
pub(crate) fn consejero(g: &mut crate::scene::output::Output) {
    let modo = leer_modo();
    g.separar();
    g.with_ink(INK_GOOD);
    g.text(b"  CONSEJERO  ");
    g.with_ink(INK_PLAIN);
    let mut t = [0u8; 160];
    let (etiqueta, n) = pista(&mut t);
    g.text(etiqueta);
    g.text(b": ");
    g.text(&t[..n]);
    g.byte(b'\n');
    g.with_ink(INK_ECHO);
    g.text(b"              ");
    match &modo {
        Some(m) => {
            g.text(b"modo ARMADO (`save mode");
            if m.n > 0 {
                g.text(b" ");
                g.text(m.args());
            }
            g.text(b"`): se repite si la maquina cae; `save mode off` lo desarma\n");
        }
        None => g.text(b"`save mode` lo corre y lo deja ARMADO; `-paso` quita uno\n"),
    }
    g.with_ink(INK_PLAIN);
}

/// **Notas y consejos**: SOLO lo que importa ahora (24-09: salian los 32
/// consejos en cada arranque, y lo que habia que mirar se perdia entre
/// ellos). Cada paso que no salio, con que hacer; y el consejo del ULTIMO
/// que salio, que es lo que hay que mirar para seguir.
fn notas(dsk: &mut Desktop, salio: &[Salio; MAX_PASOS], armado: bool) {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_GOOD);
    g.text(b"  NOTAS Y CONSEJOS\n");
    g.with_ink(INK_PLAIN);
    let ultimo_bien = (0..PASOS.len()).rev().find(|&i| matches!(salio[i], Salio::Bien | Salio::YaEstaba));
    let mut dichas = 0;
    for (i, paso) in PASOS.iter().enumerate() {
        let texto: &[u8] = match salio[i] {
            Salio::Bien | Salio::YaEstaba if Some(i) == ultimo_bien => paso.consejo,
            Salio::Bien | Salio::YaEstaba | Salio::Quitado => continue,
            Salio::Parado => b"no se dio: sin save de antes no se arriesga nada; mira `disco`",
            // Solo el primero de una cadena que no se dio: el resto es eco.
            Salio::FaltaOtro if i > 0 && matches!(salio[i - 1], Salio::FaltaOtro | Salio::No(_)) => continue,
            Salio::FaltaOtro => b"no se dio: el paso que pide no esta hecho (quita su `-`, o daselo a mano)",
            Salio::No(_) => b"NO salio: su fila en `gpu` dice por que; la foto de antes esta en el disco y `cabina fallos` lo ultimo",
        };
        g.text(b"    ");
        g.text(paso.nombre);
        g.text(b": ");
        g.text(texto);
        g.byte(b'\n');
        dichas += 1;
    }
    if dichas == 0 {
        g.text(b"    nada que mirar: ningun paso se dio\n");
    }
    g.with_ink(INK_ECHO);
    g.text(if armado {
        b"    al reiniciar la maquina APAGA todo esto, y el modo ARMADO lo vuelve a dar solo\n" as &[u8]
    } else {
        b"    al reiniciar todo esto vuelve a APAGADO, y el modo no esta armado: no se repite\n"
    });
    g.with_ink(INK_PLAIN);
}
