//! HSA1: inspeccion anfitrion del contrato de un booter, con el lector de BMO.
//! Solo lee ficheros. No carga firmware ni consulta fusibles de la GPU.
//! Uso: contrato_hs FICHERO [FUSIBLE_CRUDO_HEX]
use bmo_gpu_ga10x::arranque::booter;
use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().collect();
    if !(2..=3).contains(&args.len()) {
        return Err("uso: contrato_hs FICHERO [FUSIBLE_CRUDO_HEX]".into());
    }
    let bytes = fs::read(&args[1])?;
    let b = booter::booter(&bytes).map_err(|e| format!("booter rechazado: {e:?}"))?;
    println!("AMBITO=anfitrion; no prueba autenticacion BROM ni ejecucion HS");
    println!("bytes={} version={} ucode_offset={:#x} ucode_bytes={:#x}", bytes.len(), b.bin.version, b.bin.datos, b.bin.bytes);
    println!("firmas_offset={:#x} firmas={} firma_bytes={:#x} parche_ucode={:#x}", b.firmas_en, b.firmas, b.firma_bytes, b.parche);
    println!("fuse_ver={} engine_mask={:#x} ucode_id={}", b.fuse_ver, b.engine_id_mask, b.ucode_id);
    println!("IMEM origen={:#x} destino={:#x} bytes={:#x}", b.imem.origen, b.imem.destino, b.imem.bytes);
    println!("DMEM origen={:#x} destino={:#x} bytes={:#x}", b.dmem.origen, b.dmem.destino, b.dmem.bytes);
    println!("BROM_PARAADDR={:#x} arranque={:#x}", b.pkc_data_offset(), b.arranque());
    if let Some(raw) = args.get(2) {
        let fuse = u32::from_str_radix(raw.trim_start_matches("0x"), 16)?;
        let idx = b.indice_de_firma(fuse).ok_or("el fusible declarado no tiene firma compatible")?;
        let mut copia = b.bin.ucode(&bytes).to_vec();
        booter::firmar(&bytes, &b, idx, &mut copia).map_err(|e| format!("firma rechazada: {e:?}"))?;
        println!("fusible=DECLARADO:{fuse:#x} indice={idx} insercion_en_copia=OK; no verificacion criptografica");
    } else {
        println!("fusible=ausente indice=desconocido; no se presume fusible cero");
    }
    Ok(())
}
