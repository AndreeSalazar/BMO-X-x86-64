//! **El Dispatch del computo TRADUCIDO** (de `nativo_computo.rs`, partido
//! por L6a el 06-10): los hilos de cada grupo llamando a la funcion
//! traducida, barrera a barrera, y (A10, 06-10) OLA a OLA: cada operacion de
//! ola (`Wave*`, `Quad*`) para el hilo, y aqui se resuelve con los carriles
//! de su ola de 32, en el MISMO orden que el interprete (`dxil::computo`,
//! `Programa::correr_ola`, `Pausa::orden`): asi da los mismos bits.
//!
//! capa: puro -- quien llama al codigo sellado es la casa (`llamar`)

use alloc::vec::Vec;

use crate::dxil::programa::{Op, Programa};
use crate::nativo_computo::{paradas, Contexto, Vista, BARRERA, OLA, VISTAS};

/// **Un Dispatch con la funcion traducida** (la de [`compilar`] sobre `p`):
/// lo mismo que [`Programa::despachar`], grupo a grupo y barrera a barrera,
/// con las cuentas en x86. Los SRV son los buferes del dibujo (por ranura) y
/// los UAV, los del Dispatch.
///
/// `llamar(regs, contexto, cb)` llama al codigo sellado: lo pone la casa,
/// que es quien lo sello y quien promete que es el de `compilar(p)` (este
/// crate es puro: sin `unsafe`). Los punteros son de esta funcion y viven lo
/// que ella.
pub fn despachar(p: &Programa, llamar: &mut dyn FnMut(*mut f32, *mut Contexto, *const u8) -> u32, grupos: [u32; 3], cb: &[u8], srv: &[Option<crate::bufer::Bufer>], uavs: &mut [Option<crate::bufer::Uav>], por_hilo: &mut [crate::nativo_llamadas::Llamadas]) -> u64 {
    let [hx, hy, hz] = p.computo.hilos;
    let n = (hx * hy * hz) as usize;
    if n == 0 {
        return 0;
    }
    // El cbuffer, con lo que lea: lo que falte, a cero (como el interprete).
    let filas = p.filas_cb as usize * 16;
    let mut relleno = Vec::new();
    let cb: &[u8] = if cb.len() >= filas.max(16) {
        cb
    } else {
        relleno.extend_from_slice(cb);
        relleno.resize(filas.max(16), 0);
        &relleno
    };
    let mut compartida = alloc::vec![0u32; p.computo.compartida.max(1) as usize];
    // X2: la matematica, y la medida del cbuffer. 06-10: las de cada hilo
    // del grupo, si quien despacha las da (las texturas y las ranuras de
    // UAV llamadas, `uavs_llamados`: cada hilo con SU textura elegida, que
    // una barrera puede caer entre elegirla y leerla); si no, unas sin
    // texturas, de todos.
    let llamadas = crate::nativo_llamadas::Llamadas::nuevas(cb.len());
    for l in por_hilo.iter_mut() {
        l.cb_bytes = cb.len() as u64;
    }
    let vista = |datos: *mut u8, bytes: usize, paso: u32, elementos: u32, contador: *mut u32| Vista { datos, bytes: bytes as u64, paso, elementos, contador };
    let mut c = Contexto {
        ids: [0; 10],
        reanudar: 0,
        n_compartida: p.computo.compartida,
        compartida: compartida.as_mut_ptr(),
        entradas: core::ptr::null(),
        salidas: core::ptr::null_mut(),
        llamadas: &llamadas,
        srv: [Vista::NULA; VISTAS],
        uav: [Vista::NULA; VISTAS],
    };
    for (k, b) in srv.iter().enumerate().take(VISTAS) {
        if let Some(b) = b {
            // Solo se lee: el puntero es *mut por la forma, no por el uso.
            c.srv[k] = vista(b.bytes.as_ptr() as *mut u8, b.bytes.len(), b.paso, b.elementos, core::ptr::null_mut());
        }
    }
    for (k, u) in uavs.iter_mut().enumerate().take(VISTAS) {
        if let Some(u) = u {
            let contador = u.contador.as_deref_mut().map_or(core::ptr::null_mut(), |c| c as *mut u32);
            c.uav[k] = vista(u.bytes.as_mut_ptr(), u.bytes.len(), u.paso, u.elementos, contador);
        }
    }
    // Cada hilo: sus registros (A10: y detras, las vueltas de sus bucles),
    // por donde sigue y como esta.
    let mut hilos: Vec<(Vec<f32>, u32, Estado)> = (0..n).map(|_| (Vec::new(), 0, Estado::Corre)).collect();
    let puntos = paradas(p, false);
    let abiertos = bucles_abiertos(p);
    let de_mas = if p.olas_propias() { crate::dxil::programa::ANIDADO_MAXIMO } else { 0 };
    let mut corridos = 0u64;
    for gz in 0..grupos[2] {
        for gy in 0..grupos[1] {
            for gx in 0..grupos[0] {
                compartida.fill(0);
                for h in hilos.iter_mut() {
                    h.0.clear();
                    h.0.extend_from_slice(&p.iniciales);
                    h.0.resize(p.iniciales.len() + de_mas, 0.0);
                    (h.1, h.2) = (0, Estado::Corre);
                }
                loop {
                    let mut alguno_espera = false;
                    // A10 (06-10): OLA a OLA, como el interprete
                    // (`dxil::computo`, `Programa::correr_ola`): sus carriles en
                    // orden hasta su proxima ola, la barrera o el fin; y la ola
                    // que va antes, resuelta con los que llegaron a ella.
                    for w in 0..n.div_ceil(CARRILES) {
                        let ola = w * CARRILES..((w + 1) * CARRILES).min(n);
                        loop {
                            for t in ola.clone() {
                                let (regs, reanudar, estado) = &mut hilos[t];
                                if *estado != Estado::Corre {
                                    continue;
                                }
                                let t = t as u32;
                                let en = [t % hx, (t / hx) % hy, t / (hx * hy)];
                                c.ids = [gx * hx + en[0], gy * hy + en[1], gz * hz + en[2], gx, gy, gz, en[0], en[1], en[2], t];
                                c.reanudar = *reanudar;
                                c.llamadas = por_hilo.get(t as usize).map_or(&llamadas as *const _, |l| l as *const _);
                                *estado = match llamar(regs.as_mut_ptr(), &mut c, cb.as_ptr()) {
                                    BARRERA => Estado::Barrera,
                                    OLA => Estado::Ola,
                                    _ => {
                                        corridos += 1;
                                        Estado::Fin
                                    }
                                };
                                *reanudar = c.reanudar;
                            }
                            if !resolver_ola(p, &puntos, &abiertos, &mut hilos[ola.clone()]) {
                                break;
                            }
                        }
                        for h in hilos[ola].iter_mut().filter(|h| h.2 == Estado::Barrera) {
                            h.2 = Estado::Corre;
                            alguno_espera = true;
                        }
                    }
                    if !alguno_espera {
                        break;
                    }
                }
            }
        }
    }
    corridos
}

/// Los carriles de una ola (A10): los de D3D12 en la 3060, 32.
const CARRILES: usize = crate::dxil::olas::CARRILES as usize;

/// Como esta un hilo del computo traducido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Estado {
    Corre,
    Ola,
    Barrera,
    Fin,
}

/// Por operacion: los bucles abiertos alrededor (su principio: la
/// operacion de detras de su `Bucle`, como `Pausa::bucles`).
fn bucles_abiertos(p: &Programa) -> Vec<Vec<usize>> {
    let mut pila: Vec<usize> = Vec::new();
    p.ops
        .iter()
        .enumerate()
        .map(|(k, o)| {
            let aqui = pila.clone();
            match o {
                Op::Bucle => pila.push(k + 1),
                Op::FinBucle => {
                    pila.pop();
                }
                _ => {}
            }
            aqui
        })
        .collect()
}

/// Una clave de [`orden`] prestada.
fn como(c: &(usize, Vec<usize>, Vec<u32>)) -> (usize, &[usize], &[u32]) {
    (c.0, c.1.as_slice(), c.2.as_slice())
}

/// **Quien va antes** (A10), lo mismo que `Pausa::orden` del interprete:
/// `(pc, bucles abiertos, sus vueltas)` de dos carriles parados.
fn orden(a: (usize, &[usize], &[u32]), b: (usize, &[usize], &[u32])) -> core::cmp::Ordering {
    use core::cmp::Ordering::{Greater, Less};
    let n = a.1.len().min(b.1.len());
    for k in 0..n {
        if a.1[k] != b.1[k] {
            return a.1[k].cmp(&b.1[k]);
        }
        if a.2[k] != b.2[k] {
            return a.2[k].cmp(&b.2[k]);
        }
    }
    if a.1.len() > n {
        return if b.0 < a.1[n] { Greater } else { Less };
    }
    if b.1.len() > n {
        return if a.0 < b.1[n] { Less } else { Greater };
    }
    a.0.cmp(&b.0)
}

/// **Resolver la ola que va antes** (A10): de los carriles parados en una
/// ola, el que va ANTES (`orden`: la operacion, y dentro de los bucles la
/// vuelta) y los que van con el son los activos; su operacion, con
/// `olas::hacer` (todos los resultados antes de escribir ninguno), y
/// siguen. `false` si ninguno esperaba en una ola. Lo mismo que
/// `Programa::correr_ola` del interprete.
fn resolver_ola(p: &Programa, puntos: &[Vec<usize>], abiertos: &[Vec<usize>], ola: &mut [(Vec<f32>, u32, Estado)]) -> bool {
    let base = p.iniciales.len();
    let clave = |h: &(Vec<f32>, u32, Estado)| -> Option<(usize, Vec<usize>, Vec<u32>)> {
        let op = puntos.get((h.1 as usize).checked_sub(1)?)?[0];
        let b = abiertos.get(op)?.clone();
        let v = (0..b.len()).map(|k| h.0.get(base + k).map_or(0, |x| x.to_bits())).collect();
        Some((op + 1, b, v))
    };
    let claves: Vec<Option<(usize, Vec<usize>, Vec<u32>)>> = ola.iter().map(|h| if h.2 == Estado::Ola { clave(h) } else { None }).collect();
    let mut primero: Option<usize> = None;
    for (k, c) in claves.iter().enumerate() {
        if let Some(c) = c {
            if primero.is_none_or(|p| orden(como(c), como(claves[p].as_ref().expect("con clave"))).is_lt()) {
                primero = Some(k);
            }
        }
    }
    let Some(pr) = primero else {
        return false;
    };
    let referencia = claves[pr].clone().expect("con clave");
    let Some(&Op::Ola { d, a, b, que }) = p.ops.get(referencia.0 - 1) else {
        ola.iter_mut().filter(|h| h.2 == Estado::Ola).for_each(|h| h.2 = Estado::Corre);
        return true;
    };
    let activos = claves.iter().enumerate().filter(|(_, c)| c.as_ref().is_some_and(|c| orden(como(c), como(&referencia)).is_eq())).fold(0u32, |m, (k, _)| m | 1 << k);
    let mut r = [[0u32; 4]; CARRILES];
    for k in (0..ola.len()).filter(|&k| activos >> k & 1 != 0) {
        let de = |j: usize| ola.get(j).unwrap_or(&ola[k]).0[a as usize].to_bits();
        r[k] = crate::dxil::olas::hacer(que, k, activos, de, ola[k].0[b as usize].to_bits());
    }
    for (k, h) in ola.iter_mut().enumerate().filter(|(k, _)| activos >> k & 1 != 0) {
        for (j, v) in r[k].into_iter().enumerate().take(crate::dxil::olas::anchura(que)) {
            h.0[d as usize + j] = f32::from_bits(v);
        }
        h.2 = Estado::Corre;
    }
    true
}

