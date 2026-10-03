//! **LO ESCRITO EN HERMES**: tus notas y las tertulias, guardadas.
//!
//! [consumo] NADA      se lee al abrir y se escribe al mandar (L6h)
//!
//! H5 de `PLAN_HERMES.md`: *"se escribe, se cierra con Alt+F4, se vuelve a
//! abrir y lo escrito sigue ahi"*. Cada conversacion es un canal (`notas`,
//! `#general`, `#bmo-x`...), y lo que escribes en el se queda. Cuando HERMES/1
//! hable entre dos BMO-X (H6), llegaran lineas de otros al mismo fichero sin
//! cambiar nada de aqui.
//!
//! El fichero, `sys/hermsg.txt` (FAT32: 8.3 y una carpeta que ya existe; con
//! la carpeta como capacidad, H3, se mudara a la raiz `hermes/`), una linea
//! por mensaje:
//!
//! ```text
//!    <canal>\t<hora en ms del arranque>\t<texto>
//! ```
//!
//! Se reescribe entero al mandar (FAT32 no agrega al final), hasta [`TOPE`]
//! mensajes: los mas viejos se van. Una linea vieja sin hora (`canal\ttexto`)
//! se lee igual.

use alloc::vec::Vec;
use bmo_userland as bmo;

const RUTA: &[u8] = b"sys/hermsg.txt";
/// Lo mas que se guarda, de todos los canales.
const TOPE: usize = 400;
/// Lo mas largo que es un mensaje.
pub const LARGO: usize = 200;

pub struct Mensaje {
    pub canal: Vec<u8>,
    /// Los segundos desde el arranque en que se escribio (0 = no se sabe).
    pub cuando: u64,
    pub texto: Vec<u8>,
}

pub struct Charla {
    /// Del mas viejo al mas nuevo.
    lineas: Vec<Mensaje>,
    /// El ultimo guardar fallo: se dice en la ventana.
    pub sin_guardar: bool,
}

fn numero(b: &[u8]) -> Option<u64> {
    if b.is_empty() || b.len() > 15 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(b.iter().fold(0u64, |n, &c| n * 10 + (c - b'0') as u64))
}

impl Charla {
    /// Lo que hay en el disco (nada, si no hay fichero).
    pub fn abrir() -> Charla {
        let mut lineas = Vec::new();
        if let Ok(a) = bmo::Archivo::leer_de(RUTA) {
            let n = a.size() as usize;
            let mut b = alloc::vec![0u8; n];
            // De una vez (`read` mueve siete bytes por llamada).
            let k = match bmo::Memoria::request(n.max(1) as u64) {
                Some(m) => {
                    let k = (a.leer_en(&m, 0, n as u64) as usize).min(n);
                    // SAFETY: `k` bytes que el kernel acaba de escribir en el bloque.
                    unsafe { core::ptr::copy_nonoverlapping(m.base(), b.as_mut_ptr(), k) };
                    m.soltar();
                    k
                }
                None => a.read(&mut b),
            };
            a.close();
            for l in b[..k].split(|&c| c == b'\n') {
                let l: Vec<u8> = l.iter().copied().filter(|&c| c != b'\r').collect();
                let mut partes = l.splitn(3, |&c| c == b'\t');
                let (Some(c), Some(x)) = (partes.next(), partes.next()) else { continue };
                let (cuando, texto) = match (partes.next(), numero(x)) {
                    (Some(t), Some(s)) => (s, t),
                    // La forma vieja, sin hora: lo de detras del tab es el texto.
                    _ => (0, &l[c.len() + 1..]),
                };
                if !c.is_empty() && !texto.is_empty() {
                    lineas.push(Mensaje { canal: c.to_vec(), cuando, texto: texto.to_vec() });
                }
            }
        }
        Charla { lineas, sin_guardar: false }
    }

    /// Los mensajes del canal `canal`, del mas viejo al mas nuevo.
    pub fn de<'a>(&'a self, canal: &[u8]) -> Vec<&'a Mensaje> {
        self.lineas.iter().filter(|m| m.canal == canal).collect()
    }

    /// Cuantos mensajes tiene el canal.
    pub fn cuantos(&self, canal: &[u8]) -> usize {
        self.lineas.iter().filter(|m| m.canal == canal).count()
    }

    /// **Mandar**: se apunta y se guarda. Si no se pudo guardar, el mensaje
    /// se queda en la memoria y `sin_guardar` lo dice.
    pub fn mandar(&mut self, canal: &[u8], texto: &[u8], cuando: u64) {
        let t: Vec<u8> = texto.iter().copied().filter(|&c| c != b'\t' && c != b'\n' && c != b'\r').take(LARGO).collect();
        if t.iter().all(|&c| c == b' ') {
            return;
        }
        self.lineas.push(Mensaje { canal: canal.to_vec(), cuando, texto: t });
        if self.lineas.len() > TOPE {
            let sobran = self.lineas.len() - TOPE;
            self.lineas.drain(..sobran);
        }
        self.sin_guardar = !self.guardar();
    }

    fn guardar(&self) -> bool {
        let mut b = Vec::new();
        for m in &self.lineas {
            b.extend_from_slice(&m.canal);
            b.push(b'\t');
            let mut d = [0u8; 20];
            let n = crate::fmt_num(m.cuando, &mut d);
            b.extend_from_slice(&d[..n]);
            b.push(b'\t');
            b.extend_from_slice(&m.texto);
            b.push(b'\n');
        }
        let Ok(a) = bmo::Archivo::create(RUTA) else { return false };
        let n = match bmo::Memoria::request(b.len().max(1) as u64) {
            Some(m) => {
                // SAFETY: un bloque nuestro de al menos `b.len()` bytes.
                unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), m.base(), b.len()) };
                let n = a.escribir_de(&m, 0, b.len() as u64) as usize;
                m.soltar();
                n
            }
            None => a.write(&b),
        };
        a.close() && n == b.len()
    }
}
