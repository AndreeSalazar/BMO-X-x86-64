//! **Cargar el `.exe` Y SUS DLL en una imagen declarada** (P0.4b.4, 30-09).
//!
//! ```text
//!    1  el .exe y, de sus importaciones (tambien las retrasadas), las DLL
//!       que NO son de la casa y estan junto a el; de cada una, lo mismo:
//!       dependencias primero (el orden en que se registran y corren sus
//!       DllMain, como el cargador de Windows)
//!    2  DECLARAR todas sus partes de una vez (bmo::Imagen): el kernel las
//!       juzga contra la RAM libre de ahora; si no, dice cuanta pide y hay
//!    3  colocar cada seccion del disco a su sitio: el kernel lee a un BLOQUE
//!       (no a la imagen), asi que va por un bloque de paso de 2 MiB, a trozos
//!       y cediendo el turno
//! ```
//!
//! Resolver, registrar las exportaciones, sellar y los DllMain los hace
//! `main.rs`, despues de `empezar` (la casa tiene que estar lista antes de
//! registrar nada en ella).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bmo_proton_x::{colocar_en, tramos, Pe, Tramo};
use bmo_userland as bmo;

use crate::plataforma::ahora_ns;
use crate::{di, fin, TROZO};

/// Un PE de la imagen: el `.exe` (el primero) o una DLL del juego.
pub(crate) struct Modulo {
    pub nombre: String,
    pub ruta: Vec<u8>,
    pub pe: Pe,
    /// Sus tramos (codigo, datos, codigo...: `bmo_proton_x::tramos`), cada
    /// uno una parte de la declaracion.
    pub tramos: Vec<Tramo>,
    /// El indice de su primer tramo en la declaracion (los demas, detras).
    pub primera: usize,
    /// Donde quedo (la VA de su RVA 0; los tramos, seguidos).
    pub base: u64,
}

impl Modulo {
    /// Los indices, en la declaracion, de sus tramos de codigo: los que se
    /// SELLAN.
    pub fn de_codigo(&self) -> impl Iterator<Item = usize> + '_ {
        self.tramos
            .iter()
            .enumerate()
            .filter(|(_, t)| t.codigo)
            .map(|(k, _)| self.primera + k)
    }

    /// Su imagen entera, los tramos seguidos.
    pub fn imagen(&self) -> &'static mut [u8] {
        let total: usize = self.tramos.iter().map(|t| t.bytes as usize).sum();
        // SAFETY: las partes de este modulo estan mapeadas, seguidas y a cero
        // (el juez las pone asi: `bmo-imagen-juicio`) y viven lo que el proceso.
        unsafe { core::slice::from_raw_parts_mut(self.base as *mut u8, total) }
    }
}

pub(crate) fn junto(dir: &[u8], dll: &str) -> Vec<u8> {
    let mut r = dir.to_vec();
    r.extend_from_slice(dll.as_bytes());
    r
}

/// Las DLL del juego que pide `ruta` (importadas y retrasadas), en el orden
/// en que las nombra, y sus cabeceras (se lee UNA vez: P0.4d).
fn pedidas(ruta: &[u8], dir: &[u8]) -> (Vec<String>, Pe) {
    let m = crate::mirar(ruta)
        .unwrap_or_else(|f| fin(&format!("{}: {f}", String::from_utf8_lossy(ruta))));
    let mut v: Vec<String> = Vec::new();
    for i in m.imps.iter().chain(&m.retrasadas) {
        let d = &i.dll;
        if bmo_proton_x_casa::modulos::es_de_la_casa(d)
            || v.iter().any(|x| x.eq_ignore_ascii_case(d))
        {
            continue;
        }
        if bmo::Archivo::reflejar(&junto(dir, d)).is_ok() {
            v.push(d.clone());
        }
    }
    (v, m.pe)
}

/// Una DLL y, antes, las suyas (post-orden).
fn visitar(dll: &str, dir: &[u8], lista: &mut Vec<Modulo>, vistos: &mut Vec<String>) {
    if vistos.iter().any(|v| v.eq_ignore_ascii_case(dll)) {
        return;
    }
    vistos.push(String::from(dll));
    let ruta = junto(dir, dll);
    let (suyas, pe) = pedidas(&ruta, dir);
    for d in suyas {
        visitar(&d, dir, lista, vistos);
    }
    if !pe.es_dll {
        fin(&format!("{dll}: esta junto al .exe pero no es una DLL"));
    }
    let tramos = tramos(&pe).unwrap_or_else(|f| fin(&format!("{dll}: {f}")));
    lista.push(Modulo {
        nombre: String::from(dll),
        ruta,
        pe,
        tramos,
        primera: 0,
        base: 0,
    });
}

/// **Declarar y colocar** el `.exe` de `ruta` y sus DLL. El primero de la
/// lista es el `.exe`.
pub(crate) fn declarar_y_colocar(
    ruta: &[u8],
    pe: Pe,
    tramos: Vec<Tramo>,
) -> (bmo::Imagen, Vec<Modulo>, Tiempos) {
    let mut tiempos = Tiempos::default();
    let t0 = ahora_ns();
    let dir: Vec<u8> = match ruta.iter().rposition(|&c| c == b'/') {
        Some(k) => ruta[..k + 1].to_vec(),
        None => Vec::new(),
    };
    let nombre = String::from_utf8_lossy(ruta).into_owned();
    let mut modulos = alloc::vec![Modulo {
        nombre: nombre.clone(),
        ruta: ruta.to_vec(),
        pe,
        tramos,
        primera: 0,
        base: 0
    }];
    let mut vistos: Vec<String> =
        alloc::vec![String::from(nombre.rsplit('/').next().unwrap_or(&nombre))];
    for d in pedidas(ruta, &dir).0 {
        visitar(&d, &dir, &mut modulos, &mut vistos);
    }
    // 04-10: y las que el .exe carga EN VIVO (LoadLibrary), si estan junto
    // a el: la imagen se declara de una vez (ver `en_vivo.rs`).
    for d in crate::en_vivo::pedidas_en_vivo(ruta, &modulos[0].pe, &dir, &vistos) {
        visitar(&d, &dir, &mut modulos, &mut vistos);
    }

    tiempos.cabeceras = ahora_ns() - t0;
    // -- 2. La declaracion: de cada PE, sus tramos en orden.
    let mut decl: Vec<bmo::ParteImagen> = Vec::new();
    let (mut c, mut d) = (0u64, 0u64);
    for (k, m) in modulos.iter_mut().enumerate() {
        m.primera = decl.len();
        for t in &m.tramos {
            decl.push(bmo::ParteImagen {
                pe: k as u16,
                codigo: t.codigo,
                bytes: t.bytes as u64,
            });
            *(if t.codigo { &mut c } else { &mut d }) += t.bytes as u64;
        }
    }
    let imagen = bmo::Imagen::declarar(&decl).unwrap_or_else(|no| {
        fin(&format!("el kernel NO concede la imagen ({} modulos, {} MiB): {} [pide {} MiB, hay {} MiB; valor {:#x}]", modulos.len(), (c + d) >> 20, no.frase(), no.pide_mib(), no.hay_mib(), no.valor))
    });
    di(&format!("PROTON-X: imagen DECLARADA y concedida: {} modulo(s) (el .exe y {} DLL del juego), {} partes, codigo {} MiB + datos {} MiB\n", modulos.len(), modulos.len() - 1, decl.len(), c >> 20, d >> 20));

    // -- 3. Colocar, por un bloque de paso.
    let t1 = ahora_ns();
    let Some(paso) = bmo::Memoria::request(TROZO) else {
        fin("sin memoria para el bloque de paso")
    };
    for m in modulos.iter_mut() {
        m.base = imagen
            .parte(m.primera)
            .unwrap_or_else(|| fin(&format!("{}: el kernel no dice donde quedo", m.nombre)))
            as u64;
        let Ok(a) = bmo::Archivo::reflejar(&m.ruta) else {
            fin(&format!("{}: no se abre", m.nombre))
        };
        let img = m.imagen();
        colocar_en(&m.pe, img, m.base, |desde, destino| {
            let tam = destino.len() as u64;
            let mut hecho = 0u64;
            while hecho < tam {
                let k = TROZO.min(tam - hecho);
                let pos = desde + hecho;
                let d0 = ahora_ns();
                if a.saltar(pos) != pos || a.leer_en(&paso, 0, k) != k {
                    return false;
                }
                tiempos.disco += ahora_ns() - d0;
                tiempos.bytes += k;
                // SAFETY: `k` bytes que el kernel acaba de escribir en el
                // bloque de paso, que es nuestro.
                let de =
                    unsafe { core::slice::from_raw_parts(paso.base() as *const u8, k as usize) };
                destino[hecho as usize..(hecho + k) as usize].copy_from_slice(de);
                hecho += k;
                bmo::yield_screen();
            }
            true
        })
        .unwrap_or_else(|f| fin(&format!("{}: {f}", m.nombre)));
    }
    paso.soltar();
    tiempos.colocar = ahora_ns() - t1;
    (imagen, modulos, tiempos)
}

/// **Lo que tardo cada fase de la carga** (P0.4b.9), en ns: medir antes de
/// hacer la cache (P0.4d), para saber QUE se ahorra.
#[derive(Default)]
pub(crate) struct Tiempos {
    /// Las cabeceras y los cierres de importaciones de todos.
    pub cabeceras: u64,
    /// Colocar entero (disco + copiar + relocalizar + ceder el turno).
    pub colocar: u64,
    /// De eso, lo que fue leer del disco, y cuantos bytes.
    pub disco: u64,
    pub bytes: u64,
}
