//! **`gpu computo`: M5d S1..S3, EL PRIMER TRABAJO DEL MOTOR GRAFICO.** Tras el
//! contexto de oro (VISTO en el metal el 24-09 a las 15:51): AMPERE_COMPUTE_B
//! en el canal de GR0, su ficha, y un trabajo que solo el GR puede pagar
//! (`bmo_gpu_ga10x::computo`).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     dos RPC de hasta 5 s y una espera de hasta 100 ms
//!
//! El timbre sale de la TABLA de aparatos (la lista de GR0 << 16 | chid 2),
//! como la copia; la ficha del RM queda en la fila y es el respaldo.

use bmo_gpu_ga10x::canal::GR;
use bmo_gpu_ga10x::computo;
use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL};
use bmo_gpu_ga10x::copia;
use bmo_gpu_ga10x::objeto::{self, CABECERA_ALLOC};
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::gspsalud::{controlar, Contestada};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

#[derive(Clone, Copy)]
struct Pedido {
    r: objeto::Respuesta,
    resultado: u32,
    espera_us: u64,
    numero: u32,
}

#[derive(Clone, Copy, Default)]
struct Computo {
    pedido: Option<Result<Pedido, u32>>,
    ficha: Option<Result<Contestada, u32>>,
    /// La lista de GR0 segun la tabla de aparatos, si contesto.
    lista: Option<u32>,
    trabajo: Option<Result<u64, u32>>,
    /// Lo que se escribio en el timbre, y si salio de la tabla.
    timbre: Option<(u32, bool)>,
}

static mut ESTADO: Option<Computo> = None;

fn estado() -> Computo {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ESTADO) }.unwrap_or_default()
}

fn con(f: impl FnOnce(&mut Computo)) {
    // SAFETY: como `estado`.
    f(unsafe { (*core::ptr::addr_of_mut!(ESTADO)).get_or_insert_with(Computo::default) })
}

/// El RM contesto, pero NO creo AMPERE_COMPUTE_B.
pub(crate) const NO_COMPUTO_NEGADO: u32 = 0x137;
/// Sin ficha del canal de GR0 ni lista de GR0 en la tabla: no se toca el timbre.
pub(crate) const NO_TRABAJO_SIN_FICHA: u32 = 0x138;
/// El timbre sono pero el GR no pago el semaforo (la fila `gr trabajo`).
pub(crate) const NO_TRABAJO_MAL: u32 = 0x139;

fn pedido_bien(p: &Option<Result<Pedido, u32>>) -> bool {
    matches!(p, Some(Ok(p)) if p.r.estado == 0 && p.resultado == 0)
}

/// **S1: AMPERE_COMPUTE_B en el canal de GR0.** `Ok(su asa)`.
pub(crate) fn pedir() -> Result<u64, u32> {
    let r = (|| {
        let numero = (bmo::iommu_orden(bmo::IOMMU_OP_GSP_COMPUTO)? >> 32) as u32;
        let mut d = [0u8; CABECERA_ALLOC];
        let (m, espera_us) = esperar(objeto::GSP_RM_ALLOC, &mut d, &mut Otros::default())?;
        let r = objeto::leer(&d).ok_or(NO_COMPUTO_NEGADO)?;
        Ok(Pedido { r, resultado: m.resultado, espera_us, numero })
    })();
    con(|c| {
        c.pedido = Some(r);
        c.trabajo = None;
    });
    if pedido_bien(&Some(r)) {
        Ok(computo::COMPUTO as u64)
    } else {
        Err(r.err().unwrap_or(NO_COMPUTO_NEGADO))
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn pedido() -> bool {
    pedido_bien(&estado().pedido)
}

/// **S2: la ficha del canal de GR0**, y la lista de GR0 de la tabla.
pub(crate) fn ficha() -> Result<u64, u32> {
    let f = controlar(Control::FichaGr, &mut [0u8; CABECERA_CONTROL + 4]);
    let lista = (|| {
        let mut d = [0u8; CABECERA_CONTROL + control::DISPOSITIVOS_MEDIDA];
        let c = controlar(Control::Dispositivos, &mut d).ok()?;
        if !c.bien() {
            return None;
        }
        let (tabla, n) = control::dispositivos(&d);
        tabla[..n].iter().find(|x| x.tipo == GR.motor).map(|x| x.lista)
    })();
    con(|c| {
        c.ficha = Some(f);
        c.lista = lista;
    });
    let f = f?;
    if !f.bien() {
        return Err(super::gspsalud::NO_CONTROL_NEGADO);
    }
    Ok(f.r.valor as u64)
}

/// Lo pregunta `save mode`.
pub(crate) fn ficha_leida() -> bool {
    matches!(estado().ficha, Some(Ok(f)) if f.bien())
}

/// **S3: el primer trabajo del GR**: el kernel prepara el tramo, pone GP_PUT
/// del canal de GR0, toca el timbre y espera el semaforo de informe.
pub(crate) fn trabajar() -> Result<u64, u32> {
    let e = estado();
    let de_la_ficha = match e.ficha {
        Some(Ok(f)) if f.bien() => Some(f.r.valor),
        _ => None,
    };
    let de_la_tabla = e.lista.map(|l| copia::timbre_de(l, GR.chid));
    let valor = de_la_tabla.or(de_la_ficha);
    let r = match valor {
        Some(v) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_TRABAJO_GR, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    };
    con(|c| {
        c.trabajo = Some(r);
        c.timbre = valor.map(|v| (v, de_la_tabla.is_some()));
    });
    match r {
        Ok(v) if computo::sano(v) => Ok(v),
        Ok(_) => Err(NO_TRABAJO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn trabajado() -> bool {
    matches!(estado().trabajo, Some(Ok(v)) if computo::sano(v))
}

/// `gpu computo`: S1, S2 y S3, parando en lo primero que no sale.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "pidiendo el computo y mandandole trabajo al motor grafico", INK_DIM);
    let mut r = if pedido() { Ok(0) } else { pedir() };
    if r.is_ok() && !ficha_leida() {
        r = ficha();
    }
    if r.is_ok() && !trabajado() {
        r = trabajar();
    }
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  EL MOTOR GRAFICO CORRIO NUESTRO TRABAJO: pago el semaforo de informe (S3 de M5d)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el motor grafico no corrio el trabajo: mira las filas `computo`, `ficha gr` y `gr trabajo`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "computo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn no(s: &mut Output, m: u32) {
    s.with_ink(INK_ERR);
    s.text(b"NO: ");
    s.text(super::iommu::motivo(m));
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// **Las filas `computo`, `ficha gr` y `gr trabajo`**, si se pidieron.
pub(crate) fn fila(s: &mut Output) {
    let c = estado();
    if let Some(r) = c.pedido {
        campo(s, b"computo");
        match r {
            Err(m) => no(s, m),
            Ok(p) => {
                s.with_ink(if pedido_bien(&Some(r)) { INK_GOOD } else { INK_ERR });
                s.text(b"0x");
                s.hex(computo::COMPUTO as u64, 8);
                s.text(b" (clase 0xC7C0, en el canal de GR0): ");
                s.text(objeto::estado(p.r.estado));
                s.with_ink(INK_ECHO);
                s.text(b"   GSP_RM_ALLOC en ");
                s.dec(p.espera_us / 1000);
                s.text(b" ms (numero ");
                s.dec(p.numero as u64);
                s.byte(b')');
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.ficha {
        campo(s, b"ficha gr");
        match r {
            Err(m) => no(s, m),
            Ok(f) => {
                s.with_ink(if f.bien() { INK_GOOD } else { INK_ERR });
                s.text(b"0x");
                s.hex(f.r.valor as u64, 8);
                s.with_ink(INK_ECHO);
                match c.lista {
                    Some(l) => {
                        s.text(b"; GR0 en la lista ");
                        s.dec(l as u64);
                        s.text(b" de la tabla");
                    }
                    None => s.text(b"; la tabla no dijo la lista de GR0"),
                }
                s.text(b"   GET_WORK_SUBMIT_TOKEN en ");
                s.dec(f.espera_us / 1000);
                s.text(b" ms (numero ");
                s.dec(f.numero as u64);
                s.byte(b')');
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.trabajo {
        campo(s, b"gr trabajo");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (semaforo, gp_get, pagado, lanzado, us) = computo::desempaquetar(v);
                if pagado {
                    s.with_ink(INK_GOOD);
                    s.text(b"EL MOTOR GRAFICO CORRIO: semaforo de informe PAGADO (0x");
                    s.hex(semaforo as u64, 8);
                    s.text(b")");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el GR NO pago: semaforo 0x" as &[u8] } else { b"no se lanzo (la MMU o el USERD no quedaron): semaforo 0x" });
                    s.hex(semaforo as u64, 8);
                }
                s.with_ink(INK_ECHO);
                s.text(b", GP_GET ");
                s.dec(gp_get as u64);
                if let Some((t, tabla)) = c.timbre {
                    s.text(b"; timbre 0x");
                    s.hex(t as u64, 8);
                    s.text(if tabla { b" (la lista de GR0 segun la tabla)" as &[u8] } else { b" (la ficha del RM)" });
                }
                s.text(b"   en ");
                s.dec(us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
}
