//! Banco de diagnostico: hechos sinteticos, sin nueva medicion GPU.
//! [carril] VERDE
//! [consumo] NADA
use super::*;

fn stopped(mailbox0: Option<Mailbox>) -> Evidence {
    Evidence { stable: true, started: Some(true), halted: Some(true),
               mailbox0, host_rejected: false }
}

#[test]
fn nonzero_code_is_not_the_host_reason_or_a_translation() {
    for code in [0x15, 44, 1, 0x1234] {
        let e = stopped(Some(Mailbox::Readable(code)));
        assert_eq!(diagnose(e), Completion::MailboxNonZero(code));
        assert_eq!(diagnose(Evidence { host_rejected: true, ..e }), Completion::MailboxNonZero(code));
    }
    assert!(Completion::MailboxNonZero(0x15).explanation().windows(4).any(|w| w == b"0x15"));
}

#[test]
fn no_mailbox_and_failed_read_cannot_become_zero() {
    assert_eq!(diagnose(stopped(None)), Completion::ReadUnavailable(None));
    for raw in [0xBADF_1002, 0xBADF_5620, u32::MAX, 0] {
        assert_eq!(diagnose(stopped(Some(Mailbox::Unreadable(raw)))),
                   Completion::ReadUnavailable(Some(raw)));
    }
}

#[test]
fn zero_is_only_a_mailbox_observation_and_cannot_erase_a_recorded_failure() {
    let e = stopped(Some(Mailbox::Readable(0)));
    assert_eq!(diagnose(e), Completion::MailboxZero);
    assert_eq!(diagnose(Evidence { host_rejected: true, ..e }), Completion::Contradictory);
}

#[test]
fn a_running_falcon_does_not_publish_a_final_return() {
    let e = Evidence { halted: Some(false), ..stopped(Some(Mailbox::Readable(0x15))) };
    assert_eq!(diagnose(e), Completion::InFlight);
    assert_eq!(diagnose(Evidence { host_rejected: true, ..e }), Completion::Contradictory);
}

#[test]
fn unknown_absent_and_contradictory_stages_remain_distinct() {
    let e = stopped(Some(Mailbox::Readable(0)));
    assert_eq!(diagnose(Evidence { started: None, ..e }), Completion::Unknown);
    assert_eq!(diagnose(Evidence { halted: None, ..e }), Completion::Unknown);
    assert_eq!(diagnose(Evidence { started: Some(false), halted: Some(false), ..e }), Completion::NotStarted);
    assert_eq!(diagnose(Evidence { started: Some(false), ..e }), Completion::Contradictory);
}

#[test]
fn changing_query_overrides_every_stage_and_return() {
    for started in [None, Some(false), Some(true)] {
        for halted in [None, Some(false), Some(true)] {
            let e = Evidence { stable: false, started, halted,
                               ..stopped(Some(Mailbox::Readable(0x15))) };
            assert_eq!(diagnose(e), Completion::Changing);
        }
    }
}

#[test]
fn cached_stop_without_attempt_stamp_is_not_attributed_to_retries() {
    assert_eq!(link_capture(true, Some(1), true), CaptureLink::ThisAttempt);
    for attempt in [None, Some(0), Some(2), Some(u64::MAX)] {
        assert_eq!(link_capture(true, attempt, true), CaptureLink::Unattributed);
    }
    for attempt in [None, Some(1), Some(2)] {
        assert_eq!(link_capture(true, attempt, false), CaptureLink::Absent);
        assert_eq!(link_capture(false, attempt, true), CaptureLink::Changing);
    }
}
