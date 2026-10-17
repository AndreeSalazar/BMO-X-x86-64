//! [carril] VERDE
//! [consumo] NADA
use super::*;
fn id() -> Identity { Identity { device: Some(1), firmware: Some(2), build: Some(3), attempt: Some(4) } }
fn obs(sequence: u64, event: Event) -> Observation {
    Observation { identity: id(), sequence, time: sequence, event }
}
fn starting() -> Supervisor {
    let mut s = Supervisor::new(id()).unwrap();
    s.observe(obs(1, Event::Prepared)).unwrap();
    s.observe(obs(2, Event::Start { deadline: 10 })).unwrap(); s
}
#[test]
fn ready_requires_order_and_init_done() {
    let mut s = Supervisor::new(id()).unwrap();
    assert_eq!(s.observe(obs(1, Event::InitDone)), Err(Rejection::Order));
    s.observe(obs(1, Event::Prepared)).unwrap();
    assert_eq!(s.observe(obs(2, Event::InitDone)), Err(Rejection::Order));
    assert_eq!(s.observe(obs(2, Event::Start { deadline: 2 })), Err(Rejection::Deadline));
    s.observe(obs(2, Event::Start { deadline: 10 })).unwrap();
    assert_eq!(s.observe(obs(3, Event::Tick)), Ok(Decision::Wait));
    s.observe(obs(4, Event::InitDone)).unwrap();
    assert_eq!(s.state(), State::Ready);
}
#[test]
fn every_identity_component_is_required_and_bound() {
    for field in 0..4 {
        let mut missing = id();
        match field { 0 => missing.device=None, 1 => missing.firmware=None,
            2 => missing.build=None, _ => missing.attempt=None }
        assert!(matches!(Supervisor::new(missing), Err(Rejection::MissingIdentity)));
        let mut changed = id();
        match field { 0 => changed.device=Some(9), 1 => changed.firmware=Some(9),
            2 => changed.build=Some(9), _ => changed.attempt=Some(9) }
        let mut s = starting(); let mut o=obs(3, Event::InitDone); o.identity=changed;
        assert_eq!(s.observe(o), Err(Rejection::OtherSession));
        assert_eq!(s.state(), State::Starting);
    }
}
#[test]
fn duplicates_are_idempotent_but_reused_sequences_and_old_times_are_rejected() {
    let mut s=starting(); let o=obs(3, Event::Tick);
    s.observe(o).unwrap(); assert_eq!(s.observe(o), Ok(Decision::Duplicate));
    assert_eq!(s.observe(obs(3, Event::InitDone)), Err(Rejection::Stale));
    let mut old=obs(4, Event::InitDone); old.time=2;
    assert_eq!(s.observe(old), Err(Rejection::Stale));
    assert_eq!(s.observe(obs(1, Event::Prepared)), Err(Rejection::Stale));
    assert_eq!(s.state(), State::Starting);
}
#[test]
fn timeout_boundary_and_late_success_keep_first_failure() {
    for event in [Event::Tick, Event::InitDone, Event::Fault(0x15)] {
        let mut s=starting();
        assert_eq!(s.observe(obs(10,event)), Ok(Decision::PreserveFailure));
        assert_eq!(s.first_failure(), Some(Failure::Timeout));
        s.observe(obs(11,Event::InitDone)).unwrap();
        assert_eq!(s.state(),State::Failed);
    }
    let mut s=starting(); s.observe(obs(9,Event::InitDone)).unwrap();
    assert_eq!(s.state(),State::Ready);
}
#[test]
fn recovery_requires_all_three_known_true_and_never_clears_failure() {
    for a in [None,Some(false),Some(true)] {
        for b in [None,Some(false),Some(true)] {
            for c in [None,Some(false),Some(true)] {
                let mut s=starting(); s.observe(obs(3,Event::Fault(0x15))).unwrap();
                let good=a==Some(true)&&b==Some(true)&&c==Some(true);
                assert_eq!(s.observe(obs(4,Event::Recovery { engines_stopped:a,
                    dma_isolated:b,memory_released:c })),
                    Ok(if good {Decision::ReviewRecovery} else {Decision::BlockRecovery}));
                s.observe(obs(5,Event::Fault(99))).unwrap();
                s.observe(obs(6,Event::InitDone)).unwrap();
                assert_eq!(s.first_failure(),Some(Failure::Reported(0x15)));
                assert_eq!(s.state(),if good {State::Failed} else {State::RecoveryBlocked});
            }
        }
    }
}
#[test]
fn unknown_or_contradictory_snapshot_is_not_ready() {
    let mut s=Snapshot {prepared:Some(true),started:Some(true),init_done:Some(true),fault:None,stable:true};
    assert_eq!(diagnose(s),Some(State::Ready));
    s.stable=false; assert_eq!(diagnose(s),None); s.stable=true;
    s.prepared=None; assert_eq!(diagnose(s),None);
    s.prepared=Some(false); assert_eq!(diagnose(s),None);
    s.fault=Some(0x15); assert_eq!(diagnose(s),Some(State::Failed));
}

#[test]
fn ready_snapshot_needs_three_known_true_values() {
    for a in [None,Some(false),Some(true)] {
        for b in [None,Some(false),Some(true)] {
            for c in [None,Some(false),Some(true)] {
                for stable in [false,true] {
                    for fault in [None,Some(0x15)] {
                        let state=diagnose(Snapshot {prepared:a,started:b,init_done:c,stable,fault});
                        assert_eq!(state==Some(State::Ready), stable && fault.is_none()
                            && a==Some(true) && b==Some(true) && c==Some(true));
                    }
                }
            }
        }
    }
}
#[test]
fn favourable_recovery_does_not_unlock_a_blocked_session() {
    let mut s=starting(); s.observe(obs(3,Event::Fault(0x15))).unwrap();
    s.observe(obs(4,Event::Recovery {engines_stopped:None,dma_isolated:None,memory_released:None})).unwrap();
    assert_eq!(s.observe(obs(5,Event::Recovery {engines_stopped:Some(true),dma_isolated:Some(true),memory_released:Some(true)})),Ok(Decision::ReviewRecovery));
    assert_eq!(s.state(),State::RecoveryBlocked);
    assert_eq!(s.first_failure(),Some(Failure::Reported(0x15)));
}
