//! **UN CUADRO DE 2x2 en la 3060 de mentira** (L2, 14-10): los cuatro hilos
//! de un cuadro de pixeles, cada uno con SUS registros, que corren juntos
//! hasta cada SHFL; ahi cada uno lee el registro de quien le toca
//! (`bmo_sm86::olas::de_quien`, la semantica del manual de PTX) y siguen.
//! Es lo que hace la casa con sus carriles (`prometeo::carriles`): un SHFL
//! es una operacion de ola.
//!
//! [!] El hilo `k` del cuadro es el `k` de la casa (0 1 arriba, 2 3 abajo):
//! que la 3060 los ponga asi en la pantalla lo mide la LENTE
//! (`gpu verrano derivadas`), no esto.

use crate::simula::{correr_desde, Maquina, NoSimula, Parada};

/// Lo que un hilo lleva entre parada y parada.
#[derive(Clone, Copy)]
pub struct Hilo {
    pub r: [u32; 256],
    pub p: [bool; 7],
    pc: usize,
    fin: bool,
    pub matado: bool,
}

/// **Correr un cuadro**: `preparar(k, m)` pone lo que el hilo `k` lleva al
/// empezar (sus precargas) y lo que lee (sus texturas): se llama en cada
/// tramo. Devuelve los cuatro hilos al acabar.
pub fn correr_cuadro(codigo: &[(u64, u64)], bancos: [&[u8]; 8], preparar: impl Fn(usize, &mut Maquina)) -> Result<[Hilo; 4], NoSimula> {
    let mut h = [Hilo { r: [0; 256], p: [false; 7], pc: 0, fin: false, matado: false }; 4];
    for (k, x) in h.iter_mut().enumerate() {
        let mut m = Maquina::nueva(bancos);
        preparar(k, &mut m);
        (x.r, x.p) = (m.r, m.p);
    }
    loop {
        // Cada uno hasta su SHFL o su fin.
        for (k, x) in h.iter_mut().enumerate() {
            if x.fin {
                continue;
            }
            let mut m = Maquina::nueva(bancos);
            preparar(k, &mut m);
            (m.r, m.p, m.en_cuadro) = (x.r, x.p, true);
            let parada = correr_desde(codigo, &mut m, &mut x.pc)?;
            (x.r, x.p, x.matado) = (m.r, m.p, m.matado);
            match parada {
                Parada::Fin(_) => x.fin = true,
                Parada::Cuadro(_) => {}
                Parada::Barrera(_) => return Err(NoSimula::Instruccion(x.pc)),
            }
        }
        let parados: [bool; 4] = core::array::from_fn(|k| !h[k].fin);
        if !parados.iter().any(|&p| p) {
            return Ok(h);
        }
        // Todos los que siguen, en el MISMO SHFL (un cuadro que se separa no
        // lo sabe esto: la casa lo rehace en el interprete).
        let pc = h.iter().find(|x| !x.fin).map(|x| x.pc).unwrap_or(0);
        if h.iter().any(|x| !x.fin && x.pc != pc) {
            return Err(NoSimula::Instruccion(pc));
        }
        let (lo, _) = codigo[pc];
        let (rd, ra) = ((lo >> 16 & 0xFF) as usize, (lo >> 24 & 0xFF) as usize);
        let (c, lane) = ((lo >> 40 & 0x1FFF) as u32, (lo >> 53 & 0x1F) as u32);
        let modo = match lo >> 58 & 3 {
            0 => bmo_sm86::olas::Shfl::Idx,
            1 => bmo_sm86::olas::Shfl::Up,
            2 => bmo_sm86::olas::Shfl::Down,
            _ => bmo_sm86::olas::Shfl::Bfly,
        };
        // Todos los valores antes de escribir ninguno (uno lee el `ra` de otro;
        // uno que ya acabo deja el suyo, como en la casa).
        let leido: [u32; 4] = core::array::from_fn(|k| {
            let j = bmo_sm86::olas::de_quien(k as u32, modo, lane, c) as usize;
            let de = if j < 4 { j } else { k };
            if ra == 255 { 0 } else { h[de].r[ra] }
        });
        for (k, x) in h.iter_mut().enumerate() {
            if !x.fin {
                if rd != 255 {
                    x.r[rd] = leido[k];
                }
                x.pc += 1;
            }
        }
    }
}
