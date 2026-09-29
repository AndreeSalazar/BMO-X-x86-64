//! **LOS ICONOS DEL SISTEMA**: los de las cosas que no traen el suyo.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! === Por que hacen falta, teniendo iconos ya ===
//!
//! El escritorio sabe pintar iconos desde hace dias, pero los saca **de dentro
//! del `.bex`** (`scene/launcher.rs`): la app trae su propia cara como un
//! recurso de su paquete, y por eso no hay `.lnk` que se despegue.
//!
//! * **Una carpeta no tiene `.bex`.** Ni un `.txt`, ni un nodo que no se pudo
//! leer. No hay de donde sacarles la cara, asi que o la trae el sistema o la
//! rejilla se queda con un cuadrito de color -- que dice la clase pero no se
//! reconoce de un vistazo, que es justo lo unico que un icono tiene que hacer.
//!
//! === Escritos como DIBUJO, y desde el 29-09 como FORMAS ===
//!
//! Primero fueron rejillas de 16 lineas (`o`, `#`, `+`, `-`), por la misma
//! razon que el formato `BICO`: *"una rejilla se lee, se corrige y se ve mal
//! cuando esta mal"*. Y se veia mal de verdad agrandada: 16 pixeles a 32 son
//! cuadros de 2x2. El propietario: *"investigar los mejores iconos y eso en
//! svg ... porque se ven feo"*. Ahora son FORMAS sobre la rejilla de 24 de
//! Fluent y Lucide (`dibujos.rs`), que `bmo_dibujo::icono` rasteriza a la
//! medida justa, suavizadas y en degradado. Siguen leyendose como dibujo: una
//! caja, un circulo, un trazo, con sus numeros.
//!
//! === Un dibujo por FORMA, un color por CLASE ===
//!
//! Los simbolos no son colores: son PAPELES. El cuerpo, el brillo, la sombra y
//! el contorno se resuelven contra el color de la clase en el momento de
//! pintar, asi que hay **un** dibujo de carpeta y no uno por color.
//!
//! Y el color es el mismo que su caja en el grafo de al lado, a proposito:
//! mirar el mismo nodo en los dos paneles no puede darle dos colores. La forma
//! dice QUE ES y el color dice lo mismo -- se refuerzan en vez de competir.
//!
//! === Y por que esto NO es un `.maqueta` ===
//!
//! Porque MAQUETA maqueta: reparte cajas, no dibuja mapas de bits. Su ley L7
//! --la que le prohibe ser un navegador-- es justo lo que la deja terminable, y
//! meterle pixeles seria la primera grieta.
//!
//! Lo que SI le toca a MAQUETA de todo esto es la PALETA, y ya esta dicho en
//! `tema/tema.maqueta`: *"mientras no exista el emisor, este fichero es la
//! FUENTE y las constantes de Rust son la copia"*. Esa deuda sigue abierta y
//! los colores de aqui salen de `class_color`, que es donde ya vivian.

use bmo_userland as bmo;

/// Lado del dibujo. El mismo 16 que `BICO`, y por la misma razon: se guarda
/// chico y se agranda si hace falta.
pub(crate) const LADO: u32 = 16;

/// El contorno es FIJO y no sale de la clase: un contorno tenido del mismo
/// color que el cuerpo deja de recortar la silueta y el icono se convierte en
/// una mancha.
const CONTORNO: u32 = 0x0010_141B;

/// Los dibujos: datos en `dibujos.rs`, que el banco del anfitrion tambien
/// incluye (por eso no traen su propio `use`).
pub(crate) mod dibujos {
    use bmo_dibujo::{Capa, Figura, Tinta};
    include!("dibujos.rs");
}

fn aclarar_pct(c: u32, k: u32) -> u32 {
    let f = |s: u32| {
        let v = (c >> s) & 0xFF;
        (v + (255 - v) * k / 100) << s
    };
    f(16) | f(8) | f(0)
}

fn oscurecer_pct(c: u32, k: u32) -> u32 {
    let f = |s: u32| (((c >> s) & 0xFF) * (100 - k) / 100) << s;
    f(16) | f(8) | f(0)
}

/// Los siete papeles de `dibujos.rs` para un `color` (y la `luz` del piloto).
pub(crate) fn paleta(color: u32, luz: u32) -> [u32; 7] {
    [color, aclarar_pct(color, 35), oscurecer_pct(color, 40), CONTORNO, luz, aclarar_pct(color, 75), 0x00F0_B84A]
}

/// **Pinta un dibujo vectorial** de `lado` pixeles en `(x, y)`, mezclado contra
/// `fondo` (el color que hay debajo: el framebuffer no se lee).
pub(crate) fn vector(p: &bmo::Pantalla, x: u32, y: u32, lado: u32, capas: &[bmo_dibujo::Capa], paleta: &[u32], fondo: u32) {
    p.marcar(x, y, lado, lado);
    bmo_dibujo::icono(capas, lado, paleta, fondo, |i, j, c| p.punto_ya_marcado(x + i, y + j, c & 0x00FF_FFFF));
}

/// **Pinta el icono de `kind` en `(x, y)`, del color de su clase**, sobre
/// `fondo`.
///
/// `escala` multiplica el lado: a 1 son 16 pixeles, que es lo que cabe en una
/// fila de la rejilla; a 2 son 32, que es lo que usa el lanzador.
pub(crate) fn pintar(p: &bmo::Pantalla, x: u32, y: u32, kind: u64, color: u32, escala: u32, fondo: u32) {
    let dibujo: Option<&[bmo_dibujo::Capa]> = match kind {
        bmo::estratos::DIRECTORIO => Some(&dibujos::CARPETA),
        bmo::estratos::ARCHIVO => Some(&dibujos::HOJA),
        _ => None,
    };
    if let Some(d) = dibujo {
        vector(p, x, y, LADO * escala, d, &paleta(color, color), fondo);
        return;
    }
    // Lo que no se pudo leer NO lleva dibujo, a proposito: no se sabe que
    // es, y dibujarle una hoja o una carpeta seria contestar por el disco. Una
    // caja del color de su clase con el `?` de la fuente.
    let lado = LADO * escala;
    p.rect(x, y, lado, lado, color);
    p.glifo_escala(x + lado / 2 - 4 * escala, y + lado / 2 - 8 * escala / 2, b'?', CONTORNO, escala);
}
