//! **HERMES, pintado en el anfitrion** por SU codigo (`#[path]` a los
//! ficheros de la app), a la medida de su maqueta (1222 x 710).
//!
//! `cara-hermes <carpeta>` deja `mensajes.png` (la de la foto de la
//! maqueta), una por seccion (`sec0.png`...) y `minimo.png`.

extern crate alloc;

#[path = "../../../../../Ultra_userspace/apps/taller/src/canvas.rs"]
#[allow(dead_code)]
mod canvas;
#[path = "../../../../../Ultra_userspace/services/director/src/scene/gato.rs"]
#[allow(dead_code)]
mod gato;
#[path = "../../../../../Ultra_userspace/apps/ludoteca/src/mates.rs"]
#[allow(dead_code)]
mod mates;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/pintar.rs"]
#[allow(dead_code)]
mod pintar;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/entrada.rs"]
#[allow(dead_code)]
mod entrada;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/onda.rs"]
#[allow(dead_code)]
mod onda;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/panel.rs"]
#[allow(dead_code)]
mod panel;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/piezas.rs"]
#[allow(dead_code)]
mod piezas;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/iconos.rs"]
#[allow(dead_code)]
mod iconos;
#[path = "../../../../../Ultra_userspace/apps/hermes/src/reproductor.rs"]
#[allow(dead_code)]
mod reproductor;
/// HM6: el espejo compara con la maqueta de siempre, que no es la de mision:
/// aqui no hay escritorio de mision detras, y la ANTENA no se pinta.
mod mision {
    pub fn es_mision() -> bool {
        false
    }
}
mod antena {
    pub const ALTO: i32 = 0;
    pub fn pintar(_: &mut crate::canvas::Canvas, _: i32, _: i32, _: i32, _: u32) {}
}

/// Lo que `pintar` lee de la charla (la de verdad vive en `charla.rs`, que
/// habla con el disco).
mod charla {
    #[allow(dead_code)]
    pub struct Mensaje {
        pub canal: Vec<u8>,
        pub cuando: u64,
        pub texto: Vec<u8>,
    }
}

use bmo_espejo_cara::{png, Imagen};

pub fn fmt_num(v: u64, out: &mut [u8]) -> usize {
    let s = v.to_string();
    let n = s.len().min(out.len());
    out[..n].copy_from_slice(&s.as_bytes()[..n]);
    n
}

fn guardar(ruta: &str, px: &[u32], w: usize, h: usize) {
    let im = Imagen { ancho: w, alto: h, px: px.iter().map(|p| p & 0x00FF_FFFF).collect() };
    if let Err(e) = png::escribir(ruta, &im) {
        eprintln!("NO: {e}");
        std::process::exit(1);
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let notas: Vec<charla::Mensaje> = [
        &b"hola BMO-X: esto es una nota que se queda guardada"[..],
        b"luego: probar F3 en el Ryzen, escribir, cerrar con Alt+F4 y volver a abrir para ver que sigue aqui",
        b"nya",
    ]
    .iter()
    .map(|t| charla::Mensaje { canal: b"notas".to_vec(), cuando: 100, texto: t.to_vec() })
    .collect();
    let refs: Vec<&charla::Mensaje> = notas.iter().collect();
    let vacio: Vec<&charla::Mensaje> = Vec::new();
    bmo_userland::MEDIDOR.store(
        ((-2600i16 as u16 as u64) << 32) | ((-2900i16 as u16 as u64) << 48) | (-900i16 as u16 as u64) | ((-1100i16 as u16 as u64) << 16),
        std::sync::atomic::Ordering::Relaxed,
    );
    for (nombre, w, h) in [("", 1222u32, 710u32), ("minimo", 960, 600)] {
        pintar::medir(w, h);
        let (w, h) = (w as usize, h as usize);
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        let secciones = if nombre.is_empty() { 0..9 } else { 0..1 };
        for sec in secciones {
            let ms = 103_000;
            let v = pintar::Vista {
                sec,
                item: if sec == 4 { 9 } else { 0 },
                ms,
                desde_sec: 100_000,
                desde_item: 100_000,
                puntero: None,
                charla: if sec == 0 { &refs } else { &vacio },
                cuentas: &[3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                escribiendo: false,
                borrador: b"",
                enviado: 0,
                zumbido: None,
                zumbido_listo: true,
                pedida: Some(9),
                pedida_desde: 100_000,
                sonando: true,
                pico: [-900, -1100],
                rms: [-2600, -2900],
                sin_guardar: false,
                aviso: b"",
                pausada: false,
                volumen: 70,
            };
            pintar::pintar(&mut cv, &v);
            let n = if nombre.is_empty() { if sec == 0 { "mensajes".to_string() } else { format!("sec{sec}") } } else { nombre.to_string() };
            guardar(&format!("{out}/{n}.png"), &px, w, h);
        }
    }
    println!("ok: HERMES en {out}");
}
