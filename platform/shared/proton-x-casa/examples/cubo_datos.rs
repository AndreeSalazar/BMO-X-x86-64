//! **Fabrica `proton-x/prueba/cubo_datos.h`** (P3b2, 27-09): los datos del
//! cubo de X1 para `cubo.exe`, con los BITS exactos de `bmo-cubo`.
//!
//! `cubo.exe` no calcula sus matrices: las toma de aqui, fotograma a
//! fotograma, a 1280x720 (la medida de las huellas de D3D12 en la 3060). Asi
//! lo que sube a su bufer de constantes es, bit a bit, lo que uso X4, y el
//! banco puede comparar lo que la tuberia de la casa captura con `bmo-cubo`.
//! Y lleva dentro los dos sombreadores (`cubo_vs.dxil`, `cubo_ps.dxil`, los de
//! `dxc`, tal cual): el `.exe` no lee ficheros, y en Windows corre solo.
//!
//!     cargo run -p bmo-proton-x-casa --example cubo_datos > platform/shared/proton-x/prueba/cubo_datos.h
//!
//! `tests/corre.rs` comprueba que el `.h` del arbol sigue siendo esta salida.

use std::fmt::Write;

pub fn texto() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "/* cubo_datos.h -- FABRICADO por `cargo run -p bmo-proton-x-casa --example cubo_datos`:");
    let _ = writeln!(s, " * el cubo de X1 (bmo-cubo) en bits: 24 vertices (pos, normal, color: 10 floats),");
    let _ = writeln!(s, " * 36 indices y las constantes (wvp, world, luz: 36 floats) de los 360 fotogramas");
    let _ = writeln!(s, " * a 1280x720, y los dos sombreadores de dxc (cubo_vs.dxil, cubo_ps.dxil) tal cual.");
    let _ = writeln!(s, " * No se edita a mano: el banco lo compara con bmo-cubo y con los .dxil. */");
    let _ = writeln!(s, "#define CUBO_ANCHO 1280");
    let _ = writeln!(s, "#define CUBO_ALTO 720");
    let v = bmo_cubo::vertices();
    let _ = writeln!(s, "static const unsigned int CUBO_VERTICES[{}] = {{", v.len() * 10);
    for x in &v {
        let f: Vec<String> = x.pos.iter().chain(&x.normal).chain(&x.color).map(|f| format!("0x{:08x}", f.to_bits())).collect();
        let _ = writeln!(s, "    {},", f.join(", "));
    }
    let _ = writeln!(s, "}};");
    let i = bmo_cubo::indices();
    let _ = writeln!(s, "static const unsigned short CUBO_INDICES[{}] = {{", i.len());
    for t in i.chunks(6) {
        let f: Vec<String> = t.iter().map(|x| x.to_string()).collect();
        let _ = writeln!(s, "    {},", f.join(", "));
    }
    let _ = writeln!(s, "}};");
    let _ = writeln!(s, "static const unsigned int CUBO_CONSTANTES[360][36] = {{");
    for f in 0..360u32 {
        let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
        let todos: Vec<String> = c.wvp.iter().chain(&c.world).chain(&c.luz).map(|x| format!("0x{:08x}", x.to_bits())).collect();
        let _ = writeln!(s, "    {{{}}},", todos.join(","));
    }
    let _ = writeln!(s, "}};");
    for (nombre, d) in [("CUBO_VS", VS), ("CUBO_PS", PS)] {
        let _ = writeln!(s, "static const unsigned char {}[{}] = {{", nombre, d.len());
        for t in d.chunks(20) {
            let f: Vec<String> = t.iter().map(|x| format!("0x{x:02x}")).collect();
            let _ = writeln!(s, "    {},", f.join(","));
        }
        let _ = writeln!(s, "}};");
    }
    s
}

const VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");

#[allow(dead_code)]
fn main() {
    print!("{}", texto());
}
