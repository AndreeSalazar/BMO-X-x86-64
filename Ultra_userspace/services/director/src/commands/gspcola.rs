//! **`gpu cola`: L0c4a, LO QUE EL GSP YA DIJO.** Recorre su cola de mensajes
//! --la que escribe el GSP y lee la CPU-- desde donde la CPU iria a leer hasta
//! donde el GSP escribio, y dice que mensajes son: cuantos de cada tipo, si
//! todos traen su firma y su suma, y si esta el `GSP_INIT_DONE`.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: ~33.000
//!                     preguntas de 8 bytes, unas decenas de ms
//!
//! # Lo que NO hace
//!
//! No mueve el puntero de lectura de la CPU: para el GSP, nadie ha leido nada
//! todavia. Contestar --el secuenciador, SetSystemInfo, SetRegistry-- es L0c4b.
//!
//! # Por que desde aqui y sin tocar el kernel (2026-09-24)
//!
//! Porque ya se puede: `INFO_GPU_GSP_MEM` (L0c3b) deja leer GspMem de 8 en 8
//! bytes, y `bmo_gpu_ga10x::rpc` entiende lo leido, probado en el anfitrion.
//! La cola entera se guarda cruda en `datos/gspcola.bin`, como la ROM en L0a.

use bmo_gpu_ga10x::rpc::{self, Mensaje, Suma, CABECERA};
use bmo_userland as bmo;

use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Donde empieza GspMem en `INFO_GPU_GSP_MEM`: detras de los tres logs.
const GSPMEM: u64 = 3 * 16 * 4096;
/// La cola del GSP dentro de GspMem, su `writePtr`, y el `readPtr` de la CPU
/// sobre ella (que vive en la cabecera de la cola de la CPU).
const COLA_GSP: u64 = GSPMEM + 0x41000;
const ESCRITO: u64 = COLA_GSP + 16;
const LEIDO_CPU: u64 = GSPMEM + 0x1000 + 32;
/// Los mensajes: 63 paginas tras la de cabeceras.
const DATOS: u64 = COLA_GSP + 0x1000;
const PAGINAS: u64 = 63;
const PAGINA: u64 = 4096;
/// La cola entera, con su pagina de cabeceras, para el disco.
const COLA_BYTES: u64 = (1 + PAGINAS) * PAGINA;
const RUTA: &[u8] = b"datos/gspcola.bin";
/// Tipos distintos que se cuentan.
const MAX_TIPOS: usize = 12;

#[derive(Clone, Copy)]
struct Resumen {
    escrito: u32,
    leido: u32,
    mensajes: u32,
    /// Sin firma VRPC, version 3.0, o que no caben en sus paginas.
    malformados: u32,
    /// Bien formados, pero su suma no da 0.
    sumas_mal: u32,
    /// Con `rpc_result` distinto de 0.
    con_error: u32,
    primera_secuencia: u32,
    ultima_secuencia: u32,
    tipos: [(u32, u32); MAX_TIPOS],
    n_tipos: usize,
    init_done: bool,
    guardada: bool,
}

static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

fn mem(desde: u64) -> u64 {
    bmo::info(bmo::INFO_GPU_GSP_MEM | desde << 8)
}

/// 8 bytes de la cola, contando desde la pagina `pagina`; da la vuelta.
fn cola(pagina: u64, desde: u64) -> u64 {
    mem(DATOS + (pagina * PAGINA + desde) % (PAGINAS * PAGINA))
}

/// La cabecera del mensaje que empieza en `pagina`.
fn cabecera(pagina: u64) -> [u8; CABECERA] {
    let mut b = [0u8; CABECERA];
    for k in 0..CABECERA / 8 {
        b[k * 8..k * 8 + 8].copy_from_slice(&cola(pagina, k as u64 * 8).to_le_bytes());
    }
    b
}

/// La suma del mensaje entero: cabecera y datos, como nova-core.
fn suma(pagina: u64, m: &Mensaje) -> u32 {
    let mut s = Suma::default();
    let n = m.bytes_sumados() as u64;
    let mut o = 0;
    while o < n {
        let w = cola(pagina, o).to_le_bytes();
        let k = (n - o).min(8) as usize;
        s.mas(&w[..k]);
        o += 8;
    }
    s.valor()
}

/// **Leer la cola, sin tocarla.** `Ok(mensajes)`.
pub(crate) fn leer() -> Result<u64, u32> {
    if bmo::info(bmo::INFO_GPU_DESPIERTO) & bmo::DESPIERTO_VISTO == 0 {
        return Err(NO_COLA_SIN_GSP);
    }
    let escrito = (mem(ESCRITO) & 0xFFFF_FFFF) as u32;
    let leido = (mem(LEIDO_CPU) & 0xFFFF_FFFF) as u32;
    let mut r = Resumen {
        escrito,
        leido,
        mensajes: 0,
        malformados: 0,
        sumas_mal: 0,
        con_error: 0,
        primera_secuencia: 0,
        ultima_secuencia: 0,
        tipos: [(0, 0); MAX_TIPOS],
        n_tipos: 0,
        init_done: false,
        guardada: false,
    };
    let (fin, mut p) = (escrito as u64 % PAGINAS, leido as u64 % PAGINAS);
    let mut vueltas = 0;
    while p != fin && vueltas < PAGINAS {
        let m = Mensaje::de(&cabecera(p));
        if !m.bien_formado() {
            // Sin cabecera buena no se sabe cuanto mide: no se sigue.
            r.malformados += 1;
            break;
        }
        if suma(p, &m) != 0 {
            r.sumas_mal += 1;
        }
        if m.resultado != 0 {
            r.con_error += 1;
        }
        if r.mensajes == 0 {
            r.primera_secuencia = m.numero;
        }
        r.ultima_secuencia = m.numero;
        r.init_done |= m.funcion == rpc::INIT_DONE;
        match r.tipos[..r.n_tipos].iter_mut().find(|t| t.0 == m.funcion) {
            Some(t) => t.1 += 1,
            None if r.n_tipos < MAX_TIPOS => {
                r.tipos[r.n_tipos] = (m.funcion, 1);
                r.n_tipos += 1;
            }
            None => {}
        }
        r.mensajes += 1;
        p = (p + m.paginas as u64) % PAGINAS;
        vueltas += m.paginas as u64;
    }
    r.guardada = guardar();
    // SAFETY: como `resumen`.
    unsafe {
        *core::ptr::addr_of_mut!(RESUMEN) = Some(r);
    }
    if r.mensajes == 0 || r.malformados != 0 || r.sumas_mal != 0 {
        return Err(NO_COLA_MAL);
    }
    Ok(r.mensajes as u64)
}

/// La cola entera, cruda, a `datos/gspcola.bin`.
fn guardar() -> bool {
    let Some(bloque) = bmo::Memoria::request(COLA_BYTES) else { return false };
    // SAFETY: el bloque mide COLA_BYTES, es de este proceso y se escribe aqui
    // antes de leerse.
    let b = unsafe { core::slice::from_raw_parts_mut(bloque.base(), COLA_BYTES as usize) };
    for o in (0..COLA_BYTES).step_by(8) {
        b[o as usize..o as usize + 8].copy_from_slice(&mem(COLA_GSP + o).to_le_bytes());
    }
    match bmo::Archivo::create(RUTA) {
        Ok(a) => a.escribir_de(&bloque, 0, COLA_BYTES) == COLA_BYTES && a.close(),
        Err(_) => false,
    }
}

/// Motivos del escritorio (`gsp.rs` va hasta 0x11A).
pub(crate) const NO_COLA_SIN_GSP: u32 = 0x11B;
pub(crate) const NO_COLA_MAL: u32 = 0x11C;

/// Lo pregunta `save mode`: leida, con mensajes, y todos bien.
pub(crate) fn leida() -> bool {
    resumen().map_or(false, |r| r.mensajes > 0 && r.malformados == 0 && r.sumas_mal == 0)
}

/// `gpu cola`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "leyendo la cola del GSP", INK_DIM);
    let r = leer();
    let g = &mut dsk.out.grid;
    match r {
        Ok(n) => {
            g.with_ink(INK_GOOD);
            g.text(b"  LA COLA DEL GSP: ");
            g.dec(n);
            g.text(b" mensajes leidos, todos con su firma y su suma; cruda en datos/gspcola.bin\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  NO: ");
            g.text(super::iommu::motivo(m));
            g.byte(b'\n');
        }
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "cola", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Las filas de L0c4a**, si se leyo.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    campo(s, b"cola");
    let bien = r.mensajes > 0 && r.malformados == 0 && r.sumas_mal == 0;
    s.with_ink(if bien { INK_GOOD } else { INK_ERR });
    s.dec(r.mensajes as u64);
    s.text(b" mensajes del GSP");
    s.with_ink(INK_PLAIN);
    s.text(b" en las paginas ");
    s.dec(r.leido as u64);
    s.text(b"..");
    s.dec(r.escrito as u64);
    s.text(b" de su cola; secuencia ");
    s.dec(r.primera_secuencia as u64);
    s.text(b"..");
    s.dec(r.ultima_secuencia as u64);
    if r.malformados != 0 {
        s.with_ink(INK_ERR);
        s.text(b"; UNO SIN FORMA de mensaje: se paro ahi");
    }
    if r.sumas_mal != 0 {
        s.with_ink(INK_ERR);
        s.text(b"; ");
        s.dec(r.sumas_mal as u64);
        s.text(b" con la suma MAL");
    }
    if r.con_error != 0 {
        s.with_ink(INK_ECHO);
        s.text(b"; ");
        s.dec(r.con_error as u64);
        s.text(b" con rpc_result distinto de 0");
    }
    s.with_ink(INK_ECHO);
    s.text(if r.guardada { b"   -> datos/gspcola.bin" as &[u8] } else { b"   (NO se pudo guardar)" });
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    // Los tipos, de tres en tres por fila.
    for (i, t) in r.tipos[..r.n_tipos].iter().enumerate() {
        if i % 3 == 0 {
            if i > 0 {
                s.byte(b'\n');
            }
            campo(s, b"dijo");
        } else {
            s.text(b",  ");
        }
        s.with_ink(if t.0 == rpc::INIT_DONE || t.0 == rpc::SECUENCIADOR { INK_GOOD } else { INK_PLAIN });
        s.text(rpc::nombre(t.0));
        s.with_ink(INK_ECHO);
        s.text(b" (0x");
        s.hex(t.0 as u64, 4);
        s.text(b") x");
        s.dec(t.1 as u64);
        s.with_ink(INK_PLAIN);
    }
    if r.n_tipos > 0 {
        s.byte(b'\n');
    }
    campo(s, b"init");
    if r.init_done {
        s.with_ink(INK_GOOD);
        s.text(b"GSP_INIT_DONE YA esta en la cola: el GSP-RM acabo de arrancar\n");
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"sin GSP_INIT_DONE todavia: el GSP espera a que se le conteste (L0c4b)\n");
    }
    s.with_ink(INK_PLAIN);
    super::datos::anotar(b"gpu gsp mensajes", r.mensajes as u64, b"");
}
