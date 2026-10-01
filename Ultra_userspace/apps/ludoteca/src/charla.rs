//! **LA CHARLA de cada canal** (01-10). Pedido: *"la LUDOTECA tiene que tener
//! sus mensajes tambien"*.
//!
//! [consumo] NADA      se lee al abrir y se escribe al mandar (L6h)
//!
//! Cada juego es un canal, como en una comunidad, y lo que escribes en el se
//! queda. Hoy son TUS notas del juego (que funciona, que falta, que mod
//! llevas); cuando el protocolo HERMES hable entre dos BMO-X, un canal se
//! comparte con tus amigos sin cambiar nada de aqui: solo llegan lineas de
//! otros.
//!
//! El fichero, `sys/ludomsg.txt` (FAT32: 8.3 y una carpeta que ya existe),
//! una linea por mensaje:
//!
//! ```text
//!    <canal>\t<texto>
//! ```
//!
//! Se reescribe entero al mandar (FAT32 no agrega al final), hasta [`TOPE`]
//! mensajes: los mas viejos se van.

use alloc::vec::Vec;
use bmo_userland as bmo;

const RUTA: &[u8] = b"sys/ludomsg.txt";
/// Lo mas que se guarda, de todos los canales.
const TOPE: usize = 300;
/// Lo mas largo que es un mensaje.
pub const LARGO: usize = 160;

pub struct Charla {
    /// `(canal, texto)`, del mas viejo al mas nuevo.
    lineas: Vec<(Vec<u8>, Vec<u8>)>,
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
                if let Some(t) = l.iter().position(|&c| c == b'\t') {
                    if t > 0 && t + 1 < l.len() {
                        lineas.push((l[..t].to_vec(), l[t + 1..].iter().copied().filter(|&c| c != b'\r').collect()));
                    }
                }
            }
        }
        Charla { lineas }
    }

    /// Los textos del canal `canal`, del mas viejo al mas nuevo.
    pub fn de<'a>(&'a self, canal: &[u8]) -> Vec<&'a [u8]> {
        self.lineas.iter().filter(|(c, _)| c == canal).map(|(_, t)| t.as_slice()).collect()
    }

    /// **Mandar**: se apunta y se guarda. `false` si no se pudo guardar (el
    /// mensaje se queda igual, en la memoria).
    pub fn mandar(&mut self, canal: &[u8], texto: &[u8]) -> bool {
        let t: Vec<u8> = texto.iter().copied().filter(|&c| c != b'\t' && c != b'\n' && c != b'\r').take(LARGO).collect();
        if t.is_empty() {
            return true;
        }
        self.lineas.push((canal.to_vec(), t));
        if self.lineas.len() > TOPE {
            let sobran = self.lineas.len() - TOPE;
            self.lineas.drain(..sobran);
        }
        self.guardar()
    }

    fn guardar(&self) -> bool {
        let mut b = Vec::new();
        for (c, t) in &self.lineas {
            b.extend_from_slice(c);
            b.push(b'\t');
            b.extend_from_slice(t);
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
