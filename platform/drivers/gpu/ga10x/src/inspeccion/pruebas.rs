//! Vectores FIFO literales, despues integracion con el constructor real.
//! [carril] VERDE -- banco sobre copias, sin aparato
//! [consumo] NADA -- solo pruebas de anfitrion
extern crate std;
use super::{empuje::*, metodos::*};

#[test]
fn literal_modes_keep_packet_data_separate_from_headers() {
    for (header, expected) in [(0x20030040, [0x100,0x104,0x108]), (0x60030040, [0x100,0x100,0x100]), (0xa0030040, [0x100,0x104,0x104])] {
        let words = [header, 0x80000000, 0xe0000000, 0];
        let mut p = Push::from_words(&words);
        let packet = p.next().unwrap().unwrap();
        assert_eq!(packet.count, 3);
        for k in 0..3 {
            let c = packet.command(k).unwrap();
            assert_eq!(c.method, expected[k]); assert_eq!(c.value, words[k+1]); assert_eq!(c.word, k+1);
        }
        assert!(packet.command(3).is_none()); assert!(p.next().is_none());
    }
    let words = [0x80004040];
    let packet = Push::from_words(&words).next().unwrap().unwrap();
    assert_eq!(packet.mode, Mode::Immediate);
    assert_eq!(packet.command(0), Some(Command { word: 0, subchannel: 2, method: 0x100, value: 0 }));
}

#[test]
fn noops_do_not_consume_next_header_and_reserved_fields_are_not_guessed() {
    let words = [0, 0x20001fff, 0x80010040];
    let packets: std::vec::Vec<_> = Push::from_words(&words).map(Result::unwrap).collect();
    assert_eq!(packets.len(), 3); assert_eq!(packets[1].count, 0);
    assert_eq!(packets[2].command(0).unwrap().value, 1);
    let mut bad = Push::from_words(&[0x80011040]);
    assert_eq!(bad.next().unwrap().err().unwrap().reason, Reason::ReservedBit);
    assert!(bad.next().is_none());
}

#[test]
fn invalid_sequences_stop_without_resynchronization_or_silent_wrap() {
    for (words, reason) in [(&[0x20030040, 1][..], Reason::Truncated), (&[0xe0000000, 0][..], Reason::UnsupportedControl), (&[0x20020fff, 0, 0][..], Reason::MethodRange), (&[0xa0020fff, 0, 0][..], Reason::MethodRange)] {
        let mut p = Push::from_words(words);
        let e = p.next().unwrap().err().unwrap();
        assert_eq!(e.word, 0); assert_eq!(e.raw, words[0]); assert_eq!(e.reason, reason);
        assert!(p.next().is_none()); assert!(p.next().is_none());
    }
    for n in [1, 2, 3, 5, 6, 7] {
        assert_eq!(Push::from_bytes(&[0; 8][..n]).err().unwrap().reason, Reason::PartialWord);
    }
}

#[test]
fn byte_and_word_inputs_agree_even_for_maximum_immediate_and_subchannel() {
    let words: [u32; 2] = [0x9fffefff, 0];
    let bytes: std::vec::Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    for (a, b) in Push::from_words(&words).zip(Push::from_bytes(&bytes).unwrap()) {
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_eq!((a.raw, a.words, a.command(0)), (b.raw, b.words, b.command(0)));
        if a.mode == Mode::Immediate { assert_eq!(a.command(0).unwrap().value, 8191); assert_eq!(a.subchannel, 7); }
    }
}

#[test]
fn method_names_need_context_and_never_name_reserved_holes() {
    assert_eq!(ampere_3d(0, crate::tresde::WAIT_FOR_IDLE as u16).track, Track::Synchronization);
    assert_eq!(ampere_3d(4, crate::tresde::WAIT_FOR_IDLE as u16).track, Track::Unknown);
    for m in [0x0824, 0x2004, 0x2001] { assert_eq!(ampere_3d(0, m).track, Track::Unknown); }
}

#[test]
fn declared_context_is_required_and_rebinding_invalidates_it() {
    let mut ctx = Context::new();
    let wait = Command { word: 1, subchannel: 0, method: crate::tresde::WAIT_FOR_IDLE as u16, value: 0 };
    assert_eq!(ctx.describe(wait).track, Track::Unknown);
    let mut bind = Command { word: 0, subchannel: 0, method: 0, value: crate::gr::AMPERE_B };
    ctx.describe(bind); assert_eq!(ctx.describe(wait).track, Track::Synchronization);
    bind.value = 0xdead; ctx.describe(bind); assert_eq!(ctx.describe(wait).track, Track::Unknown);
    bind.subchannel = 4; bind.value = crate::gr::AMPERE_B; ctx.describe(bind);
    assert_eq!(ctx.describe(Command { subchannel: 4, ..wait }).track, Track::Unknown);
}

#[test]
fn actual_template_has_draw_and_final_sync_and_covers_all_words_once() {
    let o = super::verrano_template();
    let mut end = 0;
    let (mut draw, mut sync, mut commands) = (0, 0, 0);
    for packet in Push::from_words(&o.o[..o.n]) {
        let packet = packet.unwrap(); assert_eq!(packet.word, end); end += packet.words;
        for k in 0..packet.count {
            let c = packet.command(k).unwrap(); commands += 1;
            match ampere_3d(c.subchannel, c.method).track { Track::Draw => draw += 1, Track::Synchronization => sync += 1, _ => {} }
        }
    }
    assert_eq!(end, o.n); assert!(draw >= 3 && sync >= 1 && commands > 20);
}

#[test]
fn edited_public_metadata_never_indexes_outside_the_copy() {
    let words = [0x20010040, 0];
    let mut p = Push::from_words(&words).next().unwrap().unwrap();
    p.word = usize::MAX; assert!(p.command(0).is_none());
    p.word = 0; p.count = usize::MAX; assert!(p.command(usize::MAX-1).is_none());
    p.method = u16::MAX; assert!(p.command(0).is_none());
}

#[test]
fn every_prefix_of_real_orders_is_complete_or_reports_the_cut_packet() {
    let o = super::verrano_template();
    for cut in 0..=o.n {
        let words = &o.o[..cut]; let mut end = 0; let mut failed = false;
        for result in Push::from_words(words) {
            match result {
                Ok(p) => {
                    assert_eq!(p.word, end); end += p.words; assert!(end <= cut);
                    for k in 0..p.count { assert!(p.command(k).unwrap().word < cut); }
                }
                Err(e) => {
                    assert_eq!(e.reason, Reason::Truncated); assert_eq!(e.word, end);
                    assert!(e.declared > e.available); failed = true;
                }
            }
        }
        assert_eq!(end == cut, !failed);
    }
}
