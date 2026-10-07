//! **UN DIBUJO EN BANDAS: lo que se puede decir sin tocar memoria** (H4.3
//! de `PLAN_LOS_DOCE_DIRECTORES`, 07-10).
//!
//! [carril]  VERDE     numeros: que filas, que tijera, si se parte o no
//! [cuesta]  DATO -- una franja con filas de otra pinta dos veces lo mismo
//!
//! Lo que un dibujo cuesta en la CPU esta casi todo en los PIXELES. Partir la
//! imagen en franjas horizontales y dar una a cada nucleo es lo mas simple
//! que da el mismo resultado: cada franja es el MISMO lote con la tijera
//! recortada a sus filas, y la trama ya no pinta fuera de la tijera.
//!
//! ```text
//!    el que llama   limpia una vez (limpiar_rt / limpiar_z) y comprueba el
//!                   lote ([`antes`]); despues reparte n franjas (filas
//!                   pares: los cuadros de 2x2 no se parten)
//!    cada franja    el lote con su tijera, sin limpiezas ([`de_franja`])
//!    al volver      las cuentas se juntan ([`juntar`])
//! ```
//!
//! Este crate es puro: aqui esta TODO lo que decide, y [`en_orden`] (las
//! franjas una detras de otra, con el mismo `&mut`) es el contrato que el
//! banco compara con el dibujo entero. Repartirlas entre NUCLEOS (el destino
//! a trozos crudos, una casilla por franja) es de la casa:
//! `proton-x-casa/src/bandas.rs`.
//!
//! **Lo que no se parte** ([`se_parte`]): un dibujo con UAV (escribe donde
//! quiere: dos franjas escribirian lo mismo), con texturas dinamicas si
//! quien llama no garantiza que su busqueda aguanta varios a la vez, con un
//! de pixeles que usa olas PROPIAS (`WaveActiveCountBits`...: cuantos van en
//! cada ola depende de como se agrupan, y en franjas se agrupan de otra
//! forma -- el banco lo vio: el mismo RGB, otro alfa), o un destino
//! demasiado bajo. Esos van enteros, como siempre. Las DERIVADAS si se
//! parten: las franjas van en filas pares y un cuadro de 2x2 no se rompe.
//!
//! **Las cuentas de triangulos** (`dibujados`, `descartados`, `recortados`)
//! las hace cada franja con TODOS los triangulos: se toma la mayor, no la
//! suma. Las de pixeles se suman (cada pixel es de una franja).

use crate::lote::{Lote, NoDibuja};
use crate::trama::{self, Cuenta, Destino};

/// Lo minimo que mide una franja, en filas.
pub const FILAS_MINIMAS: u32 = 16;

/// Lo mas que se parte un dibujo (una mascara de 64 bits dice cuales
/// salieron).
pub const FRANJAS_MAXIMAS: u32 = 64;

/// **Quien pinta una franja**: el mismo contrato que `lote::Ejecutor`, pero
/// lo pueden llamar varios nucleos a la vez (por eso `Sync`).
pub type Franja<'a> = &'a (dyn Fn(&Lote, &mut Destino) -> Result<Cuenta, NoDibuja> + Sync);

/// **Se puede partir este dibujo en `n` franjas?** `dinamicas_seguras`: la
/// busqueda de texturas dinamicas del lote aguanta varios a la vez.
pub fn se_parte(l: &Lote, d: &Destino, n: u32, dinamicas_seguras: bool) -> bool {
    n >= 2
        && l.uavs.is_none()
        && (l.recursos.dinamicas.is_none() || dinamicas_seguras)
        && !l.enlace.ps.olas_propias()
        && d.alto >= 2 * FILAS_MINIMAS
}

/// Cuantas franjas de verdad para `n` pedidas en un destino de `alto`.
pub fn cuantas(n: u32, alto: u32) -> u32 {
    n.min(alto / FILAS_MINIMAS).clamp(1, FRANJAS_MAXIMAS)
}

/// **Las filas de la franja `k` de `n`** en un destino de `alto`: pares, sin
/// huecos ni solapes, la ultima hasta abajo.
pub fn filas(k: u32, n: u32, alto: u32) -> (u32, u32) {
    let paso = ((alto / n) & !1).max(2);
    let y0 = (k * paso).min(alto);
    let y1 = if k + 1 == n { alto } else { ((k + 1) * paso).min(alto) };
    (y0, y1)
}

/// La tijera de la franja: la del lote, cortada a sus filas (`None`: vacia,
/// la franja no pinta nada).
pub fn tijera(t: [i32; 4], y0: u32, y1: u32) -> Option<[i32; 4]> {
    let arriba = t[1].max(y0 as i32);
    let abajo = t[3].min(y1 as i32);
    (arriba < abajo && t[0] < t[2]).then_some([t[0], arriba, t[2], abajo])
}

/// **El lote de una franja**: el mismo, con su tijera y sin limpiezas (ya
/// las hizo quien reparte, [`antes`]).
pub fn de_franja<'a>(l: &Lote<'a>, t: [i32; 4]) -> Lote<'a> {
    Lote {
        enlace: l.enlace,
        entradas: l.entradas,
        vertices: l.vertices,
        paso: l.paso,
        ids: l.ids,
        topologia: l.topologia,
        cb: l.cb,
        reglas: trama::Reglas { tijera: t, ..l.reglas },
        limpiar_z: None,
        limpiar_rt: None,
        recursos: l.recursos,
        oclusion: l.oclusion,
        otros: l.otros,
        instancias: l.instancias,
        primera_instancia: l.primera_instancia,
        base_vertice: l.base_vertice,
        uavs: None,
    }
}

/// **Lo de una vez, antes de repartir**: las limpiezas apuntadas (las haria
/// cada franja sobre el destino ENTERO, encima de lo que pintan las otras) y
/// la comprobacion del lote (su NO sale aqui, no de una franja).
pub fn antes(l: &Lote, d: &mut Destino) -> Result<(), NoDibuja> {
    if let Some(p) = l.limpiar_rt {
        d.pixeles.fill(p);
    }
    if let (Some(b), Some(z)) = (l.limpiar_z, d.z.as_mut()) {
        z.fill(b);
    }
    l.comprobar().map(|_| ())
}

/// **Juntar las cuentas** de las franjas: triangulos, la mayor; pixeles, la
/// suma.
pub fn juntar(cuentas: impl IntoIterator<Item = Cuenta>) -> Cuenta {
    let mut total = Cuenta::default();
    for c in cuentas {
        total.dibujados = total.dibujados.max(c.dibujados);
        total.descartados = total.descartados.max(c.descartados);
        total.recortados = total.recortados.max(c.recortados);
        total.pixeles += c.pixeles;
        total.sombreados += c.sombreados;
        total.tapados += c.tapados;
        total.tirados += c.tirados;
        total.pasan += c.pasan;
    }
    total
}

/// **Las `n` franjas una detras de otra**, con el mismo destino: el contrato
/// que el banco compara con el dibujo entero (y lo que hace la casa con una
/// franja que un nucleo no pudo pintar).
pub fn en_orden(l: &Lote, d: &mut Destino, n: u32, franja: Franja) -> Result<Cuenta, NoDibuja> {
    antes(l, d)?;
    let n = cuantas(n, d.alto);
    let mut cuentas = alloc::vec::Vec::with_capacity(n as usize);
    for k in 0..n {
        let (y0, y1) = filas(k, n, d.alto);
        cuentas.push(match tijera(l.reglas.tijera, y0, y1) {
            None => Cuenta::default(),
            Some(t) => franja(&de_franja(l, t), d)?,
        });
    }
    Ok(juntar(cuentas))
}
