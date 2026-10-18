//! ** LB9 (18-10): EL COMPUTO en la CPU -- una gpu fn de TITAN++ que mira a
//! sus vecinas, un hilo detras de otro (la reserva, L32).
//!
//! ```text
//!    el codigo    el prologo (cada registro con su inicial) y el x86-64 de
//!                 `nativo_computo`: rdi = los registros del hilo, rsi = su
//!                 `Contexto`, rdx = el cbuffer (ninguno); devuelve 0
//!    el Contexto  los ids del hilo y una `Vista` por UAV (sus datos, sus
//!                 bytes, paso 4, sus elementos), por los desplazamientos de
//!                 `nativo_computo` (592 bytes)
//!    el contrato  el de una celda (`correr_con`) en CADA hilo, y escribe solo
//!                 en sus registros, su pila, su Contexto y el UAV del
//!                 resultado (el ultimo)
//! ```
//!
//! Una sola maquina del emulador para todos los hilos: su memoria es la de
//! los UAV, y cada hilo la ve como la dejo el anterior -- como en la CPU de
//! verdad, donde corren uno detras de otro.

use bmo_lower::emu::{run_acotado, Machine, MXCSR_REINICIO, STACK_TOP};
use bmo_prometeo::programa::{Op, Programa};
use bmo_prometeo::{Codigo, NoEmite};

use crate::{arnes, palabras, prologo, CONSERVA, NOMBRE, PASOS, PILA, REGISTROS};

/// Lo que mide el `Contexto` de `nativo_computo`, y donde empiezan sus UAV.
const CONTEXTO: usize = 80 + 2 * 8 * 32;
const C_UAV: usize = 80 + 8 * 32;
/// Los UAV que ve, como mucho.
const VISTAS: usize = 8;

/// Lo que esta tarjeta todavia no corre en un computo: lo de una celda que
/// no sabe (`no_sabe`), menos los hilos y los UAV de bufer -- sin barreras,
/// sin compartida, sin atomicos ni contadores.
pub fn no_sabe(p: &Programa) -> Option<(usize, &'static str)> {
    p.ops.iter().enumerate().find_map(|(k, op)| {
        let por_que = match op {
            Op::IdHilo { que, .. } if *que == 0 => return None,
            Op::LeeUav { modo: bmo_prometeo::bufer::Modo::Estructurado, u, .. } | Op::EscribeUav { modo: bmo_prometeo::bufer::Modo::Estructurado, u, .. } if (*u as usize) < VISTAS => return None,
            Op::IdHilo { .. } | Op::Barrera | Op::LeeCompartida { .. } | Op::EscribeCompartida { .. } | Op::AtomicoCompartido { .. } => "es de un grupo (sus ids, sus barreras, su memoria compartida), y los hilos de la CPU corren uno detras de otro",
            Op::LeeUav { .. } | Op::EscribeUav { .. } | Op::MedidasUav { .. } | Op::Contador { .. } | Op::Atomico { .. } => "toca un UAV que no es un bufer estructurado, o con atomicos, y eso todavia no tiene su prueba en esta tarjeta",
            Op::Entrada { .. } | Op::Salida { .. } => "un computo no tiene entradas ni salidas: lee y escribe sus UAV",
            otra => return crate::no_sabe(&Programa { ops: vec![otra.clone()], ..p.clone() }).map(|(_, por_que)| (k, por_que)),
        };
        Some((k, por_que))
    })
}

/// **Emite el computo** `p`: su prologo y su `nativo_computo`.
pub(crate) fn emitir(p: &Programa) -> Result<Codigo, NoEmite> {
    if let Some((op, por_que)) = no_sabe(p) {
        return Err(NoEmite::Limite { op, que: format!("{} todavia no corre esto en un computo", NOMBRE), por_que: por_que.into() });
    }
    if p.iniciales.len() > REGISTROS as usize {
        let op = p.ops.len().saturating_sub(1);
        return Err(NoEmite::Limite { op, que: format!("{} no tiene sitio para tantos valores", NOMBRE), por_que: format!("usa {} registros, y un cuerpo de la CPU guarda {} en la pila de quien lo llama", p.iniciales.len(), REGISTROS) });
    }
    let cuerpo = bmo_proton_x::nativo_computo::compilar(p).ok_or_else(|| NoEmite::Fallo { op: None, por_que: "su emisor (`nativo_computo`) no lo tradujo".into() })?;
    let mut bytes = prologo(p);
    bytes.extend_from_slice(&cuerpo);
    let mut c = Codigo { bytes, instrucciones: 0, registros: p.iniciales.len() as u32 };
    let mut uno = vec![vec![0u32; 1]; p.ranuras.uavs.len().max(1)];
    let pasos = correr(p, &c, 1, &mut uno).map_err(|por_que| NoEmite::Fallo { op: None, por_que: format!("su codigo no cumple el contrato de la llamada: {}", por_que) })?;
    c.instrucciones = pasos as usize;
    Ok(c)
}

/// **Juzga el computo**: sus techos, y un grupo corrido con el contrato mirado.
pub(crate) fn juzgar(p: &Programa, c: &Codigo) -> Result<(), String> {
    if c.bytes.is_empty() {
        return Err("un cuerpo sin bytes".into());
    }
    if c.registros > REGISTROS {
        return Err(format!("{} registros, y un cuerpo de la CPU guarda {}", c.registros, REGISTROS));
    }
    let mut uno = vec![vec![0u32; 1]; p.ranuras.uavs.len().max(1)];
    correr(p, c, 1, &mut uno).map(|_| ())
}

/// **Corre el computo** `c` de `p`, `grupos` grupos de sus hilos, sobre
/// `uavs`, con el contrato mirado en cada hilo. Devuelve las instrucciones
/// que corrio el primero.
pub(crate) fn correr(p: &Programa, c: &Codigo, grupos: u32, uavs: &mut [Vec<u32>]) -> Result<u64, String> {
    if uavs.len() > VISTAS {
        return Err(format!("{} UAV, y un computo de la CPU ve {}", uavs.len(), VISTAS));
    }
    let hilos = p.computo.hilos;
    if hilos[1] != 1 || hilos[2] != 1 {
        return Err("un computo de la CPU va en x: sus hilos en y y en z, 1".into());
    }
    let mut m = Machine::new(arnes(&c.bytes));
    let n_regs = (c.registros as usize).max(1);
    let regs = m.load_data(&palabras(n_regs, core::iter::empty()));
    let datos: Vec<(u64, usize)> = uavs.iter().map(|u| (m.load_data(&palabras(u.len().max(1), u.iter().copied())), u.len())).collect();
    let mut ctx_bytes = vec![0u8; CONTEXTO];
    for (k, &(d, n)) in datos.iter().enumerate() {
        let v = C_UAV + 32 * k;
        ctx_bytes[v..v + 8].copy_from_slice(&d.to_le_bytes());
        ctx_bytes[v + 8..v + 16].copy_from_slice(&(4 * n as u64).to_le_bytes());
        ctx_bytes[v + 16..v + 20].copy_from_slice(&4u32.to_le_bytes());
        ctx_bytes[v + 20..v + 24].copy_from_slice(&(n as u32).to_le_bytes());
    }
    let ctx = m.load_data(&ctx_bytes);
    let (salida, n_salida) = *datos.last().ok_or("un computo sin UAV de resultado")?;
    let dentro = |a: u64, desde: u64, n: usize| a >= desde && a < desde + n as u64;
    let pila = STACK_TOP - PILA;
    let mut primero = None;
    for t in 0..grupos * hilos[0] {
        let par = |a: u32, b: u32| a as u64 | (b as u64) << 32;
        let (g, en) = (t / hilos[0], t % hilos[0]);
        // ids: despacho (0..3), grupo (3..6), en el grupo (6..9), indice (9); reanudar.
        for (o, v) in [(0, par(t, 0)), (8, par(0, g)), (16, par(0, 0)), (24, par(en, 0)), (32, par(0, en)), (40, 0)] {
            m.pon_u64(ctx + o, v);
        }
        m.regs[7] = regs; // rdi
        m.regs[6] = ctx; // rsi
        m.regs[2] = 0; // rdx: sin cbuffer
        m.regs[4] = STACK_TOP;
        for (r, _, v) in CONSERVA {
            m.regs[r] = v;
        }
        m.rip = 0;
        m.escritas = Some(Vec::new());
        let antes = m.pasos;
        let (mm, acabo) = run_acotado(m, PASOS + 2);
        m = mm;
        if !acabo {
            return Err(format!("el hilo {} no vuelve: corrio mas de {} instrucciones", t, PASOS));
        }
        if m.exited || !m.syscalls.is_empty() {
            return Err("llama al kernel: un cuerpo de la CPU es una cuenta, sin puertas".into());
        }
        if m.rip != m.code.len() {
            return Err("no vuelve a quien lo llamo".into());
        }
        for (r, nombre, v) in CONSERVA {
            if m.regs[r] != v {
                return Err(format!("no conserva {} (System V: quien llama lo da por guardado)", nombre));
            }
        }
        if m.regs[4] != STACK_TOP {
            return Err("no deja la pila como estaba".into());
        }
        if m.mxcsr != MXCSR_REINICIO {
            return Err(format!("deja el MXCSR en {:#x}, y quien llama lo tenia en {:#x}", m.mxcsr, MXCSR_REINICIO));
        }
        if m.regs[0] as u32 != 0 {
            return Err(format!("el hilo {} vuelve con eax = {}, y su puerta espera 0 (acabo)", t, m.regs[0] as u32));
        }
        for &a in m.escritas.as_deref().unwrap_or(&[]) {
            if !(dentro(a, regs, 4 * n_regs) || dentro(a, ctx, CONTEXTO) || dentro(a, salida, 4 * n_salida) || (a >= pila && a < STACK_TOP)) {
                return Err(format!("escribe fuera de sus registros, su pila, su Contexto y su resultado (en {:#x})", a));
            }
        }
        primero.get_or_insert(m.pasos - antes - 2);
    }
    let palabra = |a: u64| (0..4).fold(0u32, |w, i| w | (m.read_u8_pub(a + i) as u32) << (8 * i));
    if let Some(out) = uavs.last_mut() {
        for (i, x) in out.iter_mut().enumerate() {
            *x = palabra(salida + 4 * i as u64);
        }
    }
    Ok(primero.unwrap_or(0))
}
