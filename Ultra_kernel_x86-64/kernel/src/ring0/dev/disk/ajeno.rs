//! **EL DISCO AJENO, SOLO LECTURA** (N1a, 29-09).
//!
//! [carril]  ROJO      lee por DMA de OTRO disco: equivocarse aqui escribe donde vive el propietario
//! [consumo] NADA      corre una vez al arrancar y cuando alguien lee el disco Personal
//!
//! El propietario (29-09): *"que mi BMO-X aprenda a LEER NTFS"*: su
//! Cyberpunk 2077 vive en `Personal (D:)`, en OTRO disco. Medido en su Windows
//! (solo lectura, `PLAN_LA_LUDOTECA.md`, N1): los dos SSD SATA cuelgan del
//! MISMO controlador AHCI; solo los separa el PUERTO. Hasta hoy el kernel
//! manejaba UN disco a proposito ("uno solo, los otros son ajenos", arriba en
//! `mod.rs`). Esto abre UNA excepcion, y la abre con dos cerrojos:
//!
//! ```text
//!    cerrojo 1  este dispositivo NO SABE escribir: `write` = ReadOnly,
//!               `writable` = false, y nunca se registra como el disco de BMO-X
//!               (`bmo_block::register` sigue siendo solo para `AHCI_DISK`)
//!    cerrojo 2  la escritura, el TRIM y el FLUSH del disco de BMO-X miran
//!               antes que su puerto NO sea el de este (`escribible`): si un
//!               dia llegaran a apuntarle, se niegan y lo dicen
//! ```
//!
//! Y el guardian `toolchain/tools/ajeno/ajeno.py` hace que los dos sigan ahi.
//!
//! Lo que hace, una vez, tras montar el disco de BMO-X: busca OTRO disco SATA
//! en el mismo controlador, le pide IDENTIFY (modelo, serie, sectores), lee su
//! GPT, encuentra la particion NTFS y la monta con `bmo-ntfs` (N0). La cabina
//! lo dice todo, y la raiz del volumen. `C:` (el NVMe) ni se mira: el kernel
//! solo busca controladores SATA/AHCI.
//!
//! Las lecturas usan el MISMO juez de DMA y el MISMO registro de vuelos que el
//! disco de siempre (`transfer::juzgar_el_dma`, `marcar_el_tramo`), por un
//! rebote propio: 1 MiB contiguo, 2048 sectores por comando (P0.4d, 30-09:
//! con una pagina, 8 sectores por comando, Cyberpunk se leia a 10 MiB/s --
//! 55.000 comandos de ~0,37 ms --; si al arrancar no hay 1 MiB seguido, se
//! prueba con menos, hasta la pagina de antes).

use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicU8, Ordering};

use bmo_block::{BlockDevice, BlockError, DeviceId, SECTOR};

use crate::ring0::mm::{self, phys};

/// El puerto AHCI del disco ajeno (`NINGUNO` = no hay).
static PUERTO: AtomicU8 = AtomicU8::new(NINGUNO);
const NINGUNO: u8 = 0xFF;
/// Su rebote (fisico, contiguo). Lo escribe `buscar`, una vez, al arrancar.
static mut DMA: u64 = 0;
/// Cuantos sectores caben en el rebote: los de un comando.
static LOTE: AtomicU16 = AtomicU16::new(0);
/// Lo que se intenta para el rebote, en paginas: 1 MiB, y si no, menos.
const REBOTES: [u64; 4] = [256, 64, 16, 1];
/// Quien es, segun su IDENTIFY. La escribe `buscar`, una vez.
static mut ID: DeviceId = DeviceId::EMPTY;
/// La ranura 0 del puerto es UNA: un comando a la vez.
static EN_USO: AtomicBool = AtomicBool::new(false);

/// El puerto del disco ajeno, si lo hay.
pub fn puerto() -> Option<u8> {
    match PUERTO.load(Ordering::Acquire) {
        NINGUNO => None,
        p => Some(p),
    }
}

/// **CERROJO 2**: se puede escribir en `p`. Lo miran la escritura, el TRIM y
/// el FLUSH del disco de BMO-X antes de mandar nada.
pub fn escribible(p: u8) -> bool {
    if puerto() == Some(p) {
        crate::ring0::cabina::fault("disk", "CERROJO: una escritura iba al puerto del disco AJENO (solo lectura); no se manda", p as u64);
        return false;
    }
    true
}

/// **El disco ajeno, por el contrato de bloques, SOLO LECTURA.**
pub struct Ajeno;
pub static AJENO: Ajeno = Ajeno;

impl BlockDevice for Ajeno {
    fn identity(&self) -> DeviceId {
        // SAFETY: `ID` se escribe una vez en `buscar`, antes de publicar el
        // puerto; despues solo se lee.
        unsafe { *core::ptr::addr_of!(ID) }
    }

    fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, BlockError> {
        let Some(p) = puerto() else { return Err(BlockError::NotReady) };
        // SAFETY: como `ID`.
        let dma = unsafe { DMA };
        if dma == 0 {
            return Err(BlockError::NotReady);
        }
        let bytes = count as usize * SECTOR;
        if buf.len() < bytes {
            return Err(BlockError::ShortBuffer);
        }
        if lba.checked_add(count as u64).is_none_or(|f| f > self.identity().blocks) {
            return Err(BlockError::OutOfRange);
        }
        if EN_USO.swap(true, Ordering::AcqRel) {
            return Err(BlockError::NotReady);
        }
        let r = leer(p, dma, lba, count, &mut buf[..bytes]);
        EN_USO.store(false, Ordering::Release);
        r
    }

    /// **CERROJO 1**: no sabe escribir.
    fn write(&self, _lba: u64, _count: u16, _data: &[u8]) -> Result<u16, BlockError> {
        Err(BlockError::ReadOnly)
    }

    /// Nada que bajar: nunca se escribio nada.
    fn flush(&self) -> Result<(), BlockError> {
        Ok(())
    }

    fn writable(&self) -> bool {
        false
    }
}

/// Lee de 8 sectores en 8 por la pagina de rebote, con el juez y el vuelo.
/// Lo mas que pide UN comando directo: una entrada de PRDT (22 bits), 4 MiB.
const MAX_DIRECTO: u16 = 8192;

/// Bytes que llegaron a su sitio sin rebote, y los que rebotaron (CABINA).
static DIRECTOS: AtomicU64 = AtomicU64::new(0);
static REBOTADOS: AtomicU64 = AtomicU64::new(0);

/// `(directos, rebotados)` en bytes desde el arranque.
pub fn cuentas() -> (u64, u64) {
    (DIRECTOS.load(Ordering::Relaxed), REBOTADOS.load(Ordering::Relaxed))
}

fn leer(p: u8, dma: u64, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, BlockError> {
    let lote = LOTE.load(Ordering::Acquire).max(1);
    let mut hecho = 0u16;
    while hecho < count {
        // ** DIRECTO, como el disco de BMO-X (`transfer::read`): si el trozo
        // que toca es contiguo en el physmap -- el bloque de una app al que
        // escribe `ARCH_OP_LEER_EN` lo es --, el HBA escribe AHI, sin rebote
        // ni copia y hasta 4 MiB por comando. Mismo juez (`prestando`: el
        // bufer es del que llama, no del aparato) y mismo bit en vuelo.
        let va = buf.as_ptr() as u64 + hecho as u64 * SECTOR as u64;
        let resto = (count - hecho) as u64 * SECTOR as u64;
        if let Some((fisica, bytes)) = super::transfer::tramo_dma(va, resto) {
            let n = ((bytes / SECTOR as u64) as u16).min(count - hecho).min(MAX_DIRECTO);
            if n > 0 && super::transfer::juzgar_el_dma(fisica, n as u64 * SECTOR as u64, true) {
                let b = n as u64 * SECTOR as u64;
                super::transfer::marcar_el_tramo(fisica, b, true, crate::ring0::task::scheduler::rdtsc());
                // SAFETY: el puerto `p` lo preparo `init_port_dma`; `fisica`
                // son `b` bytes contiguos del bufer de quien llama (el
                // physmap es lineal), juzgados arriba.
                let r = unsafe { bmo_ahci::read_sectors_phys(p, lba + hecho as u64, n, fisica) };
                super::transfer::marcar_el_tramo(fisica, b, false, crate::ring0::task::scheduler::rdtsc());
                let k = match r {
                    Ok(k) if k > 0 => k.min(n),
                    Ok(_) => return Err(BlockError::Device),
                    Err(e) => {
                        crate::ring0::cabina::warn("disk", e.name(), lba + hecho as u64);
                        return Err(BlockError::Device);
                    }
                };
                DIRECTOS.fetch_add(k as u64 * SECTOR as u64, Ordering::Relaxed);
                super::trafico::leido(k as u64 * SECTOR as u64);
                hecho += k;
                continue;
            }
        }
        let n = (count - hecho).min(lote);
        let bytes = n as u64 * SECTOR as u64;
        if !super::transfer::juzgar_el_dma(dma, bytes, false) {
            return Err(BlockError::Device);
        }
        super::transfer::marcar_el_tramo(dma, bytes, true, crate::ring0::task::scheduler::rdtsc());
        // SAFETY: el puerto `p` lo preparo `init_port_dma` en `buscar`, y la
        // pagina `dma` es nuestra (contigua, Neutro) y cabe `n` sectores.
        let r = unsafe { bmo_ahci::read_sectors_phys(p, lba + hecho as u64, n, dma) };
        super::transfer::marcar_el_tramo(dma, bytes, false, crate::ring0::task::scheduler::rdtsc());
        // Una lectura CORTA es legal (el disco dijo basta antes): se copia lo
        // que llego y se sigue desde ahi. Cero es un fallo.
        let n = match r {
            Ok(k) if k > 0 => k.min(n),
            Ok(_) => return Err(BlockError::Device),
            Err(e) => {
                crate::ring0::cabina::warn("disk", e.name(), lba + hecho as u64);
                return Err(BlockError::Device);
            }
        };
        let bytes = n as u64 * SECTOR as u64;
        let desde = hecho as usize * SECTOR;
        // SAFETY: el rebote mide `LOTE` sectores, contiguo, por el physmap.
        let src = unsafe { core::slice::from_raw_parts(mm::phys_to_virt(dma) as *const u8, bytes as usize) };
        buf[desde..desde + bytes as usize].copy_from_slice(src);
        REBOTADOS.fetch_add(bytes, Ordering::Relaxed);
        super::trafico::leido(bytes);
        hecho += n;
    }
    Ok(count)
}

// == Buscarlo, una vez, al arrancar ==========================================

/// Su modelo y su serie en texto (para la cabina).
static mut MODELO: [u8; 40] = [0; 40];
static mut SERIE: [u8; 20] = [0; 20];

/// **Buscar el disco ajeno** en el controlador del disco de BMO-X (`suyo` =
/// su puerto). El primero que sea un disco SATA, conteste a IDENTIFY y NO
/// tenga la serie del de BMO-X. Luego, su NTFS.
pub(super) fn buscar(suyo: u8) {
    let Some(ctrl) = bmo_ahci::controller() else { return };
    // ** EL METAL (29-09): a las 10:54 "sin otro disco SATA; con disco: 2",
    // y a las 11:06 `PI 0x33` (0, 1, 4, 5) con el disco de BMO-X en el 2 y
    // TODOS los demas con DET 0. En Windows el SSD de D: SI esta en este
    // controlador ("Port 4" el de BMO-X, "Port 5" el de D:). En esta placa
    // `PI` no dice que puertos hay, y un puerto que el firmware no encendio
    // dice DET 0 aunque tenga un disco detras. Asi que:
    //
    // 1. Si ningun otro puerto tiene disco, SEGUNDA OPORTUNIDAD a todos los
    //    de `NP` menos el de BMO-X, juntos y con UNA espera (hasta 1,5 s, y
    //    solo el arranque que no lo encuentra): arrancar el disco (SSS),
    //    COMRESET, esperar (`bmo_ahci::reanimar`). Un puerto con enlace no se
    //    toca, y el de BMO-X tampoco.
    // 2. El detalle de "no hay otro" dice lo que vio CADA puerto DESPUES: los
    //    que el HBA declara (PI, 8 bits), el DET de los 8 primeros (4 bits
    //    cada uno) y cuales traen firma de disco SATA.
    let n = (ctrl.port_count as usize).min(8);
    let otro = (0..n).any(|i| {
        let pt = &ctrl.ports[i];
        i as u8 != suyo && pt.state == bmo_ahci::PortState::Active && pt.signature == bmo_ahci::SIG_SATA_DISK
    });
    if !otro {
        let mascara = ((1u32 << n) - 1) & !(1u32 << suyo);
        // SAFETY: puertos SIN enlace (los activos se saltan dentro): no tienen
        // nada en marcha que romper, y el de BMO-X queda fuera de la mascara.
        let vivos = unsafe { bmo_ahci::reanimar(mascara) };
        crate::ring0::cabina::info("disk", "N1a: segunda oportunidad a los puertos sin enlace; ahora vivos (mascara)", vivos as u64);
    }
    let Some(ctrl) = bmo_ahci::controller() else { return };
    let mut det = 0u64;
    let mut sata = 0u64;
    for i in 0..n {
        let pt = &ctrl.ports[i];
        det |= ((pt.ssts & 0xF) as u64) << (4 * i);
        if pt.signature == bmo_ahci::SIG_SATA_DISK {
            sata |= 1 << i;
        }
    }
    etapa(ETAPA_SIN_OTRO, (ctrl.ports_implemented as u64 & 0xFF) | det << 8 | sata << 40);
    for i in 0..(ctrl.port_count as usize).min(32) {
        let pt = &ctrl.ports[i];
        if i as u8 == suyo || pt.state != bmo_ahci::PortState::Active || pt.signature != bmo_ahci::SIG_SATA_DISK {
            continue;
        }
        crate::ring0::cabina::info("disk", "N1a: otro disco SATA en el mismo controlador; puerto", i as u64);
        // SAFETY: un puerto con enlace vivo y disco; se preparan SUS
        // estructuras (lista, FIS, tabla), no las del disco de BMO-X.
        if !unsafe { bmo_ahci::init_port_dma(i as u8) } {
            crate::ring0::cabina::warn("disk", "N1a: el puerto del disco ajeno no se preparo", i as u64);
            etapa(ETAPA_PUERTO, i as u64);
            continue;
        }
        let Some((dma, paginas)) = REBOTES.iter().find_map(|&k| phys::alloc_frames_contig_de(k, phys::Titular::Neutro).map(|d| (d, k))) else {
            crate::ring0::cabina::warn("disk", "N1a: sin memoria para el rebote del disco ajeno", 0);
            return;
        };
        // Un puerto que se descarta devuelve su rebote (ahora es 1 MiB).
        let soltar = || {
            for k in 0..paginas {
                phys::free_frame_de(dma + k * mm::PAGE, phys::Titular::Neutro);
            }
        };
        // SAFETY: el puerto `i` quedo preparado arriba; la pagina es nuestra.
        if let Err(e) = unsafe { bmo_ahci::identify_phys(i as u8, dma) } {
            crate::ring0::cabina::warn("disk", e.name(), i as u64);
            etapa(ETAPA_IDENTIFY, i as u64);
            soltar();
            continue;
        }
        let src = mm::phys_to_virt(dma) as *const u8;
        // SAFETY: al arrancar, antes de publicar el puerto: nadie mas lee
        // estos estaticos todavia.
        unsafe {
            let modelo = &mut *core::ptr::addr_of_mut!(MODELO);
            let serie = &mut *core::ptr::addr_of_mut!(SERIE);
            let nm = super::ata_string(src, 27, 47, modelo);
            let ns = super::ata_string(src, 10, 20, serie);
            let mut total = 0u64;
            for k in (0..4usize).rev() {
                let w = 100 + k;
                total = (total << 16) | (src.add(w * 2 + 1).read_volatile() as u64) << 8 | src.add(w * 2).read_volatile() as u64;
            }
            // Mismo disco que el de BMO-X (la misma serie): no es ajeno.
            if ns > 0 && super::serial().as_bytes() == &serie[..ns] {
                crate::ring0::cabina::warn("disk", "N1a: ese puerto es el MISMO disco de BMO-X (misma serie): no se toca", i as u64);
                etapa(ETAPA_MISMO, i as u64);
                soltar();
                continue;
            }
            let mut id = DeviceId { blocks: total, ..DeviceId::EMPTY };
            id.model[..nm].copy_from_slice(&modelo[..nm]);
            id.model_len = nm;
            id.serial[..ns].copy_from_slice(&serie[..ns]);
            id.serial_len = ns;
            *core::ptr::addr_of_mut!(ID) = id;
            DMA = dma;
            LOTE.store((paginas * mm::PAGE / SECTOR as u64) as u16, Ordering::Release);
            crate::ring0::cabina::bytes("disk", "N1a: rebote del disco ajeno (un comando lee esto)", paginas * mm::PAGE);
            crate::ring0::cabina::info("disk", "N1a: disco AJENO, SOLO LECTURA; modelo", nm as u64);
            crate::ring0::cabina::info("disk", core::str::from_utf8(&modelo[..nm]).unwrap_or("?"), total);
            crate::ring0::cabina::info("disk", core::str::from_utf8(&serie[..ns]).unwrap_or("?"), i as u64);
        }
        PUERTO.store(i as u8, Ordering::Release);
        montar_ntfs();
        return;
    }
}

// == Su NTFS ==================================================================

/// El volumen NTFS del disco ajeno (`bmo-ntfs`, ~28 KiB aqui y no en la pila).
static mut VOLUMEN: bmo_ntfs::Volumen<'static> = bmo_ntfs::Volumen::vacio(&AJENO);
static MONTADO: AtomicBool = AtomicBool::new(false);
/// Lo que mide el volumen y lo que le queda, contado UNA vez al montar (en
/// su `$Bitmap`): el disco es de solo lectura, desde aqui no cambia.
/// `LIBRES = u64::MAX`: no se pudo contar.
static BYTES: AtomicU64 = AtomicU64::new(0);
/// **Donde se paro N1a**, para que la solapa `equipo` diga POR QUE el disco
/// Personal no esta montado (el anillo de la cabina guarda 48 eventos y los
/// del arranque se van). `etapa | detalle << 8`; las etapas, en el ABI
/// (`INFO_UNIDAD`, `que 2`). Visto en el metal el 29-09: "no montada" sin
/// mas, y sin forma de saber en que paso.
static ETAPA: AtomicU64 = AtomicU64::new(0);
pub const ETAPA_SIN_OTRO: u64 = 1;
pub const ETAPA_PUERTO: u64 = 2;
pub const ETAPA_IDENTIFY: u64 = 3;
pub const ETAPA_MISMO: u64 = 4;
pub const ETAPA_TABLA: u64 = 5;
pub const ETAPA_SIN_NTFS: u64 = 6;
pub const ETAPA_NO_MONTA: u64 = 7;
pub const ETAPA_MONTADO: u64 = 8;

fn etapa(e: u64, detalle: u64) {
    ETAPA.store(e | detalle << 8, Ordering::Relaxed);
}

/// Donde se paro N1a (ver [`ETAPA`]). 0 = no se busco.
pub fn etapa_n1a() -> u64 {
    ETAPA.load(Ordering::Relaxed)
}
static LIBRES: AtomicU64 = AtomicU64::new(u64::MAX);

/// **`(bytes, libres)` del disco PERSONAL**, si esta montado. Para
/// `INFO_UNIDAD` (la solapa `equipo` de ESTRATOS).
pub fn espacio() -> Option<(u64, Option<u64>)> {
    if !MONTADO.load(Ordering::Acquire) {
        return None;
    }
    let l = LIBRES.load(Ordering::Relaxed);
    Some((BYTES.load(Ordering::Relaxed), (l != u64::MAX).then_some(l)))
}

/// El volumen NTFS del disco ajeno, si esta montado. Un llamante a la vez
/// (el kernel de hoy: el arranque; N1b lo pondra detras de su cerrojo).
pub fn volumen() -> Option<&'static mut bmo_ntfs::Volumen<'static>> {
    // SAFETY: se monta una vez al arrancar; ver la nota de arriba.
    MONTADO.load(Ordering::Acquire).then(|| unsafe { &mut *core::ptr::addr_of_mut!(VOLUMEN) })
}

// == N1b: lo que Ring 3 puede pedir del disco Personal (SOLO LEER) ============
//
// La puerta es `d:` en las rutas de siempre (`obj/directory.rs` y
// `obj/file.rs`): `d:Cyberpunk 2077/bin`, `D:\\Cyberpunk 2077\\bin`. Lo de
// aqui LEE: recorrer una carpeta, medir un nodo y leer bytes de un fichero.
// Crear o escribir en `d:` se niega en `obj/file.rs`, y este fichero no sabe
// escribir (cerrojo 1).

/// **`d:resto` -> `resto`**, la ruta dentro del disco Personal (sin la
/// letra ni la barra de delante). `None` si la ruta no es de `d:`.
pub fn ruta_personal(ruta: &str) -> Option<&str> {
    let b = ruta.as_bytes();
    if b.len() >= 2 && (b[0] | 0x20) == b'd' && b[1] == b':' {
        Some(ruta[2..].trim_start_matches(['/', '\\']))
    } else {
        None
    }
}

/// **Abrir una ruta del disco Personal.** `None` si no esta montado.
pub fn abrir(ruta: &str) -> Option<Result<bmo_ntfs::Nodo, bmo_ntfs::NoNtfs>> {
    Some(volumen()?.abrir(ruta))
}

/// **Leer** `dst` desde `off` del fichero del registro `registro` (que mide
/// `medida`). Devuelve cuantos bytes llegaron; 0 si se acabo o fallo.
pub fn leer_fichero(registro: u64, medida: u64, off: u64, dst: &mut [u8]) -> usize {
    let Some(v) = volumen() else { return 0 };
    let n = bmo_ntfs::Nodo { registro, carpeta: false, medida };
    v.leer(&n, off, dst).unwrap_or(0)
}

/// **La entrada `n` (desde 0) de la carpeta `dir`**: su nombre en UTF-8 en
/// `nombre` y `(bytes del nombre, carpeta, medida, [creado, escrito, leido],
/// atributos)`. `None` cuando se acaban. Las fechas y los atributos (01-10)
/// salen de la misma clave del indice: no cuestan lecturas.
///
/// [!] Recorre la carpeta desde el principio en cada llamada: pedir las N es
/// cuadratico. Una carpeta de Cyberpunk tiene cientos, no millones; el dia
/// que pese se guarda por donde iba.
pub fn entrada(dir: u64, n: usize, nombre: &mut [u8]) -> Option<(usize, bool, u64, [u64; 3], u32)> {
    let v = volumen()?;
    let mut k = 0usize;
    let mut hallada = None;
    let r = v.recorrer(dir, &mut |e| {
        if k == n {
            let largo = e.nombre_utf8(nombre);
            hallada = Some((largo, e.carpeta, e.medida, e.fechas, e.atributos));
            return true;
        }
        k += 1;
        false
    });
    if r.is_err() {
        return None;
    }
    hallada
}

/// **Su tabla de particiones, su particion NTFS, montada, y la raiz a la
/// cabina.** GPT primero; si no hay GPT, la MBR (Ventoy, que vive en este
/// disco, formatea en MBR por defecto: tipo 0x07 es NTFS).
fn montar_ntfs() {
    let mut s = [0u8; SECTOR];
    let mut vistas = 0u64;
    let gpt = if AJENO.read(1, 1, &mut s).is_ok() { bmo_particiones::cabecera(&s).ok() } else { None };
    if let Some(gpt) = gpt {
        let por = gpt.por_sector();
        let sectores = (gpt.entry_count.min(128) as usize).div_ceil(por.max(1));
        for k in 0..sectores {
            if AJENO.read(gpt.entries_lba + k as u64, 1, &mut s).is_err() {
                crate::ring0::cabina::warn("ntfs", "N1a: no se pudo leer la tabla GPT del disco ajeno", k as u64);
                etapa(ETAPA_TABLA, 1);
                return;
            }
            for j in 0..por.min(4) {
                let Some(p) = bmo_particiones::entrada(&s, j * gpt.entry_size as usize, (k * por + j) as u32) else { continue };
                vistas += 1;
                crate::ring0::cabina::info("ntfs", "N1a: particion GPT del disco ajeno; empieza en la LBA", p.first_lba);
                if p.is_basic_data() && intentar(p.first_lba) {
                    return;
                }
            }
        }
    } else {
        // La MBR: la firma 0x55AA y cuatro entradas de 16 B desde el 446.
        if AJENO.read(0, 1, &mut s).is_err() || s[510] != 0x55 || s[511] != 0xAA {
            crate::ring0::cabina::warn("ntfs", "N1a: el disco ajeno no tiene ni GPT ni MBR", 0);
            etapa(ETAPA_TABLA, 2);
            return;
        }
        for j in 0..4usize {
            let e = &s[446 + j * 16..446 + j * 16 + 16];
            let lba = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as u64;
            if e[4] == 0 || lba == 0 {
                continue;
            }
            vistas += 1;
            crate::ring0::cabina::info("ntfs", "N1a: particion MBR del disco ajeno; tipo", e[4] as u64);
            if e[4] == 0x07 && intentar(lba) {
                return;
            }
        }
    }
    if etapa_n1a() & 0xFF != ETAPA_NO_MONTA {
        etapa(ETAPA_SIN_NTFS, vistas);
    }
    crate::ring0::cabina::warn("ntfs", "N1a: el disco ajeno no tiene una particion NTFS que se monte", vistas);
}

/// Monta el NTFS de la particion que empieza en `lba`, si lo es. `true` si
/// quedo montado.
fn intentar(lba: u64) -> bool {
    let mut arranque = [0u8; SECTOR];
    if AJENO.read(lba, 1, &mut arranque).is_err() || bmo_ntfs::forma(&arranque).is_none() {
        return false;
    }
    // SAFETY: al arrancar, un solo hilo; ver `volumen`.
    let v = unsafe { &mut *core::ptr::addr_of_mut!(VOLUMEN) };
    match v.montar_aqui(lba) {
        Ok(()) => {
            crate::ring0::cabina::info("ntfs", "N1a: NTFS MONTADO, solo lectura; bytes por cluster", v.forma.bytes_por_cluster);
            let bpc = v.forma.bytes_por_cluster;
            BYTES.store(v.clusteres() * bpc, Ordering::Relaxed);
            match v.libres() {
                Ok(l) => {
                    LIBRES.store(l * bpc, Ordering::Relaxed);
                    crate::ring0::cabina::info("ntfs", "N1a: libres (MiB), contados en su $Bitmap", l * bpc >> 20);
                }
                Err(e) => crate::ring0::cabina::warn("ntfs", e.nombre(), 0),
            }
            etapa(ETAPA_MONTADO, 0);
            // Montado DESPUES de las cuentas: quien lo vea montado ve los numeros.
            MONTADO.store(true, Ordering::Release);
            decir_la_raiz(v);
            true
        }
        Err(e) => {
            crate::ring0::cabina::warn("ntfs", e.nombre(), lba);
            etapa(ETAPA_NO_MONTA, codigo(e));
            false
        }
    }
}

/// El numero de cada motivo de `bmo_ntfs::NoNtfs`, para el detalle de la etapa.
fn codigo(e: bmo_ntfs::NoNtfs) -> u64 {
    use bmo_ntfs::NoNtfs as N;
    match e {
        N::Leer => 1,
        N::NoEsNtfs => 2,
        N::Forma(_) => 3,
        N::Grande(_) => 4,
        _ => 5,
    }
}

/// Las primeras entradas de la raiz a la cabina (valor: 1 si es carpeta), y
/// la prueba de que se leen DATOS: el `MZ` del `.exe` de Cyberpunk, si esta.
fn decir_la_raiz(v: &mut bmo_ntfs::Volumen<'static>) {
    let mut vistas = 0u32;
    let r = v.recorrer(bmo_ntfs::RAIZ, &mut |e| {
        let mut b = [0u8; 96];
        let n = e.nombre_utf8(&mut b);
        if !b[..n].starts_with(b"$") {
            crate::ring0::cabina::info("ntfs", core::str::from_utf8(&b[..n]).unwrap_or("?"), e.carpeta as u64);
            vistas += 1;
        }
        vistas >= 24
    });
    if let Err(e) = r {
        crate::ring0::cabina::warn("ntfs", e.nombre(), 0);
        return;
    }
    match v.abrir("Cyberpunk 2077/bin/x64/Cyberpunk2077.exe") {
        Ok(n) => {
            let mut mz = [0u8; 2];
            let firma = match v.leer(&n, 0, &mut mz) {
                Ok(2) => u16::from_le_bytes(mz) as u64,
                _ => 0,
            };
            crate::ring0::cabina::info("ntfs", "N1a: Cyberpunk2077.exe HALLADO; bytes", n.medida);
            crate::ring0::cabina::info("ntfs", "N1a: sus dos primeros bytes (0x5A4D = MZ)", firma);
        }
        Err(e) => crate::ring0::cabina::info("ntfs", e.nombre(), 0),
    }
}
