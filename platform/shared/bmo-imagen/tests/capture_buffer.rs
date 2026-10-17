//! El reparto real del DIRECTOR, probado sin kernel ni captura del metal.
#[path = "../../../../Ultra_userspace/services/director/src/desktop/capture_layout.rs"]
mod capture_layout;

use bmo_imagen::png_escribir as png;
use capture_layout::Layout;

#[test]
fn hd_capture_fits_the_last_memory_slot() {
    for (w, h) in [(1920, 1080), (3840, 2160), (1, 1)] {
        let l = Layout::new(w, h).unwrap();
        assert_eq!(l.output_bytes, 0, "con siete ranuras usadas solo queda una");
        assert_eq!(l.png_offset, l.workshop + l.raw);
        assert_eq!(l.work_bytes, l.png_offset + l.capacity);
        assert!(l.work_bytes <= 64 * 1024 * 1024);
    }
}

#[test]
fn large_capture_keeps_the_previous_two_block_path() {
    let l = Layout::new(4096, 4096).unwrap();
    assert_eq!(l.png_offset, 0);
    assert_eq!(l.output_bytes, l.capacity);
    assert_eq!(l.work_bytes, l.workshop + l.raw);
    assert!(l.work_bytes <= 64 * 1024 * 1024 && l.output_bytes <= 64 * 1024 * 1024);
    for (w, h) in [(0, 1), (1, 0), (u32::MAX, u32::MAX), (4097, 1)] {
        assert!(Layout::new(w, h).is_none());
    }
}

#[test]
fn single_block_png_round_trip_and_file_offset() {
    for (w, h) in [(1920, 1080), (256, 131), (1, 1)] {
        let l = Layout::new(w, h).unwrap();
        let mut block = vec![0xA5A5_A5A5u32; l.work_bytes.div_ceil(4) + 1];
        let color = |x: u32, y: u32| (x % 256) << 16 | (y % 256) << 8 | ((x / 16 + y / 16) % 256);
        let n = {
            let (workshop, rest) = block.split_at_mut(l.workshop / 4);
            // SAFETY: rest es propio, alineado y su medida proviene del Vec.
            let bytes = unsafe { core::slice::from_raw_parts_mut(rest.as_mut_ptr() as *mut u8, rest.len() * 4) };
            let (raw, tail) = bytes.split_at_mut(l.raw);
            let dst = &mut tail[..l.capacity];
            for y in 0..h {
                for (x, pixel) in png::fila(raw, w, y).chunks_exact_mut(3).enumerate() {
                    let c = color(x as u32, y);
                    pixel.copy_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
                }
            }
            let mut codec = png::Codificador::nuevo(w, h, raw, workshop, dst).unwrap();
            loop {
                if let png::Estado::Hecho(n) = codec.avanzar(raw, workshop, dst, 65536).unwrap() { break n; }
            }
        };
        assert_eq!(*block.last().unwrap(), 0xA5A5_A5A5);
        // El fichero recibe SOLO el PNG desde su offset, nunca el taller.
        let bytes = unsafe { core::slice::from_raw_parts(block.as_ptr() as *const u8, l.work_bytes) };
        let file = &bytes[l.png_offset..l.png_offset + n];
        assert_eq!(&file[..8], b"\x89PNG\r\n\x1a\n");
        let mut pixels = vec![0; (w * h) as usize];
        let mut decoder = vec![0; bmo_imagen::TALLER];
        bmo_imagen::decodificar_con(file, &mut pixels, &mut decoder).unwrap();
        for y in 0..h {
            for x in 0..w { assert_eq!(pixels[(y * w + x) as usize], 0xFF00_0000 | color(x, y)); }
        }
    }
}
