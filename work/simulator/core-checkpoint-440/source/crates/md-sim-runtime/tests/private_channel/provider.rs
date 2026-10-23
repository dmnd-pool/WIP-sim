use super::*;
use md_sim_runtime::private_channel::relay_provider;
use std::os::{fd::OwnedFd, unix::net::UnixStream};

fn pair() -> (File, File) {
    let (a, b) = UnixStream::pair().unwrap();
    (File::from(OwnedFd::from(a)), File::from(OwnedFd::from(b)))
}

#[test]
fn trusted_provider_reuses_framing_caps_and_has_its_own_deadline() {
    let (mut guest, guest_relay) = pair();
    let (peer, peer_relay) = pair();
    let relay = thread::spawn(move || {
        relay_provider(
            guest_relay,
            peer_relay,
            policy(),
            Arc::new(AtomicBool::new(false)),
            3,
        )
        .unwrap()
    });
    let responder = thread::spawn(move || {
        let mut file = peer;
        assert_eq!(transfer(&mut file, &frame(82, 80), 68), frame(81, 64));
        file
    });
    assert_eq!(transfer(&mut guest, &frame(81, 64), 84), frame(82, 80));
    let peer = responder.join().unwrap();
    drop(peer);
    assert!(transfer(&mut guest, &[], usize::MAX).is_empty());
    let receipt = relay.join().unwrap();
    assert_eq!(receipt.end, "peer-eof");
    assert_eq!(receipt.guest_to_peer.forwarded_bytes, 68);
    assert_eq!(receipt.peer_to_guest.forwarded_bytes, 84);
    assert_eq!(
        receipt.guest_to_peer.forwarded_sha256,
        hex::encode(Sha256::digest(frame(81, 64)))
    );
    assert_eq!(
        receipt.peer_to_guest.forwarded_sha256,
        hex::encode(Sha256::digest(frame(82, 80)))
    );
    assert!(receipt.error.is_none());

    let (_outside, inside) = pair();
    assert!(relay_provider(
        File::open("/dev/null").unwrap(),
        inside,
        policy(),
        Arc::new(AtomicBool::new(false)),
        1
    )
    .is_err());
    let (_a, b) = pair();
    let (_c, d) = pair();
    assert!(relay_provider(b, d, policy(), Arc::new(AtomicBool::new(false)), 0).is_err());
    let (_a, b) = pair();
    let (_c, d) = pair();
    assert_eq!(
        relay_provider(b, d, policy(), Arc::new(AtomicBool::new(true)), 1)
            .unwrap()
            .end,
        "cancelled"
    );
    let (_a, b) = pair();
    let (_c, d) = pair();
    let start = Instant::now();
    let receipt = relay_provider(b, d, policy(), Arc::new(AtomicBool::new(false)), 1).unwrap();
    assert_eq!(receipt.end, "timed-out");
    assert!(start.elapsed() < Duration::from_secs(2));

    let (mut a, b) = pair();
    let (_c, d) = pair();
    a.write_all(&2049u32.to_be_bytes()).unwrap();
    let receipt = relay_provider(b, d, policy(), Arc::new(AtomicBool::new(false)), 1).unwrap();
    assert_eq!(receipt.end, "failed");
    assert!(receipt.error.unwrap().contains("oversized"));
    assert_eq!(receipt.guest_to_peer.forwarded_bytes, 0);
}
