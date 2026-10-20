//! `fw-huella` -- F0b de `docs/plan/PLAN_GPU_COMO_LINUX.md`.
//!
//! ```text
//!    fw-huella CARPETA VERSION
//!        lee CARPETA/{booter_load,booter_unload,bootloader,gsp}-VERSION.bin
//! ```
//!
//! Lo que dice de cada uno sale de los lectores del driver (`booter`,
//! `elf`), los mismos que el kernel usa en L0c: si aqui no se entiende un
//! fichero, el kernel tampoco lo entendera. Del GSP-RM imprime el BLAKE3 de
//! su `.fwimage` en la forma de `HASH_570` de `ring0/dev/gpu_gsp.rs`, para
//! fijarlo alli.
//!
//! Solo lee. El firmware es de NVIDIA y no entra en el repo: vive en
//! `BMO-externo\firmware\nvidia\ga102\gsp\`.

use bmo_gpu_ga10x::{booter, elf};
use std::path::Path;

fn leer(carpeta: &Path, nombre: &str, version: &str) -> Vec<u8> {
    let p = carpeta.join(format!("{nombre}-{version}.bin"));
    std::fs::read(&p).unwrap_or_else(|e| {
        eprintln!("no puedo leer {}: {e}", p.display());
        std::process::exit(1)
    })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(carpeta), Some(version)) = (args.first(), args.get(1)) else {
        eprintln!("uso: fw-huella CARPETA VERSION   (p. ej. fw-huella BMO-externo/firmware/nvidia/ga102/gsp 615.78.08)");
        std::process::exit(2)
    };
    let carpeta = Path::new(carpeta);
    let mut roto = false;

    for nombre in ["booter_load", "booter_unload"] {
        let f = leer(carpeta, nombre, version);
        match booter::booter(&f) {
            Ok(b) => {
                println!("{nombre}-{version}.bin  {} B", f.len());
                println!("  bin       version {} cabecera {:#x} ucode {:#x}+{:#x}", b.bin.version, b.bin.cabecera, b.bin.datos, b.bin.bytes);
                println!("  firmas    {} de {} B desde {:#x}; parche en {:#x}", b.firmas, b.firma_bytes, b.firmas_en, b.parche);
                println!("  hs        fuse_ver {:#x} engine_id_mask {:#x} ucode_id {}", b.fuse_ver, b.engine_id_mask, b.ucode_id);
                println!("  imem      {:#x} -> {:#x}, {:#x} B", b.imem.origen, b.imem.destino, b.imem.bytes);
                println!("  dmem      {:#x} -> {:#x}, {:#x} B", b.dmem.origen, b.dmem.destino, b.dmem.bytes);
            }
            Err(e) => {
                println!("{nombre}-{version}.bin  NO SE ENTIENDE: {e:?}");
                roto = true;
            }
        }
    }

    let f = leer(carpeta, "bootloader", version);
    match booter::riscv(&f) {
        Ok(r) => {
            println!("bootloader-{version}.bin  {} B", f.len());
            println!("  riscv     version {} codigo {:#x} datos {:#x} manifiesto {:#x} app_version {:#x}", r.version, r.codigo, r.datos, r.manifiesto, r.app_version);
        }
        Err(e) => {
            println!("bootloader-{version}.bin  NO SE ENTIENDE: {e:?}");
            roto = true;
        }
    }

    let f = leer(carpeta, "gsp", version);
    let mut fuente: &[u8] = &f;
    match (elf::seccion(&mut fuente, elf::IMAGEN), elf::seccion(&mut fuente, elf::FIRMA_GA10X)) {
        (Ok(img), Ok(firma)) => {
            let d = bmo_hash::hash(&f[img.desde as usize..(img.desde + img.bytes) as usize]);
            println!("gsp-{version}.bin  {} B", f.len());
            println!("  .fwimage           {:#x} B desde {:#x}", img.bytes, img.desde);
            println!("  .fwsignature_ga10x {:#x} B desde {:#x}", firma.bytes, firma.desde);
            println!("  BLAKE3 .fwimage    {}", hex(&d));
            println!();
            println!("// para ring0/dev/gpu_gsp.rs:");
            println!("pub const HASH_{}: [u8; 32] = [", version.split('.').next().unwrap_or(version));
            for fila in d.chunks(16) {
                let v: Vec<String> = fila.iter().map(|x| format!("0x{x:02x}")).collect();
                println!("    {},", v.join(", "));
            }
            println!("];");
        }
        (a, b) => {
            println!("gsp-{version}.bin  NO SE ENTIENDE: {:?} {:?}", a.err(), b.err());
            roto = true;
        }
    }
    if roto {
        std::process::exit(1)
    }
}
