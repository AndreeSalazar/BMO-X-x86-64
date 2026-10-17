//! La lente recorre los programas reales del adaptador, cabecera incluida
//! en hex/bits pero excluida de la interpretacion SASS.
use bmo_gpu_ga10x::{raster, sass::{juez, lente}};
use bmo_verrano::lens::{self, Capture, Origin, Snapshot, Space};

#[test]
fn real_programs_roundtrip_and_the_reader_agrees_with_judge_coverage() {
    for program in [include_bytes!("../sombreadores/bmox12_vs.sm86").as_slice(), include_bytes!("../sombreadores/bmox12_ps.sm86").as_slice()] {
        let mut offset = 0;
        let mut hex_bytes = Vec::new();
        let mut bit_bytes = Vec::new();
        loop {
            let n = lens::page(program.len() as u64, offset).unwrap();
            let bytes = &program[offset as usize..offset as usize + n];
            let snap = Snapshot::new(Origin::Prepared, Space::Resource, 0, program.len() as u64, offset, Capture::Bytes(bytes)).unwrap();
            let mut text = String::new(); snap.write(&mut text).unwrap();
            for row in text.lines().filter(|l| l.starts_with('+')) {
                let (hex, bits) = row.split_once("| ").unwrap();
                hex_bytes.extend(hex.split_whitespace().skip(1).map(|b| u8::from_str_radix(b, 16).unwrap()));
                bit_bytes.extend(bits.split_whitespace().map(|b| u8::from_str_radix(b, 2).unwrap()));
            }
            if let Some(next) = snap.next() { offset = next; } else { break; }
        }
        assert_eq!(hex_bytes, program); assert_eq!(bit_bytes, program);
        let code = &program[4 * raster::SPH..];
        for (k, bytes) in code.chunks_exact(lente::INSTRUCTION_BYTES).enumerate() {
            let i = lente::read(bytes, (k * 16) as u64).unwrap();
            assert_eq!(i.decoded.is_some(), juez::conoce(i.lo, i.hi));
            assert!(i.decoded.is_some());
            let mut raw = Vec::from(i.lo.to_le_bytes()); raw.extend(i.hi.to_le_bytes());
            assert_eq!(raw, bytes);
        }
        assert!(code.chunks_exact(16).remainder().is_empty());
    }
}
