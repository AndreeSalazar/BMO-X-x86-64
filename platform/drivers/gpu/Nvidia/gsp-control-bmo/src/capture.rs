//! Capturas inmutables y acotadas: datos, no protocolo de aparato.
//! [carril] VERDE
//! [consumo] NADA -- publicacion explicita; nunca gira esperando un cerrojo
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const MAGIC: u64 = u64::from_le_bytes(*b"BMOGCAP1");
pub const READY: u64 = u64::from_le_bytes(*b"BMOGSET1");
pub const SELECTOR: u64 = 64;
/// Marca de transporte BMO, no un codigo NVIDIA. Cero de INFO nunca es dato.
pub const TAG: u64 = 0xC437_CA01_0000_0000;
pub fn decode(low: u64, high: u64) -> Option<u64> {
    (low & 0xFFFF_FFFF_0000_0000 == TAG && high & 0xFFFF_FFFF_0000_0000 == TAG)
        .then_some((low & 0xFFFF_FFFF) | (high & 0xFFFF_FFFF) << 32)
}
pub const CAPACITY: usize = 32;
pub const FIELDS: usize = 26;
pub const HEADER: usize = 11;
pub const WORDS: usize = HEADER + FIELDS;
pub const BEFORE_FRTS: u64 = 1;
pub const AFTER_FRTS: u64 = 2;
pub const BEFORE_BOOTER: u64 = 3;
pub const HALTED: u64 = 4;
pub const WAIT_EXPIRED: u64 = 5;
pub const NAMES: [&str; FIELDS] = [
    "gsp.mailbox0",
    "gsp.mailbox1",
    "gsp.cpuctl",
    "gsp.bcr",
    "sec2.mailbox0",
    "sec2.mailbox1",
    "sec2.cpuctl",
    "sec2.bcr",
    "wpr2.lo",
    "wpr2.hi",
    "bsi14",
    "gfw",
    "sec2.hwcfg2",
    "gsp.hwcfg2",
    "sec2.dma",
    "sec2.engine",
    "gsp.engine",
    "ecc.l2",
    "ecc.hub",
    "ecc.fill",
    "put.init",
    "put.intr",
    "put.rm",
    "iommu.events",
    "g0",
    "boot0",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    Absent,
    Value(u64),
    Pri(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record(pub [u64; WORDS]);
impl Record {
    pub const EMPTY: Self = Self([0; WORDS]);
    pub fn new(session: [u64; 2], build: u64, attempt: u64, phase: u64, start: u64) -> Self {
        let mut r = Self::EMPTY;
        r.0[..8].copy_from_slice(&[
            MAGIC, session[0], session[1], build, attempt, phase, start, start,
        ]);
        r
    }
    pub fn set(&mut self, field: usize, value: u64, pri: bool) {
        assert!(field < FIELDS);
        self.0[8] |= 1 << field;
        if pri {
            self.0[9] |= 1 << field;
        } else {
            self.0[9] &= !(1 << field);
        }
        self.0[HEADER + field] = value;
    }
    pub fn reading(&self, field: usize) -> Reading {
        if self.0[8] & (1 << field) == 0 {
            Reading::Absent
        } else if self.0[9] & (1 << field) != 0 {
            Reading::Pri(self.0[HEADER + field])
        } else {
            Reading::Value(self.0[HEADER + field])
        }
    }
    pub fn valid(&self) -> bool {
        self.0[0] == MAGIC
            && self.0[2] != 0
            && self.0[4] != 0
            && (BEFORE_FRTS..=WAIT_EXPIRED).contains(&self.0[5])
            && self.0[7] >= self.0[6]
            && self.0[8] >> FIELDS == 0
            && self.0[9] & !self.0[8] == 0
    }
}

/// Append-only. Un escritor publica con Release al acabar; los slots anteriores
/// no cambian nunca. Lleno/ocupado se declara con lost; no pisa otra captura.
pub struct Journal {
    busy: AtomicBool,
    count: AtomicU64,
    lost: AtomicU64,
    data: [[AtomicU64; WORDS]; CAPACITY],
}
impl Journal {
    pub const fn new() -> Self {
        Self {
            busy: AtomicBool::new(false),
            count: AtomicU64::new(0),
            lost: AtomicU64::new(0),
            data: [const { [const { AtomicU64::new(0) }; WORDS] }; CAPACITY],
        }
    }
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Acquire)
    }
    pub fn lost(&self) -> u64 {
        self.lost.load(Ordering::Acquire)
    }
    pub fn word(&self, slot: usize, word: usize) -> Option<u64> {
        if slot >= self.count() as usize || word >= WORDS {
            return None;
        }
        Some(self.data[slot][word].load(Ordering::Relaxed))
    }
    pub fn publish(&self, mut r: Record) -> bool {
        if !r.valid() {
            self.lost.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            self.lost.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let count = self.count() as usize;
        let terminal = |p| p == HALTED || p == WAIT_EXPIRED;
        let duplicate = (0..count).any(|i| {
            self.data[i][4].load(Ordering::Relaxed) == r.0[4] && {
                let p = self.data[i][5].load(Ordering::Relaxed);
                p == r.0[5] || terminal(p) && terminal(r.0[5])
            }
        });
        let published = if duplicate {
            false
        } else if count == CAPACITY {
            self.lost.fetch_add(1, Ordering::Relaxed);
            false
        } else {
            r.0[10] = count as u64 + 1;
            for (dst, value) in self.data[count].iter().zip(r.0) {
                dst.store(value, Ordering::Relaxed);
            }
            self.count.store(count as u64 + 1, Ordering::Release);
            true
        };
        self.busy.store(false, Ordering::Release);
        published
    }
    pub fn has(&self, attempt: u64, phase: u64) -> bool {
        (0..self.count() as usize)
            .any(|i| self.word(i, 4) == Some(attempt) && self.word(i, 5) == Some(phase))
    }
}

/// Primera diferencia por fase y campo observado, no primera instruccion ni causa.
pub fn first_difference(a: &Record, b: &Record) -> Option<usize> {
    (0..FIELDS).find(|&i| a.reading(i) != b.reading(i))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(attempt: u64, phase: u64) -> Record {
        Record::new([123, 456], 789, attempt, phase, 10)
    }
    #[test]
    fn absence_zero_and_pri_are_distinct_and_raw() {
        assert_eq!(decode(TAG, TAG), Some(0));
        assert_eq!(decode(0, TAG), None);
        assert_eq!(decode(TAG, 0), None);
        assert_eq!(
            decode(TAG | 0xBADF1002, TAG | 0xFFFF_FFFF),
            Some(0xFFFF_FFFF_BADF1002)
        );
        let mut r = record(1, HALTED);
        assert_eq!(r.reading(0), Reading::Absent);
        r.set(0, 0, false);
        assert_eq!(r.reading(0), Reading::Value(0));
        r.set(0, 0xBADF1002, true);
        assert_eq!(r.reading(0), Reading::Pri(0xBADF1002));
        let mut zero = r;
        zero.set(0, 0, false);
        assert_eq!(first_difference(&r, &zero), Some(0));
    }
    #[test]
    fn retries_never_modify_published_fields_or_pair_attempts() {
        let j = Journal::new();
        assert!(j.publish(record(1, BEFORE_BOOTER)));
        assert!(j.publish(record(1, HALTED)));
        let mut next = record(2, BEFORE_BOOTER);
        next.set(0, 33, false);
        assert!(j.publish(next));
        assert_eq!(j.word(0, HEADER), Some(0));
        assert_eq!(j.word(2, HEADER), Some(33));
        assert_eq!(j.word(3, HEADER), None);
        assert!(!j.has(2, AFTER_FRTS));
    }
    #[test]
    fn timeout_is_not_overwritten_by_a_later_halt() {
        let j = Journal::new();
        assert!(j.publish(record(1, WAIT_EXPIRED)));
        assert!(!j.publish(record(1, HALTED)));
        assert_eq!(j.count(), 1);
        assert_eq!(j.word(0, 5), Some(WAIT_EXPIRED));
    }
    #[test]
    fn bounded_journal_declares_loss_and_does_not_wrap() {
        let j = Journal::new();
        for i in 1..=CAPACITY {
            assert!(j.publish(record(i as u64, HALTED)));
        }
        assert!(!j.publish(record(99, HALTED)));
        assert_eq!(j.lost(), 1);
        assert_eq!(j.word(0, 4), Some(1));
        assert_eq!(j.word(0, WORDS), None);
    }
    #[test]
    fn malformed_masks_and_clock_ranges_are_rejected() {
        for field in [0, 2, 4, 5] {
            let mut r = record(1, HALTED);
            r.0[field] = 0;
            assert!(!r.valid());
        }
        let mut r = record(1, HALTED);
        r.0[9] = 1;
        assert!(!r.valid());
        r.0[9] = 0;
        r.0[7] = 9;
        assert!(!r.valid());
    }
}
