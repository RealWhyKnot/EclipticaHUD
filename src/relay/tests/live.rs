use super::*;

#[test]
#[ignore = "needs the live relay"]
fn live_relay_round_trip() {
    let url = std::env::var("EHUD_E2E_URL").expect("EHUD_E2E_URL");
    let hold: u64 = std::env::var("EHUD_E2E_HOLD_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let room_b = "wrld_00000000-0000-0000-0000-000000000000:54321~region(us)";
    let usr2 = "usr_11111111-1111-1111-1111-111111111111";
    let usr3 = "usr_22222222-2222-2222-2222-222222222222";
    let a1: &'static Hub = Box::leak(Box::new(Hub::new()));
    let a2: &'static Hub = Box::leak(Box::new(Hub::new()));
    let b1: &'static Hub = Box::leak(Box::new(Hub::new()));
    let (w1, w2, w3) = (
        live_want(&url, LOC, USR),
        live_want(&url, LOC, usr2),
        live_want(&url, room_b, usr3),
    );
    for (hub, w) in [(a1, &w1), (a2, &w2), (b1, &w3)] {
        hub.sync(Some(w.clone()), None);
    }
    for hub in [a1, a2, b1] {
        wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    }
    a1.sync(Some(w1.clone()), Some(dealt(1111)));
    a2.sync(Some(w2.clone()), Some(dealt(2222)));
    b1.sync(Some(w3.clone()), Some(dealt(3333)));
    wait_for(|| live_seen(a1, w2.member(), 2222).then_some(()));
    wait_for(|| live_seen(a2, w1.member(), 1111).then_some(()));
    std::thread::sleep(Duration::from_millis(1500));
    for s in a1.poll().snaps.iter().chain(a2.poll().snaps.iter()) {
        assert!(!s.entries.iter().any(|(m, _)| *m == w3.member()));
    }
    assert!(b1.poll().snaps.is_empty());
    let usr4 = "usr_44444444-4444-4444-4444-444444444444";
    let b2: &'static Hub = Box::leak(Box::new(Hub::new()));
    let w4 = live_want(&url, room_b, usr4);
    b2.sync(Some(w4), None);
    wait_for(|| live_seen(b2, w3.member(), 3333).then_some(()));
    let end = Instant::now() + Duration::from_secs(hold);
    let mut tick = 0u64;
    while Instant::now() < end {
        tick += 1;
        std::thread::sleep(Duration::from_millis(1000));
        for hub in [a1, a2, b1, b2] {
            assert_eq!(hub.poll().link, Link::Connected, "dropped after {tick} s");
        }
    }
    a1.sync(Some(w1.clone()), Some(dealt(5555)));
    wait_for(|| live_seen(a2, w1.member(), 5555).then_some(()));
    for hub in [a1, a2, b1, b2] {
        assert!(hub.shutdown(Duration::from_secs(5)));
        assert_eq!(hub.poll().link, Link::Off);
    }
}

#[test]
#[ignore = "needs the live relay"]
fn live_relay_rejoins_after_restart() {
    let url = std::env::var("EHUD_E2E_URL").expect("EHUD_E2E_URL");
    let usr2 = "usr_33333333-3333-3333-3333-333333333333";
    let a1: &'static Hub = Box::leak(Box::new(Hub::new()));
    let a2: &'static Hub = Box::leak(Box::new(Hub::new()));
    let (w1, w2) = (live_want(&url, LOC, USR), live_want(&url, LOC, usr2));
    a1.sync(Some(w1.clone()), None);
    a2.sync(Some(w2), None);
    for hub in [a1, a2] {
        wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    }
    a1.sync(Some(w1.clone()), Some(dealt(10)));
    wait_for(|| live_seen(a2, w1.member(), 10).then_some(()));
    crate::console::say(format_args!("ready for restart"));
    let end = Instant::now() + Duration::from_secs(120);
    while a1.poll().link == Link::Connected {
        assert!(Instant::now() < end, "server never restarted");
        std::thread::sleep(Duration::from_millis(100));
    }
    let lost = Instant::now();
    for hub in [a1, a2] {
        let end = Instant::now() + Duration::from_secs(60);
        while hub.poll().link != Link::Connected {
            assert!(Instant::now() < end, "no reconnect");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    crate::console::say(format_args!(
        "rejoined after {} ms",
        lost.elapsed().as_millis()
    ));
    a1.sync(Some(w1.clone()), Some(dealt(20)));
    let end = Instant::now() + Duration::from_secs(10);
    while !live_seen(a2, w1.member(), 20) {
        assert!(Instant::now() < end, "no snap after rejoin");
        std::thread::sleep(Duration::from_millis(50));
    }
    for hub in [a1, a2] {
        assert!(hub.shutdown(Duration::from_secs(5)));
    }
}

#[test]
#[ignore = "needs the live relay"]
fn live_relay_catches_up_a_late_joiner() {
    let url = std::env::var("EHUD_E2E_URL").expect("EHUD_E2E_URL");
    let usr2 = "usr_55555555-5555-5555-5555-555555555555";
    let (sharer, late) = (hub(), hub());
    let (w1, w2) = (live_want(&url, LOC, USR), live_want(&url, LOC, usr2));
    sharer.sync(Some(w1), None);
    wait_for(|| (sharer.poll().link == Link::Connected).then_some(()));
    let sample = super::super::proto::tests::sample();
    sharer.share_run(Some(sample.clone()), 1500);
    std::thread::sleep(Duration::from_millis(RUN_GAP_MS + 2500));
    late.sync(Some(w2.clone()), None);
    let runs = wait_for(|| Some(late.poll().runs).filter(|r| !r.is_empty()));
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].room, w2.room());
    assert!((2..=4).contains(&runs[0].held), "{}", runs[0].held);
    let got = runs[0].summary(1500 + u64::from(runs[0].held)).unwrap();
    assert_eq!(got.stages, sample.stages);
    assert_eq!(got.marks.len(), sample.marks.len());
    for (a, b) in got.marks.iter().zip(&sample.marks) {
        assert_eq!((a.name.as_str(), a.boss), (b.name.as_str(), b.boss));
        assert!(b.start.abs_diff(a.start) <= 10, "{} {}", a.start, b.start);
    }
    let more = Summary {
        stages: Some(4),
        ..sample
    };
    sharer.share_run(Some(more), 1600);
    let runs = wait_for(|| Some(late.poll().runs).filter(|r| !r.is_empty()));
    assert_eq!(runs[0].summary(1600).unwrap().stages, Some(4));
    sharer.share_run(None, 1700);
    std::thread::sleep(Duration::from_millis(2500));
    assert!(late.poll().runs.is_empty());
    assert!(sharer.poll().runs.is_empty());
    for h in [sharer, late] {
        assert!(h.shutdown(Duration::from_secs(5)));
    }
}

#[test]
fn client_shares_its_run_and_reads_peer_runs() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let w = test_want(port, LOC);
    hub.sync(Some(w.clone()), None);
    let mut p = Peer::accept(&l);
    assert_eq!(p.data().unwrap().1[0], 0x01);
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    assert_eq!(p.run(), [RUN]);
    let sample = super::super::proto::tests::sample();
    hub.share_run(Some(sample.clone()), 1500);
    let sent = p.run();
    let got = decode_run(&sent[1..], 1500).unwrap();
    let shift = sample.start - got.start;
    assert!(shift <= 10, "{shift}");
    assert_eq!(got.stages, Some(3));
    for (a, b) in got.marks.iter().zip(&sample.marks) {
        assert_eq!(
            (a.name.as_str(), a.start + shift),
            (b.name.as_str(), b.start)
        );
    }
    let mut frame = vec![RUNS];
    frame.extend_from_slice(&42u32.to_le_bytes());
    frame.extend_from_slice(&sent[1..]);
    p.send(0x2, &frame);
    let runs = wait_for(|| Some(hub.poll().runs).filter(|r| !r.is_empty()));
    assert_eq!(runs.len(), 1);
    assert_eq!((runs[0].held, runs[0].room), (42, w.room()));
    assert_eq!(runs[0].summary(1542 + shift), Some(sample));
    p.send(0x2, &[RUNS, 1, 2]);
    p.send(0x2, &[0x03]);
    let snaps = wait_for(|| Some(hub.poll().snaps).filter(|s| !s.is_empty()));
    assert_eq!(snaps[0].entries.len(), 0);
    assert_eq!(hub.poll().link, Link::Connected);
    hub.share_run(None, 1600);
    assert_eq!(p.run(), [RUN]);
    let closed = p.close_later();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(closed.join().unwrap(), 1001);
}
