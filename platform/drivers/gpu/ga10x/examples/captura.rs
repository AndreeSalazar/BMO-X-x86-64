//! Lectura/comparacion anfitriona de conjuntos privados BMO-GSP v1.
//! Uso: cargo run -p bmo-gpu-ga10x --example captura -- CARPETA [OTRA_CARPETA]
use bmo_nvidia_gsp_control::capture::{self as c, Record};
use std::{error::Error, fs::File, io::Read, path::Path};

fn load(path: &Path, max: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut data = Vec::new();
    File::open(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > max {
        return Err("archivo mayor del limite".into());
    }
    Ok(data)
}
fn words(data: &[u8]) -> Result<Vec<u64>, Box<dyn Error>> {
    if data.len() % 8 != 0 {
        return Err("palabra truncada".into());
    }
    Ok(data
        .chunks_exact(8)
        .map(|w| u64::from_le_bytes(w.try_into().unwrap()))
        .collect())
}
#[derive(Debug)]
struct Set {
    meta: Vec<u64>,
    records: Vec<Record>,
    logs: Vec<u8>,
}
fn read(path: &Path) -> Result<Set, Box<dyn Error>> {
    let manifest = load(&path.join("listo.bin"), 256)?;
    if manifest.len() != 256 {
        return Err("conjunto sin cierre completo".into());
    }
    let m = words(&manifest[..160])?;
    if m[0] != c::READY
        || m[19] != c::WORDS as u64
        || m[2] == 0
        || m[5] == 0
        || m[6] == 0
        || m[6] > c::CAPACITY as u64
        || m[11] != m[6]
        || m[9] < m[8]
        || m[10] & !31 != 0
        || m[10] & 24 != 24
        || ![0, 196608].contains(&m[7])
    {
        return Err("identidad, version o limites invalidos".into());
    }
    let raw = load(&path.join("captura.bin"), c::CAPACITY * c::WORDS * 8)?;
    if raw.len() != m[11] as usize * c::WORDS * 8 {
        return Err("cuenta de capturas incorrecta".into());
    }
    let report = load(&path.join("informe.txt"), 4096)?;
    let logs = if m[7] == 0 {
        if path.join("gsplog.bin").exists() {
            return Err("binario presente donde se declaro ausencia".into());
        }
        Vec::new()
    } else {
        if m[10] & 3 != 3 {
            return Err("binario sin reserva valida".into());
        }
        load(&path.join("gsplog.bin"), 196608)?
    };
    if logs.len() != m[7] as usize {
        return Err("medida de logs incorrecta".into());
    }
    if (m[7] != 0) != (m[10] & 3 == 3)
        || m[10] & 4 != 0 && (m[10] & 3 != 3 || m[13..16] != m[16..19])
    {
        return Err("banderas de reserva/PUT contradictorias".into());
    }
    for (i, data) in [&raw, &logs, &report].into_iter().enumerate() {
        if bmo_hash::hash(data) != manifest[160 + i * 32..192 + i * 32] {
            return Err(
                "huella BLAKE3 no coincide: conjunto incompleto, alterado o mezclado".into(),
            );
        }
    }
    let all = words(&raw)?;
    let mut records = Vec::new();
    for (i, w) in all.chunks_exact(c::WORDS).enumerate() {
        let r = Record(w.try_into().unwrap());
        if !r.valid() || r.0[1..4] != m[1..4] || r.0[10] != i as u64 + 1 {
            return Err("registro invalido o de otra sesion/build".into());
        }
        for field in 0..c::FIELDS {
            let is_register = field < 20 || field == 25;
            match r.reading(field) {
                c::Reading::Value(v)
                    if is_register
                        && (v > u32::MAX as u64 || bmo_gpu_ga10x::es_error_pri(v as u32)) =>
                {
                    return Err("registro con validez contradictoria".into())
                }
                c::Reading::Pri(v)
                    if !is_register
                        || v > u32::MAX as u64
                        || !bmo_gpu_ga10x::es_error_pri(v as u32) =>
                {
                    return Err("clasificacion PRI contradictoria".into())
                }
                _ => {}
            }
        }
        if records.iter().any(|old: &Record| {
            old.0[4] == r.0[4]
                && (old.0[5] == r.0[5] || old.0[5] >= c::HALTED && r.0[5] >= c::HALTED)
        }) {
            return Err("fase/terminal duplicado dentro del intento".into());
        }
        records.push(r);
    }
    if records.last().unwrap().0[4] != m[5] {
        return Err("logs asociados a otro intento".into());
    }
    Ok(Set {
        meta: m,
        records,
        logs,
    })
}
fn phase(p: u64) -> &'static str {
    match p {
        c::BEFORE_FRTS => "antes FRTS",
        c::AFTER_FRTS => "despues FRTS",
        c::BEFORE_BOOTER => "antes booter",
        c::HALTED => "parada observada",
        c::WAIT_EXPIRED => "espera vencida",
        _ => "desconocida",
    }
}
fn reading(r: c::Reading) -> String {
    match r {
        c::Reading::Absent => "ausente".into(),
        c::Reading::Value(v) => format!("valido 0x{v:016x}"),
        c::Reading::Pri(v) => format!("error PRI crudo 0x{v:016x}"),
    }
}
fn show(s: &Set) {
    println!(
        "sesion {:016x}/{:016x}; build kernel {:016x} Director {:016x}; intento {}; secuencia {}",
        s.meta[1], s.meta[2], s.meta[3], s.meta[4], s.meta[5], s.meta[6]
    );
    println!(
        "perdidas {}; logs {} bytes; PUT antes {:x?}, despues {:x?}; flags copia {:x}",
        s.meta[12],
        s.logs.len(),
        (s.meta[10] & 1 != 0).then_some(&s.meta[13..16]),
        (s.meta[10] & 2 != 0).then_some(&s.meta[16..19]),
        s.meta[10]
    );
    println!("RTC/TSC es identidad observacional, no UUID garantizado; BLAKE3 no es firma.");
    for r in s.records.iter().filter(|r| r.0[4] == s.meta[5]) {
        println!(
            "  {} [TSC {}..{}] intento {}",
            phase(r.0[5]),
            r.0[6],
            r.0[7],
            r.0[4]
        );
        for (i, name) in c::NAMES.iter().enumerate() {
            println!("    {name}: {}", reading(r.reading(i)));
        }
    }
    for (name, copy) in ["LOGINIT", "LOGINTR", "LOGRM"]
        .into_iter()
        .zip(s.logs.chunks_exact(65536))
    {
        println!(
            "{name}: {:?}",
            bmo_gpu_ga10x::inspeccion::libos::resumen(copy)
        );
    }
    println!(
        "Lecturas secuenciales, no instantanea atomica. PUT cero no prueba ausencia de ejecucion."
    );
}
fn compare(a: &Set, b: &Set) -> String {
    if a.meta[1..3] == b.meta[1..3] && a.meta[5..7] == b.meta[5..7] {
        return "Misma identidad de captura: no son dos arranques independientes".into();
    }
    if a.meta[3] != b.meta[3]
        || a.meta[3] != a.meta[4]
        || b.meta[3] != b.meta[4]
        || a.meta[3] & (1 << 63) == 0
        || a.meta[3] & (1 << 62) != 0
    {
        return "Builds distintos, desconocidos o sucios: no es una comparacion con la misma build"
            .into();
    }
    if a.meta[12] != 0 || b.meta[12] != 0 {
        return "Diario incompleto: hay capturas perdidas".into();
    }
    for p in c::BEFORE_FRTS..=c::WAIT_EXPIRED {
        let ar = a
            .records
            .iter()
            .find(|r| r.0[4] == a.meta[5] && r.0[5] == p);
        let br = b
            .records
            .iter()
            .find(|r| r.0[4] == b.meta[5] && r.0[5] == p);
        match (ar, br) {
            (Some(x), Some(y)) => if let Some(i) = c::first_difference(x,y) {
                return format!("Primera diferencia observada por orden fase/campo: {}, {}: {} -> {}. No es causa demostrada ni instante interno del firmware.", phase(p),c::NAMES[i],reading(x.reading(i)),reading(y.reading(i)));
            },
            (None, None) => {},
            _ => return format!("Primera diferencia de cobertura: fase {} ausente en uno de los intentos. No inferir causa.",phase(p)),
        }
    }
    if a.logs != b.logs {
        let at = a
            .logs
            .iter()
            .zip(&b.logs)
            .position(|(x, y)| x != y)
            .unwrap_or(a.logs.len().min(b.logs.len()));
        return format!("Sin diferencias en campos observados; primera diferencia de logs en offset {at}. No es causa demostrada.");
    }
    "Sin diferencias en campos observados/logs; no demuestra igualdad de estados internos".into()
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(2..=3).contains(&args.len()) {
        return Err("uso: captura CARPETA [OTRA_CARPETA]".into());
    }
    let a = read(Path::new(&args[1]))?;
    show(&a);
    if let Some(p) = args.get(2) {
        let b = read(Path::new(p))?;
        show(&b);
        println!("{}", compare(&a, &b));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static ID: AtomicU64 = AtomicU64::new(0);
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture(session: u64, attempt: u64, value: u64) -> Fixture {
        let path = std::env::temp_dir().join(format!(
            "bmo-gsp-synthetic-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let mut raw = Vec::new();
        for (seq, phase) in [c::BEFORE_FRTS, c::AFTER_FRTS, c::BEFORE_BOOTER, c::HALTED]
            .into_iter()
            .enumerate()
        {
            let mut r = Record::new([123, session], 1 << 63 | 12345, attempt, phase, 10);
            r.set(4, if phase == c::HALTED { value } else { 0 }, false);
            r.0[10] = seq as u64 + 1;
            for w in r.0 {
                raw.extend_from_slice(&w.to_le_bytes());
            }
        }
        let report = b"SYNTHETIC ONLY";
        let logs = vec![0u8; 196608];
        let mut m = [0u8; 256];
        let mut meta = [0u64; 20];
        meta[..13].copy_from_slice(&[
            c::READY,
            123,
            session,
            1 << 63 | 12345,
            1 << 63 | 12345,
            attempt,
            4,
            196608,
            10,
            20,
            31,
            4,
            0,
        ]);
        meta[19] = c::WORDS as u64;
        for (i, w) in meta.into_iter().enumerate() {
            m[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
        }
        for (i, b) in [&raw[..], &logs[..], &report[..]].into_iter().enumerate() {
            m[160 + i * 32..192 + i * 32].copy_from_slice(&bmo_hash::hash(b));
        }
        for (name, bytes) in [
            ("captura.bin", &raw[..]),
            ("gsplog.bin", &logs[..]),
            ("informe.txt", &report[..]),
            ("listo.bin", &m[..]),
        ] {
            fs::write(path.join(name), bytes).unwrap();
        }
        Fixture(path)
    }
    #[test]
    fn detects_first_observed_change_and_duplicate_capture() {
        let a = fixture(456, 1, 0);
        let b = fixture(789, 1, 0x15);
        let x = read(&a.0).unwrap();
        let y = read(&b.0).unwrap();
        assert!(compare(&x, &y).contains("parada observada, sec2.mailbox0"));
        assert!(compare(&x, &x).contains("no son dos arranques"));
    }
    #[test]
    fn mixed_or_incomplete_files_are_rejected() {
        let a = fixture(456, 1, 0);
        let b = fixture(789, 1, 0x15);
        fs::copy(b.0.join("captura.bin"), a.0.join("captura.bin")).unwrap();
        assert!(read(&a.0).is_err());
        fs::remove_file(b.0.join("listo.bin")).unwrap();
        assert!(read(&b.0).is_err());
    }
    #[test]
    fn changed_report_or_missing_log_is_rejected() {
        let a = fixture(456, 1, 0);
        fs::write(a.0.join("informe.txt"), b"other").unwrap();
        assert!(read(&a.0).is_err());
        let b = fixture(789, 1, 0);
        fs::remove_file(b.0.join("gsplog.bin")).unwrap();
        assert!(read(&b.0).is_err());
    }
    #[test]
    fn comparison_refuses_dirty_builds_and_unobserved_phases() {
        let a = fixture(456, 1, 0);
        let b = fixture(789, 1, 0);
        let x = read(&a.0).unwrap();
        let mut y = read(&b.0).unwrap();
        y.meta[3] |= 1 << 62;
        assert!(compare(&x, &y).contains("sucios"));
        y.meta[3] = x.meta[3];
        y.records.remove(0);
        assert!(compare(&x, &y).contains("cobertura"));
    }
    #[test]
    fn validity_cannot_claim_a_pri_pattern_as_a_valid_zero_or_value() {
        let f = fixture(456, 1, 0);
        let mut raw = fs::read(f.0.join("captura.bin")).unwrap();
        raw[8 * 8..9 * 8].copy_from_slice(&(1u64 << 4 | 1).to_le_bytes());
        raw[c::HEADER * 8..(c::HEADER + 1) * 8].copy_from_slice(&0xBADF1002u64.to_le_bytes());
        let mut manifest = fs::read(f.0.join("listo.bin")).unwrap();
        manifest[160..192].copy_from_slice(&bmo_hash::hash(&raw));
        fs::write(f.0.join("captura.bin"), raw).unwrap();
        fs::write(f.0.join("listo.bin"), manifest).unwrap();
        assert!(read(&f.0).unwrap_err().to_string().contains("validez"));
    }
}
