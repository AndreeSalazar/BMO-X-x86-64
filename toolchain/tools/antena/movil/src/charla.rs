//! **La charla**: ANTENA/1 linea a linea, con sus topes de largo, de
//! cantidad y de tiempo. Cada pedido lo lee `bmo_antena::leer_pedido`; cada
//! lamina la juzga `bmo_antena::lamina::Lector` ANTES de mandar la primera
//! linea.

use crate::carpeta::{catalogo, dentro, limpio};
use crate::Ajuste;
use bmo_antena::{leer_pedido, Pedido, LINEAS_MAX, LINEA_MAX, VERSION};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

pub const SALUDO: Duration = Duration::from_secs(10);
pub const CHARLA: Duration = Duration::from_secs(120);

/// Por que se colgo.
#[derive(Debug)]
pub enum Corte {
    /// Una linea que no es ANTENA/1: se cuelga sin contestar.
    Fuera(&'static str),
    /// El tiempo o la red.
    Red(io::Error),
}

impl From<io::Error> for Corte {
    fn from(e: io::Error) -> Self {
        Corte::Red(e)
    }
}

/// **Un tubo**: se lee, se escribe y se le pone plazo a la proxima lectura.
pub trait Tubo: Read + Write {
    fn plazo(&mut self, d: Duration) -> io::Result<()>;
}

/// El tubo de verdad.
pub struct Tcp(pub TcpStream);

impl Read for Tcp {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.read(b)
    }
}

impl Write for Tcp {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Tubo for Tcp {
    fn plazo(&mut self, d: Duration) -> io::Result<()> {
        self.0.set_read_timeout(Some(d.max(Duration::from_millis(1))))?;
        self.0.set_write_timeout(Some(Duration::from_secs(30)))
    }
}

/// Lee lineas con tope de largo, de cantidad y de tiempo.
struct Lector {
    leidas: u32,
    fin: Instant,
}

impl Lector {
    /// La siguiente linea, o `None` si el otro colgo.
    fn leer<T: Tubo>(&mut self, t: &mut T, espera: Duration) -> Result<Option<Vec<u8>>, Corte> {
        self.leidas += 1;
        if self.leidas > LINEAS_MAX {
            return Err(Corte::Fuera("demasiadas lineas"));
        }
        let queda = self.fin.saturating_duration_since(Instant::now());
        if queda.is_zero() {
            return Err(Corte::Fuera("la charla paso de su tiempo"));
        }
        t.plazo(espera.min(queda))?;
        let mut linea = Vec::new();
        let mut b = [0u8; 1];
        loop {
            if t.read(&mut b)? == 0 {
                return Ok(None);
            }
            if b[0] == b'\n' {
                return Ok(Some(linea));
            }
            linea.push(b[0]);
            if linea.len() > LINEA_MAX + 1 {
                return Err(Corte::Fuera("linea demasiado larga"));
            }
        }
    }
}

fn enviar<T: Tubo>(t: &mut T, linea: &str) -> io::Result<()> {
    t.write_all(linea.as_bytes())?;
    t.write_all(b"\n")
}

/// **Juzga una lamina entera** con el juez de BMO-X. `Ok` con sus lineas
/// (sin `\r`), o el motivo.
pub fn juzgar(datos: &[u8]) -> Result<Vec<&[u8]>, String> {
    let mut lineas: Vec<&[u8]> = datos.split(|&c| c == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)).collect();
    if lineas.last() == Some(&&b""[..]) {
        lineas.pop();
    }
    let mut juez = bmo_antena::lamina::Lector::nuevo();
    for (k, l) in lineas.iter().enumerate() {
        juez.empujar(l).map_err(|r| format!("linea {}: {}", k + 1, r.texto()))?;
    }
    if !juez.completa() {
        return Err("faltan lineas: la cabecera anuncia mas".into());
    }
    Ok(lineas)
}

fn servir_lamina<T: Tubo>(t: &mut T, datos: &[u8]) -> io::Result<()> {
    match juzgar(datos) {
        Ok(lineas) => {
            for l in lineas {
                t.write_all(l)?;
                t.write_all(b"\n")?;
            }
            Ok(())
        }
        Err(m) => enviar(t, &format!("NO la lamina no vale: {}", limpio(&m))),
    }
}

/// **Una conversacion entera.**
pub fn atender<T: Tubo>(t: &mut T, a: &Ajuste) -> Result<(), Corte> {
    let mut l = Lector { leidas: 0, fin: Instant::now() + CHARLA };
    match l.leer(t, SALUDO)? {
        Some(x) if leer_pedido(&x) == Ok(Pedido::Hola) => {}
        _ => return Err(Corte::Fuera("no empezo con HOLA ANTENA/1")),
    }
    enviar(t, &format!("HOLA {} {}", core::str::from_utf8(VERSION).unwrap_or("ANTENA/1"), limpio(&a.nombre)))?;
    loop {
        let Some(linea) = l.leer(t, CHARLA)? else { return Ok(()) };
        match leer_pedido(&linea) {
            Ok(Pedido::Lista) => {
                let lista = catalogo(&a.carpeta);
                enviar(t, &format!("LISTA {}", lista.len()))?;
                for (id, fichero) in lista {
                    enviar(t, &format!("ENTRADA {id} {}", limpio(&fichero)))?;
                }
            }
            Ok(Pedido::Pide(id)) => {
                let id = String::from_utf8_lossy(id).into_owned();
                let ruta = catalogo(&a.carpeta).into_iter().find(|(i, _)| *i == id).and_then(|(_, f)| dentro(&a.carpeta, &f));
                let Some(ruta) = ruta else {
                    enviar(t, "NO no hay nada con ese id")?;
                    continue;
                };
                if id.starts_with('p') {
                    servir_lamina(t, &std::fs::read(&ruta)?)?;
                    continue;
                }
                crate::video::servir(t, &ruta, &a.ffmpeg)?;
                return Ok(());
            }
            Ok(Pedido::Pagina(_)) => {
                // Navegar es de la WebView, con la app chica (AO2). No se finge.
                enviar(t, "NO la antena en Rust aun no navega (AO2): usa antena.py con --navegador")?;
            }
            Ok(Pedido::Hola) => return Err(Corte::Fuera("un segundo HOLA")),
            Err(_) => return Err(Corte::Fuera("orden fuera de ANTENA/1")),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::carpeta::pruebas::carpeta_de;
    use std::io::Cursor;
    use std::net::{IpAddr, Ipv4Addr};

    /// Un tubo de mentira: lo que dice BMO-X entra de `entra`, lo que dice la
    /// antena queda en `sale`.
    struct Mentira {
        entra: Cursor<Vec<u8>>,
        sale: Vec<u8>,
    }

    impl Read for Mentira {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.entra.read(b)
        }
    }
    impl Write for Mentira {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.sale.write(b)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Tubo for Mentira {
        fn plazo(&mut self, _d: Duration) -> io::Result<()> {
            Ok(())
        }
    }

    const LAMINA: &[u8] = b"LAMINA 640 200 2\nCAJA 0 0 640 40 102030\nTEXTO 8 8 1 ffffff hola desde la antena\n";

    fn ajuste(nombre: &str) -> Ajuste {
        let d = carpeta_de(nombre);
        std::fs::write(d.join("gato.mp4"), b"no es video").unwrap();
        std::fs::write(d.join("buena.lamina"), LAMINA).unwrap();
        std::fs::write(d.join("mala.lamina"), b"LAMINA 640 200 1\nCAJA 600 0 100 40 102030\n").unwrap();
        Ajuste { carpeta: d, permitir: IpAddr::V4(Ipv4Addr::LOCALHOST), nombre: "honor".into(), ffmpeg: "/no/existe/ffmpeg".into() }
    }

    fn charla(nombre: &str, dice: &[u8]) -> (Result<(), Corte>, String) {
        let a = ajuste(nombre);
        let mut t = Mentira { entra: Cursor::new(dice.to_vec()), sale: Vec::new() };
        let r = atender(&mut t, &a);
        (r, String::from_utf8_lossy(&t.sale).into_owned())
    }

    #[test]
    fn hola_y_lista() {
        let (r, sale) = charla("lista", b"HOLA ANTENA/1\nLISTA\n");
        assert!(r.is_ok());
        assert_eq!(sale, "HOLA ANTENA/1 honor\nLISTA 3\nENTRADA v1 gato.mp4\nENTRADA p1 buena.lamina\nENTRADA p2 mala.lamina\n");
    }

    #[test]
    fn lo_que_dice_la_antena_lo_acepta_bmo_x() {
        // El otro lado del MISMO crate: la conversacion de BMO-X oye a esta
        // antena entera y no rechaza ni una linea.
        let (_, sale) = charla("bmo-x", b"HOLA ANTENA/1\nLISTA\nPIDE p1\n");
        let mut c = bmo_antena::Conversacion::default();
        let mut lineas = sale.lines();
        c.pedir(&Pedido::Hola).unwrap();
        c.oir(lineas.next().unwrap().as_bytes()).unwrap();
        c.pedir(&Pedido::Lista).unwrap();
        for _ in 0..4 {
            c.oir(lineas.next().unwrap().as_bytes()).unwrap();
        }
        c.pedir(&Pedido::Pide(b"p1")).unwrap();
        for l in lineas {
            c.oir(l.as_bytes()).unwrap();
        }
        assert_eq!(c.fase(), bmo_antena::Fase::Charla, "la lamina entro entera y se vuelve a la charla");
    }

    #[test]
    fn una_lamina_que_bmo_x_rechazaria_no_sale_de_la_antena() {
        let (r, sale) = charla("mala", b"HOLA ANTENA/1\nPIDE p2\n");
        assert!(r.is_ok());
        assert!(sale.ends_with("\n") && sale.lines().nth(1).unwrap().starts_with("NO la lamina no vale: linea 2:"), "{sale}");
        assert!(!sale.contains("CAJA"), "ni una linea de la lamina mala");
    }

    #[test]
    fn sin_hola_no_hay_ni_una_palabra() {
        for dice in [&b"LISTA\n"[..], b"GET / HTTP/1.1\n", b"HOLA ANTENA/2\n", b""] {
            let (r, sale) = charla("sin-hola", dice);
            assert!(matches!(r, Err(Corte::Fuera(_))), "{:?}", String::from_utf8_lossy(dice));
            assert!(sale.is_empty());
        }
    }

    #[test]
    fn la_primera_orden_rara_cuelga() {
        for dice in [&b"HOLA ANTENA/1\nPIDE ../secreto\n"[..], b"HOLA ANTENA/1\nBORRA todo\n", b"HOLA ANTENA/1\nHOLA ANTENA/1\n"] {
            let (r, _) = charla("rara", dice);
            assert!(matches!(r, Err(Corte::Fuera(_))));
        }
    }

    #[test]
    fn lo_que_no_esta_o_no_se_puede_se_dice() {
        let (_, sale) = charla("no", b"HOLA ANTENA/1\nPIDE v9\nPAGINA https://example.org\nPIDE v1\n");
        let l: Vec<&str> = sale.lines().collect();
        assert_eq!(l[1], "NO no hay nada con ese id");
        assert!(l[2].starts_with("NO la antena en Rust aun no navega"));
        assert_eq!(l[3], "NO no hay ffmpeg en la antena (pkg install ffmpeg)");
    }

    #[test]
    fn la_antena_que_no_para_de_oir() {
        let mut dice = b"HOLA ANTENA/1\n".to_vec();
        for _ in 0..LINEAS_MAX + 4 {
            dice.extend_from_slice(b"PIDE v9\n");
        }
        let (r, _) = charla("muchas", &dice);
        assert!(matches!(r, Err(Corte::Fuera("demasiadas lineas"))));
        let mut largo = b"HOLA ANTENA/1\n".to_vec();
        largo.extend(std::iter::repeat(b'A').take(LINEA_MAX + 10));
        assert!(matches!(charla("larga", &largo).0, Err(Corte::Fuera("linea demasiado larga"))));
    }
}
