//! La antena de verdad, por TCP en el anfitrion: la IP permitida charla; las
//! demas no oyen ni una palabra.

use bmo_antena_movil::{servir, Ajuste};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};

fn ajuste(permitir: IpAddr) -> Ajuste {
    let d = std::env::temp_dir().join(format!("bmo-antena-tcp-{}-{}", permitir.is_loopback(), std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("gato.mp4"), b"x").unwrap();
    Ajuste { carpeta: d, permitir, nombre: "honor".into(), ffmpeg: "/no/existe".into() }
}

#[test]
fn la_ip_permitida_charla_por_tcp() {
    let escucha = TcpListener::bind("127.0.0.1:0").unwrap();
    let puerto = escucha.local_addr().unwrap().port();
    let a = ajuste(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let hilo = std::thread::spawn(move || servir(&escucha, &a, Some(1)).unwrap());
    let mut s = TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    s.write_all(b"HOLA ANTENA/1\nLISTA\n").unwrap();
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut l = String::new();
    for esperada in ["HOLA ANTENA/1 honor", "LISTA 1", "ENTRADA v1 gato.mp4"] {
        l.clear();
        r.read_line(&mut l).unwrap();
        assert_eq!(l.trim_end(), esperada);
    }
    drop(s);
    drop(r);
    hilo.join().unwrap();
}

#[test]
fn otra_ip_no_oye_ni_una_palabra() {
    let escucha = TcpListener::bind("127.0.0.1:0").unwrap();
    let puerto = escucha.local_addr().unwrap().port();
    // Una de documentacion (TEST-NET-3): la de este anfitrion seguro que no es.
    let a = ajuste(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)));
    let hilo = std::thread::spawn(move || servir(&escucha, &a, Some(1)).unwrap());
    let mut s = TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    let _ = s.write_all(b"HOLA ANTENA/1\n");
    let mut todo = Vec::new();
    let _ = s.read_to_end(&mut todo);
    assert!(todo.is_empty(), "la antena le hablo a una IP que no era la suya");
    hilo.join().unwrap();
}
