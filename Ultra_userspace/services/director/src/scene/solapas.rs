//! **LAS SOLAPAS DE EJECUTAR: cuales hay y como se llaman** (01-10). Solo
//! datos: la caja las pinta con esto (`scene::caja`) y `desktop::solapas`
//! las cambia. El porque, en `desktop::solapas`.
//!
//! [consumo] NADA      datos, sin bucle (L6h)

/// Cuantas caben.
pub(crate) const MAX: usize = 4;

pub(crate) struct Estado {
    pub(crate) activa: usize,
    pub(crate) vivas: [bool; MAX],
    /// Su numero, el que se ve (1, 2, 3...): no cambia al cerrar otra.
    pub(crate) numero: [u8; MAX],
    pub(crate) siguiente: u8,
    /// La primera palabra de la ultima orden que se corrio en ella.
    pub(crate) nombre: [[u8; 12]; MAX],
    pub(crate) nombre_n: [usize; MAX],
}

static mut ESTADO: Estado = Estado {
    activa: 0,
    vivas: [true, false, false, false],
    numero: [1, 0, 0, 0],
    siguiente: 2,
    nombre: [[0; 12]; MAX],
    nombre_n: [0; MAX],
};

pub(crate) fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// Las vivas, en orden, y cuantas son.
pub(crate) fn lista() -> ([usize; MAX], usize) {
    let e = estado();
    let mut l = [0usize; MAX];
    let mut n = 0;
    for k in 0..MAX {
        if e.vivas[k] {
            l[n] = k;
            n += 1;
        }
    }
    (l, n)
}

pub(crate) fn activa() -> usize {
    estado().activa
}

/// Lo que dice la solapa `k`: su numero y su ultima orden, o "Ejecutar".
pub(crate) fn rotulo(k: usize, out: &mut [u8; 20]) -> usize {
    let e = estado();
    let mut n = 0;
    let (_, vivas) = lista();
    if vivas > 1 {
        out[0] = b'0' + e.numero[k] % 10;
        out[1] = b' ';
        n = 2;
    }
    let t: &[u8] = if e.nombre_n[k] > 0 { &e.nombre[k][..e.nombre_n[k]] } else { b"Ejecutar" };
    let m = t.len().min(out.len() - n);
    out[n..n + m].copy_from_slice(&t[..m]);
    n + m
}

/// La orden que se acaba de correr en la de delante le pone nombre.
pub(crate) fn apuntar(linea: &[u8]) {
    let e = estado();
    let a = e.activa;
    let palabra = linea.split(|&c| c == b' ').find(|p| !p.is_empty()).unwrap_or(b"");
    let palabra = palabra.rsplit(|&c| c == b'/').next().unwrap_or(palabra);
    let m = palabra.len().min(12);
    e.nombre[a][..m].copy_from_slice(&palabra[..m]);
    e.nombre_n[a] = m;
}
