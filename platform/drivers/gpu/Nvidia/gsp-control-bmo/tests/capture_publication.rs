//! Publicacion real con escritores concurrentes; datos completamente sinteticos.
use bmo_nvidia_gsp_control::capture::{self as c, Journal, Record};
use std::sync::Arc;

#[test]
fn readers_never_observe_half_a_published_record() {
    let j = Arc::new(Journal::new());
    let mut writers = Vec::new();
    for worker in 0..3 {
        let bank = j.clone();
        writers.push(std::thread::spawn(move || {
            for n in 1..=8 {
                let id = worker * 8 + n;
                let mut r = Record::new([123, 456], 789, id, c::HALTED, 10);
                for i in 0..c::FIELDS {
                    r.set(i, id, false);
                }
                bank.publish(r);
            }
        }));
    }
    let check = || {
        for slot in 0..j.count() as usize {
            let attempt = j.word(slot, 4).unwrap();
            assert_eq!(j.word(slot, 10), Some(slot as u64 + 1));
            for field in 0..c::FIELDS {
                assert_eq!(j.word(slot, c::HEADER + field), Some(attempt));
            }
        }
    };
    for _ in 0..100 {
        check();
        std::thread::yield_now();
    }
    for thread in writers {
        thread.join().unwrap();
    }
    check();
    assert_eq!(j.count() + j.lost(), 24);
}
