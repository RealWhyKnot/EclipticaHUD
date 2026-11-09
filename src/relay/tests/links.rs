use super::*;

#[test]
fn client_joins_sends_damage_and_reads_snaps() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let w = test_want(port, LOC);
    hub.sync(Some(w.clone()), None);
    let mut p = Peer::accept(&l);
    assert_eq!(p.path, "/ws");
    let (op, join) = p.data().unwrap();
    assert_eq!(op, 0x2);
    assert_eq!(join.len(), 41);
    assert_eq!(
        crate::relay::sha256::hex(&join[..25]),
        "016e35fa9ce4780b2a38a74e3b8d9ce66b6a59070463d1200d"
    );
    assert_eq!(join[25..], claim_hash(&[0x42; 32], LOC));
    assert_eq!(w.claim(), claim_hash(&[0x42; 32], LOC));
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    hub.sync(Some(w.clone()), Some(dealt(100)));
    let (op, d) = p.data().unwrap();
    let first = Instant::now();
    assert_eq!((op, Msg::decode(&d)), (0x2, Some(Msg::Dmg(dealt(100)))));
    hub.sync(Some(w.clone()), Some(dealt(200)));
    hub.sync(Some(w.clone()), Some(dealt(300)));
    let (_, d) = p.data().unwrap();
    assert_eq!(Msg::decode(&d), Some(Msg::Dmg(dealt(300))));
    assert!(first.elapsed() >= Duration::from_millis(850));
    let other = [7u8; 8];
    p.send(
        0x2,
        &Msg::Snap(vec![
            (member_hash(LOC, USR), dealt(300)),
            (other, Totals::from((4242, 77))),
        ])
        .encode(),
    );
    let snaps = wait_for(|| Some(hub.poll().snaps).filter(|s| !s.is_empty()));
    assert_eq!(snaps[0].room, room_hash(LOC));
    assert_eq!(snaps[0].entries[1], (other, Totals::from((4242, 77))));
    p.send(0x2, &[0x03]);
    let snaps = wait_for(|| Some(hub.poll().snaps).filter(|s| !s.is_empty()));
    assert_eq!(snaps[0].entries.len(), 0);
    hub.sync(Some(w), None);
    let (_, d) = p.data().unwrap();
    assert_eq!(Msg::decode(&d), Some(Msg::End));
    hub.sync(None, None);
    assert_eq!(p.expect_close(), 1001);
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(hub.poll().link, Link::Off);
}

#[test]
fn client_reconnects_when_location_changes() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    let (_, join) = a.data().unwrap();
    assert_eq!(&join[1..17], &room_hash(LOC));
    let other = "wrld_00000000-0000-0000-0000-000000000000:99999~region(eu)";
    hub.sync(Some(test_want(port, other)), Some(dealt(5)));
    assert_eq!(a.expect_close(), 1000);
    let mut b = Peer::accept(&l);
    let (_, join) = b.data().unwrap();
    assert_eq!(
        Msg::decode(&join),
        Some(Msg::Join {
            room: room_hash(other),
            member: member_hash(other, USR),
            claim: claim_hash(&[0x42; 32], other),
        })
    );
    let (_, d) = b.data().unwrap();
    assert_eq!(Msg::decode(&d), Some(Msg::Dmg(dealt(5))));
    let closed = b.close_later();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(closed.join().unwrap(), 1001);
}

#[test]
fn client_closes_on_bad_frames_and_backs_off() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut first = Peer::accept(&listener);
    first.data().unwrap();
    first.send(0x1, b"hello");
    let shut = Instant::now();
    assert_eq!(first.expect_close(), 1008);
    wait_for(|| (hub.poll().link == Link::Retrying).then_some(()));
    let mut b = Peer::accept(&listener);
    let gap = shut.elapsed();
    assert!(gap >= Duration::from_millis(1900), "{gap:?}");
    assert!(gap <= Duration::from_millis(4000), "{gap:?}");
    b.data().unwrap();
    b.send(0x2, &[0x03, 1, 2]);
    assert_eq!(b.expect_close(), 1008);
    let mut second = Peer::accept(&listener);
    second.data().unwrap();
    let mut huge = vec![0x03];
    huge.resize(1 + 16 * (proto::MAX_MEMBERS + 1), 0);
    second.send(0x2, &huge);
    assert_eq!(second.expect_close(), 1008);
    let start = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn refused_backs_off_to_the_maximum() {
    let mut b = Backoff::default();
    for r in [0, 1, 7, 15_000, 15_001, u64::MAX, rand()] {
        let ms = b.max_ms(r);
        assert!((60_000..=75_000).contains(&ms), "{ms}");
    }
    b.reset();
    assert_eq!(b.next_ms(0), 2000);
    assert_eq!(b.max_ms(0), 60_000);
    assert_eq!(b.next_ms(0), 60_000);
}

#[test]
fn room_full_backs_off_to_the_maximum() {
    refused_by_server(client::ROOM_FULL, Link::Full);
}

#[test]
fn member_taken_backs_off_to_the_maximum() {
    refused_by_server(client::MEMBER_TAKEN, Link::Taken);
}

#[test]
fn no_join_once_sharing_is_off() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept_then(&l, || hub.sync(None, None));
    if let Some((op, _)) = a.data() {
        assert_eq!(op, 0x8);
    }
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(hub.poll().link, Link::Off);
}

#[test]
fn quiet_socket_stays_open() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    a.data().unwrap();
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(14) {
        assert_eq!(hub.poll().link, Link::Connected);
        std::thread::sleep(Duration::from_millis(250));
    }
    a.send(0x2, &[0x03]);
    let snaps = wait_for(|| Some(hub.poll().snaps).filter(|s| !s.is_empty()));
    assert_eq!(snaps[0].entries.len(), 0);
    let closed = a.close_later();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(closed.join().unwrap(), 1001);
}

#[test]
fn shutdown_gives_up_on_a_server_that_never_answers_close() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    a.data().unwrap();
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    let t = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(8)));
    let took = t.elapsed();
    assert!(took >= Duration::from_millis(1900), "{took:?}");
    assert!(took < Duration::from_secs(5), "{took:?}");
    assert_eq!(hub.poll().link, Link::Off);
    drop(a);
}

#[test]
fn unreachable_server_retries_without_blocking_sync() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let t = Instant::now();
    hub.sync(Some(test_want(port, LOC)), Some(dealt(1)));
    assert!(t.elapsed() < Duration::from_millis(50));
    wait_for(|| (hub.poll().link == Link::Retrying).then_some(()));
    let t = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert!(t.elapsed() < Duration::from_secs(1));
    assert_eq!(hub.poll().link, Link::Off);
}
