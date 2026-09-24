//! **`gpu vaciar`: L0c4b1, DEVOLVERLE AL GSP SU COLA.** Lee los mensajes que
//! el GSP CUENTA sin pedir nada --sus NOCAT, sus `LIBOS_PRINT`, su registro de
//! errores--, dice lo que traen en claro, y mueve el `readPtr` de la CPU detras
//! de cada uno: el hueco vuelve al GSP y el GSP puede seguir hablando.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: unas decenas de
//!                     ms por mensaje y hasta 10 s esperando a que diga mas
//!
//! # Por que (el metal, 24-09 07:34)
//!
//! `gpu cola` encontro 62 mensajes, TODOS `GSP_POST_NOCAT_RECORD`, secuencia
//! 0..61: la cola LLENA (63 huecos, uno siempre libre). Lo que el GSP-RM quiera
//! pedir despues --el secuenciador, `GSP_INIT_DONE`-- no cabe hasta que la CPU
//! consuma. nova-core hace lo mismo: lo que no espera, lo lee y lo deja pasar.
//!
//! # Lo que NO hace
//!
//! Contestar. El primer mensaje que PIDE algo se queda en la cola, sin mover
//! el puntero, para L0c4b2 -- y la fila `pide` dice cual es.

use bmo_gpu_ga10x::rpc::{self, Mensaje, CABECERA};
use bmo_userland as bmo;

use super::gspcola::{cabecera, cola, mem, suma, ESCRITO, LEIDO_CPU, NO_COLA_SIN_GSP, PAGINA, PAGINAS};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Tope de mensajes por vuelta: si el GSP no para de contar, se para aqui.
const MAX_MENSAJES: u32 = 1024;
/// Tipos distintos que se cuentan.
const MAX_TIPOS: usize = 6;
/// Textos distintos que se guardan, y lo que mide cada uno como mucho.
const MAX_TEXTOS: usize = 6;
const LARGO_TEXTO: usize = 48;
/// De los datos de cada mensaje, lo que se mira buscando texto.
const MIRADO: usize = 2048;
/// Lo consumido, crudo, a disco: hasta 128 paginas.
const CRUDO_BYTES: u64 = 128 * PAGINA;
const RUTA: &[u8] = b"datos/gspnocat.bin";

#[derive(Clone, Copy)]
struct Resumen {
    consumidos: u32,
    tipos: [(u32, u32); MAX_TIPOS],
    n_tipos: usize,
    textos: [([u8; LARGO_TEXTO], usize); MAX_TEXTOS],
    n_textos: usize,
    /// Textos distintos que ya no cupieron.
    sobran: u32,
    /// El primero que pide algo: `(funcion, numero)`, y se quedo en la cola.
    pide: Option<(u32, u32)>,
    /// Donde lee ahora la CPU.
    leido: u32,
    /// Sin forma o con la suma mal: se paro ahi, sin consumirlo.
    malo: bool,
    /// El kernel dijo que no al mover el puntero.
    negado: u32,
    guardada: bool,
}

static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

/// Motivo del escritorio (`gspcola.rs` va hasta 0x11C).
pub(crate) const NO_VACIAR_MAL: u32 = 0x11D;

fn contar(r: &mut Resumen, funcion: u32) {
    match r.tipos[..r.n_tipos].iter_mut().find(|t| t.0 == funcion) {
        Some(t) => t.1 += 1,
        None if r.n_tipos < MAX_TIPOS => {
            r.tipos[r.n_tipos] = (funcion, 1);
            r.n_tipos += 1;
        }
        None => {}
    }
}

/// Los textos en claro de los datos del mensaje en `pagina`, sin repetir.
fn apuntar_textos(r: &mut Resumen, pagina: u64, m: &Mensaje) {
    let mut d = [0u8; MIRADO];
    let n = (m.largo as usize).min(MIRADO);
    let mut o = 0;
    while o < n {
        let w = cola(pagina, (CABECERA + o) as u64).to_le_bytes();
        let k = (n - o).min(8);
        d[o..o + k].copy_from_slice(&w[..k]);
        o += 8;
    }
    let mut t = [(0usize, 0usize); 8];
    let hay = rpc::textos(&d[..n], &mut t);
    for &(a, b) in &t[..hay] {
        let dicho = &d[a..b.min(a + LARGO_TEXTO)];
        if r.textos[..r.n_textos].iter().any(|x| &x.0[..x.1] == dicho) {
            continue;
        }
        if r.n_textos < MAX_TEXTOS {
            r.textos[r.n_textos].0[..dicho.len()].copy_from_slice(dicho);
            r.textos[r.n_textos].1 = dicho.len();
            r.n_textos += 1;
        } else {
            r.sobran += 1;
        }
    }
}

/// **Esperar a que el GSP diga algo mas** que `hasta`: `true` si su
/// `writePtr` se movio antes de 3 s, o antes de `fin`.
fn esperar_mas(hasta: u32, fin: u64, hz: u64) -> bool {
    let tope = (bmo::ciclos() + hz * 3).min(fin);
    loop {
        if (mem(ESCRITO) & 0xFFFF_FFFF) as u32 != hasta {
            return true;
        }
        if bmo::ciclos() >= tope {
            return false;
        }
        bmo::yield_screen();
    }
}

/// **Vaciar lo que se puede consumir sin contestar.** `Ok(consumidos)`.
pub(crate) fn vaciar() -> Result<u64, u32> {
    if bmo::info(bmo::INFO_GPU_DESPIERTO) & bmo::DESPIERTO_VISTO == 0 {
        return Err(NO_COLA_SIN_GSP);
    }
    let crudo = bmo::Memoria::request(CRUDO_BYTES);
    let mut guardado = 0u64;
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let fin = bmo::ciclos() + hz * 10;
    let mut r = Resumen {
        consumidos: 0,
        tipos: [(0, 0); MAX_TIPOS],
        n_tipos: 0,
        textos: [([0; LARGO_TEXTO], 0); MAX_TEXTOS],
        n_textos: 0,
        sobran: 0,
        pide: None,
        leido: 0,
        malo: false,
        negado: 0,
        guardada: false,
    };
    let mut p = (mem(LEIDO_CPU) & 0xFFFF_FFFF) % PAGINAS;
    'fuera: loop {
        let escrito = (mem(ESCRITO) & 0xFFFF_FFFF) as u32;
        while p != escrito as u64 % PAGINAS {
            let m = Mensaje::de(&cabecera(p));
            if !m.bien_formado() || suma(p, &m) != 0 {
                r.malo = true;
                break 'fuera;
            }
            if !rpc::informativo(m.funcion) {
                r.pide = Some((m.funcion, m.numero));
                break 'fuera;
            }
            contar(&mut r, m.funcion);
            apuntar_textos(&mut r, p, &m);
            // Crudo al bloque ANTES de devolver el hueco: despues es del GSP.
            if let Some(b) = &crudo {
                let largo = m.paginas as u64 * PAGINA;
                if guardado + largo <= CRUDO_BYTES {
                    // SAFETY: el bloque mide CRUDO_BYTES, es de este proceso,
                    // y lo escrito cabe (mirado justo arriba).
                    let s = unsafe { core::slice::from_raw_parts_mut(b.base().add(guardado as usize), largo as usize) };
                    for o in (0..largo).step_by(8) {
                        s[o as usize..o as usize + 8].copy_from_slice(&cola(p, o).to_le_bytes());
                    }
                    guardado += largo;
                }
            }
            let siguiente = (p + m.paginas as u64) % PAGINAS;
            if let Err(e) = bmo::iommu_orden_con(bmo::IOMMU_OP_GSP_LEIDO, siguiente) {
                r.negado = e;
                break 'fuera;
            }
            p = siguiente;
            r.consumidos += 1;
            if r.consumidos >= MAX_MENSAJES {
                break 'fuera;
            }
        }
        if bmo::ciclos() >= fin || !esperar_mas(escrito, fin, hz) {
            break;
        }
    }
    r.leido = p as u32;
    if let (Some(b), true) = (&crudo, guardado > 0) {
        r.guardada = match bmo::Archivo::create(RUTA) {
            Ok(a) => a.escribir_de(b, 0, guardado) == guardado && a.close(),
            Err(_) => false,
        };
    }
    // SAFETY: como `resumen`.
    unsafe {
        *core::ptr::addr_of_mut!(RESUMEN) = Some(r);
    }
    if r.negado != 0 {
        return Err(r.negado);
    }
    if r.malo {
        return Err(NO_VACIAR_MAL);
    }
    Ok(r.consumidos as u64)
}

/// Lo pregunta `save mode`: se vacio sin que nada saliera mal.
pub(crate) fn vaciada() -> bool {
    resumen().map_or(false, |r| !r.malo && r.negado == 0)
}

/// `gpu vaciar`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "vaciando la cola del GSP", INK_DIM);
    let r = vaciar();
    let g = &mut dsk.out.grid;
    match r {
        Ok(n) => {
            g.with_ink(INK_GOOD);
            g.text(b"  VACIADA: ");
            g.dec(n);
            g.text(b" mensajes que solo contaban, consumidos; sus huecos, devueltos al GSP\n");
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
    paint_status(p, &dsk.run_box, "vaciar", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Las filas de L0c4b1**, si se vacio.
pub(crate) fn fila(s: &mut Output) {
    let Some(r) = resumen() else { return };
    campo(s, b"vacia");
    s.with_ink(if r.malo || r.negado != 0 { INK_ERR } else { INK_GOOD });
    s.dec(r.consumidos as u64);
    s.text(b" consumidos");
    s.with_ink(INK_PLAIN);
    s.text(b"; la CPU lee ahora en la pagina ");
    s.dec(r.leido as u64);
    for (i, t) in r.tipos[..r.n_tipos].iter().enumerate() {
        s.text(if i == 0 { b": " as &[u8] } else { b", " });
        s.text(rpc::nombre(t.0));
        s.text(b" x");
        s.dec(t.1 as u64);
    }
    if r.consumidos > 0 {
        s.with_ink(INK_ECHO);
        s.text(if r.guardada { b"   -> datos/gspnocat.bin" as &[u8] } else { b"   (NO se pudo guardar)" });
    }
    s.byte(b'\n');
    // Lo que traian en claro, de dos en dos.
    for (i, t) in r.textos[..r.n_textos].iter().enumerate() {
        if i % 2 == 0 {
            if i > 0 {
                s.byte(b'\n');
            }
            campo(s, b"nocat");
        } else {
            s.text(b",  ");
        }
        s.with_ink(INK_ECHO);
        s.byte(b'"');
        s.text(&t.0[..t.1]);
        s.byte(b'"');
        s.with_ink(INK_PLAIN);
    }
    if r.sobran > 0 {
        s.text(b"  y ");
        s.dec(r.sobran as u64);
        s.text(b" textos mas");
    }
    if r.n_textos > 0 {
        s.byte(b'\n');
    }
    campo(s, b"pide");
    if r.malo {
        s.with_ink(INK_ERR);
        s.text(b"UN MENSAJE SIN FORMA o con la suma mal: se paro ahi, sin consumirlo\n");
    } else if r.negado != 0 {
        s.with_ink(INK_ERR);
        s.text(b"el kernel no movio el puntero: ");
        s.text(super::iommu::motivo(r.negado));
        s.byte(b'\n');
    } else if let Some((f, n)) = r.pide {
        s.with_ink(INK_GOOD);
        s.text(rpc::nombre(f));
        s.with_ink(INK_ECHO);
        s.text(b" (0x");
        s.hex(f as u64, 4);
        s.text(b") numero ");
        s.dec(n as u64);
        s.with_ink(INK_PLAIN);
        s.text(if f == rpc::INIT_DONE {
            b": el GSP-RM ACABO DE ARRANCAR; se queda en la cola\n" as &[u8]
        } else {
            b": PIDE respuesta; se queda en la cola para L0c4b2\n"
        });
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"nada todavia: en 10 s el GSP no dijo nada que pida respuesta\n");
    }
    s.with_ink(INK_PLAIN);
    super::datos::anotar(b"gpu gsp consumidos", r.consumidos as u64, b"");
}
