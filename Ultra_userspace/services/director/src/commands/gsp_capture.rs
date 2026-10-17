//! Exportacion privada de una captura y su copia LIBOS, con manifiesto final.
//! [consumo] NADA -- solo al acabar despertar; ninguna orden nueva de GPU
use crate::scene::output::Output;
use bmo_nvidia_gsp_control::capture as c;
use bmo_userland as bmo;
use core::sync::atomic::{AtomicU64, Ordering};

static EXPORTED: AtomicU64 = AtomicU64::new(0);
const LOG_BYTES: usize = 196608;

fn header(index: u64) -> Option<u64> {
    let query = bmo::INFO_GPU_DESPIERTO_BUZON | c::SELECTOR << 8 | index << 16;
    c::decode(bmo::info(query), bmo::info(query | 1 << 32))
}
fn word(slot: u64, index: u64) -> Option<u64> {
    let query = bmo::INFO_GPU_DESPIERTO_BUZON | (c::SELECTOR + 1) << 8 | slot << 16;
    c::decode(
        bmo::info(query | (index * 2) << 24),
        bmo::info(query | (index * 2 + 1) << 24),
    )
}
fn log_word(offset: u64) -> Option<u64> {
    let query = bmo::INFO_GPU_DESPIERTO_BUZON | (c::SELECTOR + 3) << 8;
    let part = offset / 8 * 2;
    c::decode(
        bmo::info(query | part << 16),
        bmo::info(query | (part + 1) << 16),
    )
}
pub(super) fn puts() -> Option<[u64; 3]> {
    Some([log_word(0)?, log_word(65536)?, log_word(131072)?])
}
fn prepared() -> bool {
    bmo::info(bmo::INFO_GPU_LIBOS) & bmo::LIBOS_PREPARADO != 0
}

struct Path {
    bytes: [u8; 128],
    n: usize,
}
impl Path {
    fn new(session: [u64; 2], attempt: u64) -> Self {
        let mut p = Self {
            bytes: [0; 128],
            n: 0,
        };
        p.add(b"informe/gpu");
        // Componentes 8.3, los 128 bits observados completos; no truncar SID.
        for v in [
            session[0] >> 32,
            session[0],
            session[1] >> 32,
            session[1],
            attempt,
        ] {
            p.add(b"/");
            for shift in (0..8).rev() {
                p.bytes[p.n] = b"0123456789ABCDEF"[((v >> (shift * 4)) & 15) as usize];
                p.n += 1;
            }
        }
        p
    }
    fn add(&mut self, s: &[u8]) {
        self.bytes[self.n..self.n + s.len()].copy_from_slice(s);
        self.n += s.len();
    }
    fn file(&mut self, name: &[u8]) {
        self.add(b"/");
        self.add(name);
    }
    fn slice(&self) -> &[u8] {
        &self.bytes[..self.n]
    }
}
fn write(path: &[u8], bytes: &[u8]) -> bool {
    let Ok(f) = bmo::Archivo::create(path) else {
        return false;
    };
    let complete = f.write(bytes) == bytes.len();
    f.close() && complete
}
fn write_block(path: &[u8], block: &bmo::Memoria, bytes: u64) -> bool {
    let Ok(f) = bmo::Archivo::create(path) else {
        return false;
    };
    let complete = f.escribir_de(block, 0, bytes) == bytes;
    f.close() && complete
}

/// Ningun archivo compartido se usa como identidad: cada conjunto vive bajo
/// SID/intento. listo.bin se escribe ULTIMO, despues de cerrar los otros tres.
pub(super) fn save() -> bool {
    if header(0) != Some(c::MAGIC) {
        return false;
    }
    let Some(count) = header(1) else { return false };
    if count == 0 || count > c::CAPACITY as u64 {
        return false;
    }
    let (Some(date), Some(tsc)) = (header(3), header(4)) else {
        return false;
    };
    let session = [date, tsc];
    let attempt = bmo::info(bmo::INFO_GPU_INTENTO);
    if attempt == 0 || attempt > u32::MAX as u64 || session[1] == 0 {
        return false;
    }
    if word(count - 1, 4) != Some(attempt) {
        return false;
    }
    let mut p = Path::new(session, attempt);
    let base = p.n;
    // No pisar ni siquiera un conjunto parcial de una ejecucion anterior.
    // ERROR_DIR_NO_ESTA=26, contrato de obj/directory::open. Cualquier otro
    // error (incluida tabla llena) impide crear; no se confunde con ausencia.
    match bmo::Directorio::open(p.slice()) {
        Err(26) => {}
        _ => return false,
    }
    let Ok(_dir) = bmo::Directorio::crear(p.slice()) else {
        return false;
    };
    let Some(records) = bmo::Memoria::request(count * c::WORDS as u64 * 8) else {
        return false;
    };
    let Some(logs) = bmo::Memoria::request(LOG_BYTES as u64) else {
        return false;
    };
    // SAFETY: bloques privados recien reservados, medidas solicitadas exactas;
    // no hay punteros GPU ni se entregan al kernel como autoridad de aparato.
    let data =
        unsafe { core::slice::from_raw_parts_mut(records.base(), count as usize * c::WORDS * 8) };
    let bytes = unsafe { core::slice::from_raw_parts_mut(logs.base(), LOG_BYTES) };
    for i in 0..count {
        for j in 0..c::WORDS {
            let Some(value) = word(i, j as u64) else {
                return false;
            };
            data[(i as usize * c::WORDS + j) * 8..(i as usize * c::WORDS + j + 1) * 8]
                .copy_from_slice(&value.to_le_bytes());
        }
    }
    let mut meta = [0u64; 20];
    let start = bmo::ciclos();
    let reserve_before = prepared();
    let before = if reserve_before {
        let Some(p) = puts() else { return false };
        p
    } else {
        [0; 3]
    };
    if reserve_before {
        for o in (0..LOG_BYTES).step_by(8) {
            let Some(v) = log_word(o as u64) else {
                return false;
            };
            bytes[o..o + 8].copy_from_slice(&v.to_le_bytes());
        }
    }
    let reserve_after = prepared();
    let after = if reserve_after {
        let Some(p) = puts() else { return false };
        p
    } else {
        [0; 3]
    };
    let end = bmo::ciclos();
    // Identidad cambiante: conjunto parcial sin listo.bin, nunca acreditado.
    if bmo::info(bmo::INFO_GPU_INTENTO) != attempt
        || header(1) != Some(count)
        || [header(3), header(4)] != [Some(date), Some(tsc)]
    {
        return false;
    }
    let log_bytes = if reserve_before && reserve_after {
        LOG_BYTES
    } else {
        0
    };
    let (Some(build), Some(lost)) = (word(count - 1, 3), header(2)) else {
        return false;
    };
    meta[..13].copy_from_slice(&[
        c::READY,
        session[0],
        session[1],
        build,
        crate::commands::obs_build::DIRECTOR,
        attempt,
        count,
        log_bytes as u64,
        start,
        end,
        reserve_before as u64
            | (reserve_after as u64) << 1
            | ((reserve_before && reserve_after && before == after) as u64) << 2
            | 8
            | 16,
        count,
        lost,
    ]);
    meta[13..16].copy_from_slice(&before);
    meta[16..19].copy_from_slice(&after);
    meta[19] = c::WORDS as u64;
    let mut manifest = [0u8; 256];
    for (i, v) in meta.iter().enumerate() {
        manifest[i * 8..i * 8 + 8].copy_from_slice(&v.to_le_bytes());
    }
    manifest[160..192].copy_from_slice(&bmo_hash::hash(data));
    manifest[192..224].copy_from_slice(&bmo_hash::hash(&bytes[..log_bytes]));
    // Informe dedicado: la misma captura, no una consulta viva posterior.
    // 32 lineas de 86 bytes y cabecera <256: caben en 4096, sin truncar.
    let mut text = [0u8; 4096];
    let mut n = 0;
    for label in [
        b"BMO-GSP captura v1\n" as &[u8],
        b"session-date session-tsc build attempt sequence (hex):\n",
    ] {
        text[n..n + label.len()].copy_from_slice(label);
        n += label.len();
    }
    for v in [session[0], session[1], build, attempt, count] {
        for shift in (0..16).rev() {
            text[n] = b"0123456789ABCDEF"[((v >> (shift * 4)) & 15) as usize];
            n += 1;
        }
        text[n] = b' ';
        n += 1;
    }
    let label = b"\nphase attempt present-mask pri-mask sequence (hex):\n";
    text[n..n + label.len()].copy_from_slice(label);
    n += label.len();
    for r in data.chunks_exact(c::WORDS * 8) {
        for index in [5, 4, 8, 9, 10] {
            let v = u64::from_le_bytes(r[index * 8..index * 8 + 8].try_into().unwrap());
            for shift in (0..16).rev() {
                text[n] = b"0123456789ABCDEF"[((v >> (shift * 4)) & 15) as usize];
                n += 1;
            }
            text[n] = b' ';
            n += 1;
        }
        text[n] = b'\n';
        n += 1;
    }
    // El lector anfitrion muestra fases y validez del detalle binario.
    manifest[224..256].copy_from_slice(&bmo_hash::hash(&text[..n]));
    p.file(b"captura.bin");
    if !write_block(p.slice(), &records, data.len() as u64) {
        return false;
    }
    p.n = base;
    if log_bytes != 0 {
        p.file(b"gsplog.bin");
        if !write_block(p.slice(), &logs, log_bytes as u64) {
            return false;
        }
        p.n = base;
    }
    p.file(b"informe.txt");
    if !write(p.slice(), &text[..n]) {
        return false;
    }
    p.n = base;
    p.file(b"listo.bin");
    if !write(p.slice(), &manifest) {
        return false;
    }
    EXPORTED.store(count, Ordering::Release);
    true
}

pub(super) fn row(s: &mut Output) {
    if header(0) != Some(c::MAGIC) {
        return;
    }
    s.text(b"  captura GSP: ");
    let (Some(count), Some(lost)) = (header(1), header(2)) else {
        s.text(b"lectura de diario no disponible\n");
        return;
    };
    s.dec(count);
    s.text(b" fases publicadas; perdidas ");
    s.dec(lost);
    let saved = EXPORTED.load(Ordering::Acquire);
    if saved != 0 {
        let (Some(attempt), Some(date), Some(tsc)) = (word(saved - 1, 4), header(3), header(4))
        else {
            s.text(b"; identidad del guardado no disponible\n");
            return;
        };
        s.text(b"; ultimo conjunto cerrado del intento ");
        s.dec(attempt);
        if bmo::info(bmo::INFO_GPU_INTENTO) != attempt {
            s.text(b" (ANTERIOR, no atribuir al intento actual)");
        }
        s.text(b": ");
        let p = Path::new([date, tsc], attempt);
        s.text(p.slice());
        s.text(b" (verificar listo.bin fuera de linea)");
    } else {
        s.text(b"; sin conjunto cerrado por este Director");
    }
    s.byte(b'\n');
}
