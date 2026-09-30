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
use bmo_proton_x::{colocar_en, leer_cabeceras, partir, Partes, Pe};
use bmo_userland as bmo;

use crate::{di, fin, CABECERAS, TROZO};

/// Un PE de la imagen: el `.exe` (el primero) o una DLL del juego.
pub(crate) struct Modulo {
    pub nombre: String,
    pub ruta: Vec<u8>,
    pub pe: Pe,
    pub partes: Partes,
    /// El indice de su parte de codigo en la declaracion (los datos, si los
    /// tiene, van en la siguiente).
    pub parte_codigo: usize,
    /// Donde quedo (la VA de su codigo; los datos, justo detras).
    pub base: u64,
}

impl Modulo {
    /// Su imagen entera, codigo y datos seguidos.
    pub fn imagen(&self) -> &'static mut [u8] {
        let total = self.partes.codigo as usize + self.partes.datos as usize;
        // SAFETY: las partes de este modulo estan mapeadas, seguidas y a cero
        // (el juez las pone asi: `bmo-imagen-juicio`) y viven lo que el proceso.
        unsafe { core::slice::from_raw_parts_mut(self.base as *mut u8, total) }
    }
}

/// Las cabeceras de un PE, sin traerlo.
fn cabeceras(ruta: &[u8]) -> Result<Pe, String> {
    let a = bmo::Archivo::reflejar(ruta).map_err(|_| String::from("no esta"))?;
    let mide = a.size();
    let n = CABECERAS.min(mide).max(1);
    let hb = bmo::Memoria::request(n).ok_or("sin memoria para las cabeceras")?;
    let k = a.leer_en(&hb, 0, n);
    // SAFETY: `k` bytes que el kernel acaba de escribir en un bloque nuestro.
    let cab: Vec<u8> = unsafe { core::slice::from_raw_parts(hb.base() as *const u8, k as usize) }.to_vec();
    hb.soltar();
    leer_cabeceras(&cab, mide).map_err(|f| format!("{f}"))
}

fn junto(dir: &[u8], dll: &str) -> Vec<u8> {
    let mut r = dir.to_vec();
    r.extend_from_slice(dll.as_bytes());
    r
}

/// Las DLL del juego que pide `ruta` (importadas y retrasadas), en el orden
/// en que las nombra.
fn pedidas(ruta: &[u8], dir: &[u8]) -> Vec<String> {
    let m = crate::mirar(ruta).unwrap_or_else(|f| fin(&format!("{}: {f}", String::from_utf8_lossy(ruta))));
    let mut v: Vec<String> = Vec::new();
    for i in m.imps.iter().chain(&m.retrasadas) {
        let d = &i.dll;
        if bmo_proton_x_casa::modulos::es_de_la_casa(d) || v.iter().any(|x| x.eq_ignore_ascii_case(d)) {
            continue;
        }
        if bmo::Archivo::reflejar(&junto(dir, d)).is_ok() {
            v.push(d.clone());
        }
    }
    v
}

/// Una DLL y, antes, las suyas (post-orden).
fn visitar(dll: &str, dir: &[u8], lista: &mut Vec<Modulo>, vistos: &mut Vec<String>) {
    if vistos.iter().any(|v| v.eq_ignore_ascii_case(dll)) {
        return;
    }
    vistos.push(String::from(dll));
    let ruta = junto(dir, dll);
    for d in pedidas(&ruta, dir) {
        visitar(&d, dir, lista, vistos);
    }
    let pe = cabeceras(&ruta).unwrap_or_else(|f| fin(&format!("{dll}: {f}")));
    if !pe.es_dll {
        fin(&format!("{dll}: esta junto al .exe pero no es una DLL"));
    }
    let partes = partir(&pe).unwrap_or_else(|f| fin(&format!("{dll}: {f}")));
    lista.push(Modulo { nombre: String::from(dll), ruta, pe, partes, parte_codigo: 0, base: 0 });
}

/// **Declarar y colocar** el `.exe` de `ruta` y sus DLL. El primero de la
/// lista es el `.exe`.
pub(crate) fn declarar_y_colocar(ruta: &[u8], pe: Pe, partes: Partes) -> (bmo::Imagen, Vec<Modulo>) {
    let dir: Vec<u8> = match ruta.iter().rposition(|&c| c == b'/') {
        Some(k) => ruta[..k + 1].to_vec(),
        None => Vec::new(),
    };
    let nombre = String::from_utf8_lossy(ruta).into_owned();
    let mut modulos = alloc::vec![Modulo { nombre: nombre.clone(), ruta: ruta.to_vec(), pe, partes, parte_codigo: 0, base: 0 }];
    let mut vistos: Vec<String> = alloc::vec![String::from(nombre.rsplit('/').next().unwrap_or(&nombre))];
    for d in pedidas(ruta, &dir) {
        visitar(&d, &dir, &mut modulos, &mut vistos);
    }

    // -- 2. La declaracion: de cada PE, su codigo y (si hay) sus datos.
    let mut decl: Vec<bmo::ParteImagen> = Vec::new();
    for (k, m) in modulos.iter_mut().enumerate() {
        m.parte_codigo = decl.len();
        decl.push(bmo::ParteImagen { pe: k as u16, codigo: true, bytes: m.partes.codigo as u64 });
        if m.partes.datos > 0 {
            decl.push(bmo::ParteImagen { pe: k as u16, codigo: false, bytes: m.partes.datos as u64 });
        }
    }
    let (c, d): (u64, u64) = modulos.iter().fold((0, 0), |(c, d), m| (c + m.partes.codigo as u64, d + m.partes.datos as u64));
    let imagen = bmo::Imagen::declarar(&decl).unwrap_or_else(|no| {
        fin(&format!("el kernel NO concede la imagen ({} modulos, {} MiB): {} [pide {} MiB, hay {} MiB; valor {:#x}]", modulos.len(), (c + d) >> 20, no.frase(), no.pide_mib(), no.hay_mib(), no.valor))
    });
    di(&format!("PROTON-X: imagen DECLARADA y concedida: {} modulo(s) (el .exe y {} DLL del juego), {} partes, codigo {} MiB + datos {} MiB\n", modulos.len(), modulos.len() - 1, decl.len(), c >> 20, d >> 20));

    // -- 3. Colocar, por un bloque de paso.
    let Some(paso) = bmo::Memoria::request(TROZO) else { fin("sin memoria para el bloque de paso") };
    for m in modulos.iter_mut() {
        m.base = imagen.parte(m.parte_codigo).unwrap_or_else(|| fin(&format!("{}: el kernel no dice donde quedo", m.nombre))) as u64;
        let Ok(a) = bmo::Archivo::reflejar(&m.ruta) else { fin(&format!("{}: no se abre", m.nombre)) };
        let img = m.imagen();
        colocar_en(&m.pe, img, m.base, |desde, destino| {
            let tam = destino.len() as u64;
            let mut hecho = 0u64;
            while hecho < tam {
                let k = TROZO.min(tam - hecho);
                let pos = desde + hecho;
                if a.saltar(pos) != pos || a.leer_en(&paso, 0, k) != k {
                    return false;
                }
                // SAFETY: `k` bytes que el kernel acaba de escribir en el
                // bloque de paso, que es nuestro.
                let de = unsafe { core::slice::from_raw_parts(paso.base() as *const u8, k as usize) };
                destino[hecho as usize..(hecho + k) as usize].copy_from_slice(de);
                hecho += k;
                bmo::yield_screen();
            }
            true
        })
        .unwrap_or_else(|f| fin(&format!("{}: {f}", m.nombre)));
    }
    paso.soltar();
    (imagen, modulos)
}
