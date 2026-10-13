//! **UN GRUPO DE COMPUTO en la 3060 de mentira** (LI2f, 12-10): los hilos de
//! un grupo (`[numthreads]`), cada uno con SUS registros y SUS ids (S2R), la
//! memoria COMPARTIDA del grupo y la GLOBAL de quien llama, y las barreras
//! (BAR.SYNC): cada hilo corre hasta su barrera o su EXIT, en orden, y otra
//! ronda mientras alguno se haya parado en una. Es el orden de la casa
//! (`Programa::despachar`: hasta la barrera, hilo tras hilo).
//!
//! [!] Lo que es MODELO: la 3060 corre los hilos de 32 en 32 y en el orden
//! que quiere. Lo que no depende del orden (cada hilo con lo suyo, y lo que
//! lee tras una barrera) da lo mismo; lo de los atomicos que se devuelve (lo
//! de antes) SI depende, en la 3060 y en D3D12: eso lo dira el metal.

use alloc::vec::Vec;

use crate::simula::{correr_desde, Maquina, Memoria, NoSimula, Parada};

/// Lo que un hilo lleva entre ronda y ronda.
struct Hilo {
    r: [u32; 256],
    p: [bool; 7],
    pc: usize,
    fin: bool,
}

/// **Correr un grupo** `ctaid` de `hilos`: `preparar(t, m)` pone lo que el
/// hilo `t` lleva al empezar (sus precargas: los registros del kernel).
/// Devuelve las instrucciones corridas de todos.
pub fn correr_grupo(codigo: &[(u64, u64)], hilos: [u32; 3], ctaid: [u32; 3], bancos: [&[u8]; 8], compartida: &mut [u8], global: &mut dyn Memoria, preparar: impl Fn(u32, &mut Maquina)) -> Result<usize, NoSimula> {
    let n = (hilos[0] * hilos[1] * hilos[2]) as usize;
    let mut estado: Vec<Hilo> = Vec::with_capacity(n);
    for t in 0..n as u32 {
        let mut m = Maquina::nueva(bancos);
        preparar(t, &mut m);
        estado.push(Hilo { r: m.r, p: m.p, pc: 0, fin: false });
    }
    let mut pasos = 0usize;
    loop {
        let mut alguno_espera = false;
        for (t, h) in estado.iter_mut().enumerate() {
            if h.fin {
                continue;
            }
            let t = t as u32;
            let mut m = Maquina::nueva(bancos);
            m.r = h.r;
            m.p = h.p;
            m.tid = [t % hilos[0], (t / hilos[0]) % hilos[1], t / (hilos[0] * hilos[1])];
            m.ctaid = ctaid;
            m.global = Some(&mut *global);
            m.compartida = Some(&mut *compartida);
            let parada = correr_desde(codigo, &mut m, &mut h.pc)?;
            h.r = m.r;
            h.p = m.p;
            match parada {
                Parada::Fin(k) => {
                    h.fin = true;
                    pasos += k;
                }
                Parada::Barrera(k) => {
                    alguno_espera = true;
                    pasos += k;
                }
            }
        }
        if !alguno_espera {
            return Ok(pasos);
        }
    }
}
