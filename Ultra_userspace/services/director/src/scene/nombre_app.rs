//! **EL NOMBRE DE UNA APP** (04-10) -- lo que dice su titulo, su ficha y
//! Alt+Tab.
//!
//! [consumo] NADA      se pregunta UNA vez, al nacer la ventana; despues se
//!                     lee de memoria (L6h)
//!
//! El propietario, con TALLER y DOOM abiertos: la barra decia `App 1`,
//! `App 2` y el titulo `tid 16`. El escritorio SI sabe el nombre: el kernel
//! apunta que programa lanzo cada tid (`INFO_TXT_PROG_NOMBRE`), y es lo mismo
//! que ya miraba F4 para saber si la LUDOTECA estaba abierta. Aqui se
//! pregunta al nacer y se guarda por hueco de la mesa.
//!
//! ```text
//!    apps/taller.bex   ->  TALLER       los de la casa, como se escriben
//!    apps/juego.bex    ->  juego        los demas, el fichero sin `.bex`
//! ```
//!
//! El color de la ficha es el del icono de la rejilla (la misma cuenta,
//! `launcher::color_de`): la app se reconoce igual en los dos sitios.

use bmo_userland as bmo;

use super::surface::MAX;

/// Lo mas largo que se guarda de un nombre.
const LARGO: usize = 24;

/// **El nombre de una app**: como se escribe y su color.
#[derive(Clone, Copy)]
pub(crate) struct Nombre {
    texto: [u8; LARGO],
    n: u8,
    pub(crate) color: u32,
}

impl Nombre {
    pub(crate) const NADA: Nombre = Nombre { texto: [0; LARGO], n: 0, color: 0x0060_A5FA };

    pub(crate) fn texto(&self) -> &[u8] {
        &self.texto[..self.n as usize]
    }

    /// **El de `tid`**, preguntado al kernel. `NADA` si no lo apunto.
    pub(crate) fn de(tid: u32) -> Nombre {
        let mut ruta = [0u8; 40];
        let n = programa(tid, &mut ruta);
        if n == 0 {
            return Nombre::NADA;
        }
        let ruta = &ruta[..n];
        let fichero = match ruta.iter().rposition(|&b| b == b'/' || b == b'\\') {
            Some(k) => &ruta[k + 1..],
            None => ruta,
        };
        let mut me = Nombre::NADA;
        me.color = super::launcher::color_de(fichero);
        let corto = fichero.strip_suffix(b".bex").unwrap_or(fichero);
        let bonito = CASA.iter().find(|(f, _)| *f == corto).map_or(corto, |(_, b)| *b);
        let k = bonito.len().min(LARGO);
        me.texto[..k].copy_from_slice(&bonito[..k]);
        me.n = k as u8;
        me
    }
}

/// Los de la casa, como se escriben en el resto del sistema.
const CASA: [(&[u8], &[u8]); 8] = [
    (b"taller", b"TALLER"),
    (b"doom", b"DOOM"),
    (b"hermes", b"HERMES"),
    (b"ludoteca", b"LUDOTECA"),
    (b"bankcat", b"BANK CAT"),
    (b"proton-x", b"PROTON-X"),
    (b"navegar", b"Navegar"),
    (b"quake", b"QUAKE"),
];

/// **La ruta del programa de `tid`**, la que el kernel apunto al lanzarlo.
/// Devuelve cuantos bytes (0 si no lo encuentra).
pub(crate) fn programa(tid: u32, dst: &mut [u8; 40]) -> usize {
    for k in 0..64u64 {
        let quien = bmo::info(bmo::INFO_PROG_QUIEN | (k << 8));
        if quien == 0 {
            break;
        }
        if (quien >> 16) & 0xFFFF != tid as u64 {
            continue;
        }
        return bmo::info_texto(bmo::INFO_TXT_PROG_NOMBRE | (k << 8), dst).min(dst.len());
    }
    0
}

static mut POR_HUECO: [Nombre; MAX] = [Nombre::NADA; MAX];

/// La ventana del hueco `k` se llama asi (al nacer).
pub(crate) fn apuntar(k: usize, n: Nombre) {
    if k < MAX {
        // SAFETY: el escritorio es un solo hilo; esto solo lo toca la mesa.
        unsafe { (*core::ptr::addr_of_mut!(POR_HUECO))[k] = n };
    }
}

/// **Como se llama la app del hueco `k`**, si se sabe.
pub(crate) fn del_hueco(k: usize) -> Option<&'static Nombre> {
    // SAFETY: el mismo hilo; se lee lo que `apuntar` dejo.
    let n = unsafe { (*core::ptr::addr_of!(POR_HUECO)).get(k)? };
    (n.n > 0).then_some(n)
}
