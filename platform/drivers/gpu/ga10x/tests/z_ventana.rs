//! MUNDO: destino 640x360 con Z, en las dos rutas de la lamina.
use bmo_bsf::{abi, kind, Bsf};
use bmo_gpu_ga10x::{
    destino::{Destino, Flotante},
    profundidad::{UNO, Z},
    tuberia as tu,
};

#[test]
fn mundo_con_z_cabe_en_su_ventana() {
    let bytes = include_bytes!("../sombreadores/cubo.bsf");
    let bsf = Bsf::parse(bytes).unwrap();
    bsf.verify_all().unwrap();
    let vs = bsf
        .module(0)
        .target(kind::SM86, abi::SM86_V1, 0)
        .unwrap()
        .code()
        .unwrap();
    let ps = bsf
        .module(1)
        .target(kind::SM86, abi::SM86_V1, 0)
        .unwrap()
        .code()
        .unwrap();
    let dst = Destino {
        fila: 2560,
        ancho: 640,
        alto: 360,
        rgb: false,
        flotante: Flotante::No,
    };
    let d = tu::Dibujo {
        vertices: 492,
        destino: Some((0x1000_0000, dst)),
        color: Some(0xFF87_ADE8),
        z: Some(Z {
            funcion: 2,
            escribir: true,
            limpiar: Some(UNO),
        }),
        descarte: tu::Descarte::Traseras,
        ..tu::Dibujo::default()
    };
    let mut out = vec![0; tu::MAX_PAQUETE];
    let datos = vec![0; 492 * tu::BYTES_VERTICE];
    let n = tu::escribir_paquete_dibujo(&mut out, 1, vs, ps, 492, &datos, d)
        .expect("MUNDO copiado con Z");
    let p = tu::leer(&out[..n]).unwrap();
    assert_eq!(p.dibujo.destino, d.destino);
    assert_eq!(p.dibujo.z, d.z);
    let n = tu::escribir_paquete_prestado(&mut out, 2, vs, ps, 0x2000_0000, 492, d)
        .expect("MUNDO prestado con Z");
    let p = tu::leer(&out[..n]).unwrap();
    assert_eq!(p.dibujo.prestada.unwrap().bytes, datos.len() as u64);
    // El viewport usa la medida del juego; color y Z conservan su
    // superficie fisica fija, con el mismo paso y la misma reserva VRAM.
    let o = tu::ordenes_dibujo(&dst.ventana(), 164, true, d);
    let w = &o.o[..o.n];
    let cab = |metodo, n| bmo_gpu_ga10x::copia::cabecera_en(0, metodo, n);
    let clip = cab(bmo_gpu_ga10x::tresde::SET_SURFACE_CLIP_HORIZONTAL, 2);
    assert!(w.windows(3).any(|x| x == [clip, 640 << 16, 360 << 16]));
    let ct = cab(bmo_gpu_ga10x::tresde::SET_COLOR_TARGET_A0, 8);
    let k = w.iter().rposition(|&x| x == ct).unwrap();
    assert_eq!(&w[k + 3..k + 5], &[1280, 720]);
    let zt = cab(bmo_gpu_ga10x::profundidad::SET_ZT_SIZE_A, 3);
    assert!(w
        .windows(4)
        .any(|x| x == [zt, 1280, 720, bmo_gpu_ga10x::profundidad::UNA_CAPA]));
    for (ancho, alto, fila) in [
        (1280, 720, 5120),
        (1, 1, 128),
        (1281, 360, 5248),
        (640, 721, 2560),
    ] {
        let dst = Destino {
            ancho,
            alto,
            fila,
            ..dst
        };
        assert_eq!(
            tu::cabe(&tu::Dibujo {
                destino: Some((0x1000_0000, dst)),
                ..d
            }),
            ancho <= 1280 && alto <= 720
        );
    }
}
