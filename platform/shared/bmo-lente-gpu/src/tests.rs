extern crate std;
use super::*;
use std::string::String;

fn snap(b: &[u8]) -> Snapshot<'_> {
    Snapshot::new(Origin::Prepared, Space::Resource, 0, b.len() as u64, 0, Capture::Bytes(b)).unwrap()
}

#[test]
fn all_bytes_roundtrip_in_memory_order_with_leading_zeroes() {
    let bytes: std::vec::Vec<u8> = (0..=255).collect();
    let mut text = String::new();
    snap(&bytes).write(&mut text).unwrap();
    let mut recovered = std::vec::Vec::new();
    for (k, line) in text.lines().filter(|s| s.starts_with('+')).enumerate() {
        let (hex, bits) = line.split_once("| ").unwrap();
        assert!(hex.starts_with(&std::format!("+{:016x}", k * ROW_BYTES)));
        let hex: std::vec::Vec<u8> = hex.split_whitespace().skip(1).map(|b| u8::from_str_radix(b, 16).unwrap()).collect();
        let binary: std::vec::Vec<u8> = bits.split_whitespace().map(|b| { assert_eq!(b.len(), 8); u8::from_str_radix(b, 2).unwrap() }).collect();
        assert_eq!(hex, binary);
        recovered.extend(hex);
    }
    assert_eq!(recovered, bytes);
}

#[test]
fn pages_cover_every_byte_once_including_tail_and_eof() {
    for total in 0..=257 {
        let mut offset = 0;
        loop {
            let n = page(total, offset).unwrap();
            assert!(n <= PAGE_BYTES);
            let buffer = [0; PAGE_BYTES];
            let s = Snapshot::new(Origin::Prepared, Space::Resource, 0, total, offset, Capture::Bytes(&buffer[..n])).unwrap();
            if let Some(next) = s.next() { assert!(next > offset); offset = next; }
            else { assert_eq!(offset + n as u64, total); break; }
        }
        assert_eq!(page(total, total), Ok(0));
        assert_eq!(page(total, total + 1), Err(Error::Bounds));
    }
    assert_eq!(page(u64::MAX, u64::MAX - 3), Ok(3));
}

#[test]
fn incomplete_and_unavailable_are_never_complete() {
    for (capture, state) in [(Capture::Bytes(&[]), State::Window), (Capture::Unavailable, State::Unavailable), (Capture::Bytes(&[1]), State::Window)] {
        let s = Snapshot::new(Origin::Readback, Space::GpuVirtual, 0x1000, 10, 0, capture).unwrap();
        assert_eq!(s.state(), state);
        let mut out = String::new(); s.write(&mut out).unwrap();
        assert!(!out.contains("COMPLETO"));
    }
    assert_eq!(snap(&[]).state(), State::Complete);
    let s = Snapshot::new(Origin::Readback, Space::GpuVirtual, 0, 4, 2, Capture::Bytes(&[0, 255])).unwrap();
    assert_eq!(s.state(), State::Window);
    assert_eq!(s.next(), None);
}

#[test]
fn bounds_and_address_domains_are_explicit() {
    let new = |base, total, offset, b| Snapshot::new(Origin::Prepared, Space::Resource, base, total, offset, Capture::Bytes(b));
    assert_eq!(new(0, 1, 2, &[]).unwrap_err(), Error::Bounds);
    assert_eq!(new(0, 1, 0, &[1, 2]).unwrap_err(), Error::Bounds);
    assert_eq!(new(u64::MAX, 2, 0, &[1]).unwrap_err(), Error::AddressOverflow);
    assert!(new(u64::MAX, 1, 0, &[255]).is_ok());
    let mut out = String::new();
    Snapshot::new(Origin::Prepared, Space::CpuVirtual, 0x1234, 1, 0, Capture::Bytes(&[1])).unwrap().write(&mut out).unwrap();
    assert!(out.contains("VA CPU base=0000000000001234"));
    assert!(out.contains("+0000000000000000"));
}

#[test]
fn comparisons_report_exact_first_offset_and_refuse_missing_ranges() {
    let a = Snapshot::new(Origin::Prepared, Space::Resource, 0, 20, 8, Capture::Bytes(&[0, 1, 2])).unwrap();
    let b = Snapshot::new(Origin::Readback, Space::GpuVirtual, 0x1000, 20, 8, Capture::Bytes(&[0, 9, 8])).unwrap();
    assert_eq!(compare(a, b), Ok(Difference { count: 2, first: Some(9) }));
    assert_eq!(compare(a, a), Ok(Difference { count: 0, first: None }));
    assert_eq!(compare(a, snap(&[0, 1, 2])), Err(Error::DifferentWindow));
    let missing = Snapshot::new(Origin::Readback, Space::Resource, 0, 20, 8, Capture::Unavailable).unwrap();
    assert_eq!(compare(a, missing), Err(Error::MissingBytes));
}

#[test]
fn output_failure_is_propagated_instead_of_reporting_success() {
    struct Full;
    impl Write for Full { fn write_str(&mut self, _: &str) -> fmt::Result { Err(fmt::Error) } }
    assert!(snap(&[1]).write(&mut Full).is_err());
}
