//! Ejecuta el exportador REAL de Director con memoria/archivos sinteticos.
//! No ejecuta syscalls ni MMIO. Ningun dato privado entra en este banco.
extern crate self as bmo_userland;
use bmo_nvidia_gsp_control::capture as c;
use std::{
    cell::{RefCell, UnsafeCell},
    collections::{BTreeMap, BTreeSet},
};
pub const INFO_GPU_DESPIERTO_BUZON: u64 = 0xC0;
pub const INFO_GPU_INTENTO: u64 = 0xDC;
pub const INFO_GPU_GSP_MEM: u64 = 0xC1;
pub const INFO_GPU_LIBOS: u64 = 0xBE;
pub const LIBOS_PREPARADO: u64 = 1 << 56;
#[derive(Default)]
struct Door {
    records: Vec<c::Record>,
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    dirs: BTreeSet<Vec<u8>>,
    reads: u64,
    reserve: bool,
    change: bool,
    fail: Vec<u8>,
    dir_error: Option<u32>,
    transport_fail: u8,
}
thread_local! { static DOOR: RefCell<Door> = RefCell::new(Door::default()); }
// El exportador real pertenece a UN Director. No simular varios procesos
// compartiendo su cache EXPORTED: cada caso usa la misma instancia serializada.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn info(sel: u64) -> u64 {
    DOOR.with(|d| {
        let mut d = d.borrow_mut();
        match sel & 255 {
            INFO_GPU_INTENTO => {
                if d.change && d.reads > 100 {
                    2
                } else {
                    d.records.last().map_or(0, |r| r.0[4])
                }
            }
            INFO_GPU_LIBOS => {
                if d.reserve {
                    LIBOS_PREPARADO
                } else {
                    0
                }
            }
            INFO_GPU_DESPIERTO_BUZON => {
                let (value, half) = match (sel >> 8) & 255 {
                    c::SELECTOR => {
                        if d.transport_fail == 1 && (sel >> 16) & 255 == 1 {
                            return 0;
                        }
                        (
                            match (sel >> 16) & 255 {
                                0 => c::MAGIC,
                                1 => d.records.len() as u64,
                                2 => 0,
                                3 => 123,
                                4 => 456,
                                _ => panic!(),
                            },
                            (sel >> 32) & 1,
                        )
                    }
                    x if x == c::SELECTOR + 1 => {
                        if d.transport_fail == 2 && sel >> 24 == c::HEADER as u64 * 2 {
                            return 0;
                        }
                        (
                            d.records[((sel >> 16) & 255) as usize].0[(sel >> 24) as usize / 2],
                            (sel >> 24) & 1,
                        )
                    }
                    x if x == c::SELECTOR + 3 => {
                        if d.transport_fail == 3 {
                            return 0;
                        }
                        d.reads += 1;
                        let part = sel >> 16;
                        (((part / 2) * 8) ^ 0x1234, part & 1)
                    }
                    _ => panic!(),
                };
                c::TAG | (value >> (half * 32)) & 0xFFFF_FFFF
            }
            _ => panic!("consulta inventada"),
        }
    })
}
pub fn ciclos() -> u64 {
    10
}
pub struct Memoria {
    bytes: Box<[UnsafeCell<u8>]>,
}
impl Memoria {
    pub fn request(n: u64) -> Option<Self> {
        Some(Self {
            bytes: (0..n).map(|_| UnsafeCell::new(0)).collect(),
        })
    }
    pub fn base(&self) -> *mut u8 {
        self.bytes.as_ptr() as *mut u8
    }
}
pub struct Directorio;
impl Directorio {
    pub fn open(path: &[u8]) -> Result<Self, u32> {
        DOOR.with(|d| {
            let d = d.borrow();
            if let Some(e) = d.dir_error {
                return Err(e);
            }
            if d.dirs.contains(path) {
                Ok(Self)
            } else {
                Err(26)
            }
        })
    }
    pub fn crear(path: &[u8]) -> Result<Self, u32> {
        DOOR.with(|d| {
            d.borrow_mut().dirs.insert(path.to_vec());
        });
        Ok(Self)
    }
}
pub struct Archivo {
    path: Vec<u8>,
}
impl Archivo {
    pub fn create(path: &[u8]) -> Result<Self, u32> {
        DOOR.with(|d| {
            d.borrow_mut().files.insert(path.to_vec(), Vec::new());
        });
        Ok(Self {
            path: path.to_vec(),
        })
    }
    pub fn write(&self, b: &[u8]) -> usize {
        DOOR.with(|d| {
            let mut d = d.borrow_mut();
            if !d.fail.is_empty() && self.path.ends_with(&d.fail) {
                return 0;
            }
            d.files.get_mut(&self.path).unwrap().extend_from_slice(b);
            b.len()
        })
    }
    pub fn escribir_de(&self, b: &Memoria, from: u64, n: u64) -> u64 {
        let data: Vec<_> = b.bytes[from as usize..(from + n) as usize]
            .iter()
            .map(|v| {
                // SAFETY: banco de un hilo, rango acotado del bloque propio; la
                // escritura del exportador termino antes de esta copia simulada.
                unsafe { *v.get() }
            })
            .collect();
        self.write(&data) as u64
    }
    pub fn close(self) -> bool {
        true
    }
}
mod scene {
    pub mod output {
        #[derive(Default)]
        pub struct Output(pub Vec<u8>);
        impl Output {
            pub fn text(&mut self, s: &[u8]) {
                self.0.extend_from_slice(s)
            }
            pub fn dec(&mut self, v: u64) {
                self.text(v.to_string().as_bytes())
            }
            pub fn byte(&mut self, b: u8) {
                self.0.push(b)
            }
        }
    }
}
mod commands {
    pub mod obs_build {
        pub const DIRECTOR: u64 = 1 << 63 | 789;
    }
}
#[path = "../../../../../Ultra_userspace/services/director/src/commands/gsp_capture.rs"]
mod actual;
fn reset(reserve: bool, change: bool, fail: &[u8]) {
    let mut records = Vec::new();
    for (i, p) in [c::BEFORE_FRTS, c::AFTER_FRTS, c::BEFORE_BOOTER, c::HALTED]
        .into_iter()
        .enumerate()
    {
        let mut r = c::Record::new([123, 456], commands::obs_build::DIRECTOR, 1, p, 10);
        r.0[10] = i as u64 + 1;
        records.push(r);
    }
    DOOR.with(|d| {
        *d.borrow_mut() = Door {
            records,
            reserve,
            change,
            fail: fail.to_vec(),
            ..Door::default()
        }
    });
}
fn file(name: &[u8]) -> Vec<u8> {
    DOOR.with(|d| {
        d.borrow()
            .files
            .iter()
            .find(|(p, _)| p.ends_with(name))
            .unwrap()
            .1
            .clone()
    })
}
fn ready() -> bool {
    DOOR.with(|d| d.borrow().files.keys().any(|p| p.ends_with(b"listo.bin")))
}
#[test]
fn real_export_binds_all_three_files_and_refuses_overwrite_or_changed_attempt() {
    let _serial = SERIAL.lock().unwrap();
    reset(true, false, b"");
    assert!(actual::save());
    let m = file(b"listo.bin");
    assert_eq!(m.len(), 256);
    for (i, name) in [b"captura.bin" as &[u8], b"gsplog.bin", b"informe.txt"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(bmo_hash::hash(&file(name)), m[160 + i * 32..192 + i * 32]);
    }
    assert_eq!(file(b"gsplog.bin").len(), 196608);
    assert!(file(b"informe.txt").starts_with(b"BMO-GSP captura v1"));
    let mut out = scene::output::Output::default();
    actual::row(&mut out);
    assert!(String::from_utf8(out.0)
        .unwrap()
        .contains("conjunto cerrado del intento 1"));
    let original = file(b"listo.bin");
    assert!(!actual::save());
    assert_eq!(file(b"listo.bin"), original);
    reset(true, true, b"");
    assert!(!actual::save());
    assert!(!ready());
}
#[test]
fn absence_does_not_manufacture_logs_and_failed_write_never_publishes_ready() {
    let _serial = SERIAL.lock().unwrap();
    reset(false, false, b"");
    assert!(actual::save());
    DOOR.with(|d| assert!(!d.borrow().files.keys().any(|p| p.ends_with(b"gsplog.bin"))));
    let m = file(b"listo.bin");
    assert_eq!(&m[56..64], &0u64.to_le_bytes());
    assert_eq!(u64::from_le_bytes(m[80..88].try_into().unwrap()) & 7, 0);
    reset(true, false, b"captura.bin");
    assert!(!actual::save());
    assert!(!ready());
    reset(true, false, b"informe.txt");
    assert!(!actual::save());
    assert!(!ready());
}
#[test]
fn directory_errors_do_not_mean_absence() {
    let _serial = SERIAL.lock().unwrap();
    reset(true, false, b"");
    DOOR.with(|d| d.borrow_mut().dir_error = Some(25));
    assert!(!actual::save());
    DOOR.with(|d| assert!(d.borrow().files.is_empty()));
    let kernel =
        include_str!("../../../../../Ultra_kernel_x86-64/kernel/src/ring0/obj/directory.rs");
    assert!(kernel.contains("pub const ERROR_DIR_NO_ESTA: u32 = 26;"));
}
#[test]
fn transport_failures_never_become_valid_zero_words_or_zero_logs() {
    let _serial = SERIAL.lock().unwrap();
    for failure in 1..=3 {
        reset(true, false, b"");
        DOOR.with(|d| d.borrow_mut().transport_fail = failure);
        assert!(!actual::save());
        assert!(!ready());
    }
}
#[test]
fn full_journal_report_fits_without_truncation() {
    let _serial = SERIAL.lock().unwrap();
    reset(true, false, b"");
    DOOR.with(|d| {
        let mut d = d.borrow_mut();
        let first = d.records.clone();
        d.records.clear();
        for attempt in 1..=8 {
            for old in &first {
                let mut r = *old;
                r.0[4] = attempt;
                r.0[10] = d.records.len() as u64 + 1;
                d.records.push(r);
            }
        }
    });
    assert!(actual::save());
    assert_eq!(file(b"captura.bin").len(), c::CAPACITY * c::WORDS * 8);
    let report = file(b"informe.txt");
    assert!(report.len() < 4096);
    assert!(String::from_utf8(report)
        .unwrap()
        .contains("0000000000000020"));
}
