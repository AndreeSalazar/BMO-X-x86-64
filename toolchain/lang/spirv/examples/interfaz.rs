//! La INTERFAZ de cada `.spv`: los buffers que el sombreador toca.
//!
//!     cargo run -p bmo-spirv-front --example interfaz -- pruebas/*.spv

fn main() {
    for ruta in std::env::args().skip(1) {
        let bytes = std::fs::read(&ruta).unwrap();
        let mut ids = vec![0u32; 1 << 16];
        let m = match bmo_spirv_front::read(&bytes, &mut ids) {
            Ok(m) => m,
            Err(e) => {
                println!("{ruta}: {e}");
                continue;
            }
        };
        match bmo_spirv_front::interface(&m) {
            Ok(i) => {
                println!("{ruta}: {} local_size {:?}", String::from_utf8_lossy(i.name), i.local_size);
                for b in i.bindings() {
                    let clase = if b.storage { "storage" } else { "uniform" };
                    println!("  set {} binding {} {} acceso {} base {} paso {}", b.set, b.binding, clase, b.access, b.base_bytes, b.stride);
                }
            }
            Err(e) => println!("{ruta}: {e}"),
        }
    }
}
