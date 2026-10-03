//! **LA ANTENA, en Rust** -- AO0 de `docs/plan/PLAN_LA_ANTENA_AOT.md`
//! (2026-10-03). El propietario: *"vamos a empezar con la ANTENA y preparar
//! en Rust todo eso en mi HONOR"*.
//!
//! Hace lo de `toolchain/tools/antena/antena.py`, con las MISMAS reglas
//! estrictas, y una diferencia que es la razon de existir:
//!
//! ```text
//!    los pedidos     los lee `bmo_antena::leer_pedido`, el espejo de lo que
//!                    BMO-X escribe con `bmo_antena::escribir`
//!    las laminas     las juzga `bmo_antena::lamina::Lector`, el MISMO juez
//!                    que BMO-X: `lamina_juez.py` era una copia a mano
//!    lo de fuera     ni un crate: `std` y el protocolo de la casa
//! ```
//!
//! ** Lo que AUN no hace, dicho: `PAGINA <url>` contesta `NO` -- navegar es
//! de la WebView del sistema, que llega con la app chica (AO2). El navegador
//! de Chromium en Termux sigue siendo cosa de `antena.py`.
//!
//! ESTRICTA, como la de Python (2026-09-14, *"MAS ESTRICTO ANTENA"*):
//!
//! ```text
//!    UNA IP          otra se cierra sin una palabra, y un segundo de castigo
//!    UNA conexion    a la vez, y UN video por conexion
//!    plazos          el saludo en 10 s; la charla entera, 120 s
//!    72 lineas       como mucho por conexion
//!    fuera           la primera linea que no es ANTENA/1 cuelga sin contestar
//!    la carpeta      solo ficheros NORMALES de DENTRO: un enlace que sale, no
//!    ffmpeg          solo lee ficheros (`-protocol_whitelist file,pipe`)
//!    ninguna IP      ni se escribe en un fichero ni se imprime
//! ```

pub mod carpeta;
pub mod charla;
pub mod video;

use std::io;
use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::time::Duration;

/// El castigo tras una IP que no es la permitida: un barrido no la tumba.
pub const CASTIGO: Duration = Duration::from_secs(1);

/// Lo que la antena sabe de si misma.
#[derive(Clone, Debug)]
pub struct Ajuste {
    pub carpeta: PathBuf,
    pub permitir: IpAddr,
    pub nombre: String,
    /// El programa que convierte (para las pruebas, uno que no existe).
    pub ffmpeg: String,
}

/// **Atiende**, una conexion detras de otra. `veces`: cuantas conexiones
/// atender antes de volver (`None` = para siempre; las pruebas ponen una).
pub fn servir(escucha: &TcpListener, a: &Ajuste, veces: Option<usize>) -> io::Result<()> {
    let mut hechas = 0;
    while veces.map_or(true, |v| hechas < v) {
        let (conexion, origen) = escucha.accept()?;
        hechas += 1;
        if origen.ip() != a.permitir {
            println!("antena: cerrada una conexion de una IP no permitida");
            drop(conexion);
            std::thread::sleep(CASTIGO);
            continue;
        }
        println!("antena: BMO-X conectado");
        let mut tubo = charla::Tcp(conexion);
        match charla::atender(&mut tubo, a) {
            Ok(()) => {}
            Err(charla::Corte::Fuera(m)) => println!("antena: colgada, fuera de protocolo: {m}"),
            Err(charla::Corte::Red(_)) => println!("antena: colgada por tiempo o por la red"),
        }
        println!("antena: conexion cerrada");
    }
    Ok(())
}
