use bitcoin_hashes::{sha256, Hash};
use md_sim_firmware::pic16::{Error, Mode, Pic, Program, APP_HASH, APP_START, APP_WORDS};

const APP: &str = include_str!("../../../simulator/firmware/pic16/braiins-s9-app.txt");

fn pic() -> Pic {
    Pic::with_erased_development_storage(&Program::parse(APP).unwrap())
}
fn command(p: &mut Pic, now: u64, command: u8, data: &[u8]) -> Result<(), Error> {
    for byte in [0x55, 0xaa, command].iter().chain(data) {
        p.write_byte(now, *byte)?;
        p.stop(); // Real driver's one byte per I2C transaction.
    }
    Ok(())
}
fn read(p: &mut Pic, now: u64, count: usize) -> Vec<u8> {
    (0..count)
        .map(|_| {
            assert!(p.read_ready(now).unwrap());
            let byte = p.read_byte(now).unwrap();
            assert!(byte.defined);
            // The driver requests a longer read but only uses the first byte.
            let suffix = p.read_byte(now).unwrap();
            assert!(!suffix.defined);
            assert_eq!(suffix.value, byte.value);
            p.stop();
            byte.value
        })
        .collect()
}

#[test]
fn exact_archived_program_identity_and_strict_format() {
    assert_eq!(
        sha256::Hash::hash(APP.as_bytes()).to_string(),
        "ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a"
    );
    let program = Program::parse(APP).unwrap();
    assert_eq!(program.word_bytes().len(), 6400);
    assert_eq!(program.sha256(), APP_HASH);
    assert_eq!(
        sha256::Hash::hash(&program.word_bytes()).to_string(),
        APP_HASH
    );
    assert!(Program::parse(&APP.replace("3183", "FFFF")).is_err());
    assert!(Program::parse(&APP.replacen("3183", " 3183", 1)).is_err());
    assert!(Program::parse(&APP[..APP.len() - 6]).is_err());
    assert!(Program::parse(&format!("{APP}0000\n")).is_err());
}

#[test]
fn heartbeat_expiry_cuts_power_and_keeps_the_application_available() {
    let mut p = pic();
    assert_eq!(command(&mut p, 0, 0x17, &[]), Err(Error::Mode));
    command(&mut p, 0, 6, &[]).unwrap();
    assert_eq!(p.mode(), Mode::ApplicationContract);
    assert!(!p.ready(99_999_999).unwrap());
    command(&mut p, 100_000_000, 0x17, &[]).unwrap();
    assert_eq!(read(&mut p, 100_000_000, 1), [3]);
    assert_eq!(
        command(&mut p, 100_000_000, 0x18, &[]),
        Err(Error::UnknownPowerOnVoltage)
    );
    assert_eq!(
        command(&mut p, 100_000_000, 0x15, &[1]),
        Err(Error::UnknownPowerOnVoltage)
    );
    command(&mut p, 100_000_000, 0x10, &[64]).unwrap();
    assert_eq!(command(&mut p, 199_999_999, 0x15, &[1]), Err(Error::Busy));
    command(&mut p, 200_000_000, 0x15, &[1]).unwrap();
    command(&mut p, 200_000_000, 0x18, &[]).unwrap();
    assert_eq!(read(&mut p, 200_000_000, 1), [64]);
    command(&mut p, 200_000_000, 0x16, &[]).unwrap();
    p.advance(10_199_999_999).unwrap();
    assert!(p.voltage_enabled());
    p.advance(10_200_000_000).unwrap();
    assert!(!p.voltage_enabled());
    assert_eq!(p.mode(), Mode::ApplicationContract);
    assert_eq!(p.next_deadline(), None);
    command(&mut p, 10_200_000_001, 0x16, &[]).unwrap();
    // A received heartbeat resets the software counter, not the power pin.
    assert!(!p.voltage_enabled());
    assert_eq!(p.next_deadline(), None);
    command(&mut p, 10_200_000_001, 0x17, &[]).unwrap();
    assert_eq!(read(&mut p, 10_200_000_001, 1), [3]);
    command(&mut p, 10_200_000_001, 0x18, &[]).unwrap();
    assert_eq!(read(&mut p, 10_200_000_001, 1), [64]);
    p.reset(10_200_000_002).unwrap();
    assert_eq!(p.mode(), Mode::Loader);
    assert!(!p.ready(10_700_000_001).unwrap());
    assert!(p.ready(10_700_000_002).unwrap());
}

#[test]
fn power_off_pauses_heartbeat_time_without_forgiving_elapsed_power_on_time() {
    let mut p = pic();
    command(&mut p, 0, 6, &[]).unwrap();
    // The app starts with RA2 high. Its main loop skips both counters.
    p.advance(20_000_000_000).unwrap();
    assert_eq!(p.next_deadline(), None);
    command(&mut p, 20_000_000_000, 0x10, &[64]).unwrap();
    command(&mut p, 21_000_000_000, 0x15, &[1]).unwrap();
    command(&mut p, 21_000_000_000, 0x16, &[]).unwrap();
    assert_eq!(p.next_deadline(), Some(31_000_000_000));
    command(&mut p, 24_000_000_000, 0x15, &[0]).unwrap();
    assert_eq!(p.next_deadline(), None);
    p.advance(50_000_000_000).unwrap();
    command(&mut p, 50_000_000_000, 0x15, &[0]).unwrap();
    command(&mut p, 51_000_000_000, 0x15, &[1]).unwrap();
    assert_eq!(p.next_deadline(), Some(58_000_000_000));
    // Repeating ENABLE also must not act as a heartbeat.
    command(&mut p, 52_000_000_000, 0x15, &[1]).unwrap();
    assert_eq!(p.next_deadline(), Some(58_000_000_000));
    p.advance(57_999_999_999).unwrap();
    assert!(p.voltage_enabled());
    p.advance(58_000_000_000).unwrap();
    assert!(!p.voltage_enabled());
    // The cutoff path clears the heartbeat counter. Only ENABLE restores power.
    command(&mut p, 59_000_000_000, 0x15, &[1]).unwrap();
    assert!(p.voltage_enabled());
    assert_eq!(p.next_deadline(), Some(69_000_000_000));
}

#[test]
fn received_heartbeat_while_disabled_resets_only_the_paused_counter() {
    let mut p = pic();
    command(&mut p, 0, 6, &[]).unwrap();
    command(&mut p, 100_000_000, 0x10, &[64]).unwrap();
    command(&mut p, 200_000_000, 0x15, &[1]).unwrap();
    command(&mut p, 200_000_000, 0x16, &[]).unwrap();
    command(&mut p, 9_000_000_000, 0x15, &[0]).unwrap();
    command(&mut p, 15_000_000_000, 0x16, &[]).unwrap();
    assert!(!p.voltage_enabled());
    assert_eq!(p.next_deadline(), None);
    command(&mut p, 40_000_000_000, 0x15, &[1]).unwrap();
    assert_eq!(p.next_deadline(), Some(50_000_000_000));
    // Expiry is applied before a command at that exact virtual time.
    command(&mut p, 50_000_000_000, 0x16, &[]).unwrap();
    assert!(!p.voltage_enabled());
    assert_eq!(p.mode(), Mode::ApplicationContract);
}

#[test]
fn documented_parser_flush_and_non_rematching_prefix() {
    let mut p = pic();
    for byte in [0x55, 0x55, 0xaa, 6] {
        p.write_byte(0, byte).unwrap();
        p.stop();
    }
    assert_eq!(p.mode(), Mode::Loader);
    // Flush an unfinished16-byte buffer command exactly as the source driver.
    command(&mut p, 0, 2, &[0, 1, 2]).unwrap();
    command(&mut p, 0, 0, &[0; 16]).unwrap();
    command(&mut p, 0, 7, &[]).unwrap();
    assert!(!p.ready(499_999_999).unwrap());
    assert!(p.ready(500_000_000).unwrap());
    command(&mut p, 500_000_000, 1, &[0x0f, 0x80]).unwrap();
    command(&mut p, 500_000_000, 8, &[]).unwrap();
    assert_eq!(read(&mut p, 500_000_000, 2), [0x0f, 0x80]);
}

#[test]
fn interrupted_application_prefix_consumes_data_without_an_i2c_reply() {
    let mut p = pic();
    command(&mut p, 0, 6, &[]).unwrap();
    let now = 100_000_000;
    command(&mut p, now, 0x10, &[64]).unwrap();
    let now = 200_000_000;
    command(&mut p, now, 0x15, &[1]).unwrap();
    command(&mut p, now, 0x16, &[]).unwrap();
    let deadline = p.next_deadline();
    let flash = p.flash_words().to_vec();
    // Last successful bytes from the failed native power-loss restart190.
    // The old process stops after55 AA. Its replacement starts a flush.
    for byte in [0x55, 0xaa, 0x55] {
        p.write_byte(now, byte).unwrap();
        p.stop();
    }
    assert!(!p.read_ready(now).unwrap());
    // Static words04f0..04f3 emit EE on UART, not on the I2C reply path.
    p.write_byte(now, 0xaa).unwrap();
    p.stop();
    assert!(!p.read_ready(now).unwrap());
    assert_eq!(p.read_byte(now), Err(Error::NoReply));
    assert!(p.voltage_enabled());
    assert_eq!(p.next_deadline(), deadline);
    assert_eq!(p.flash_words(), flash);
    // Finish the interrupted flush and repeat its ordinary complete form.
    for byte in [0; 17] {
        p.write_byte(now, byte).unwrap();
        p.stop();
    }
    command(&mut p, now, 0, &[0; 16]).unwrap();
    assert!(!p.read_ready(now).unwrap());
    command(&mut p, now, 0x17, &[]).unwrap();
    assert_eq!(read(&mut p, now, 1), [3]);
    command(&mut p, now, 7, &[]).unwrap();
    assert_eq!(p.mode(), Mode::Loader);
    assert!(!p.voltage_enabled());
    assert!(!p.ready(now + 499_999_999).unwrap());
    assert!(p.ready(now + 500_000_000).unwrap());
    // The application evidence does not define an unknown loader command.
    assert_eq!(
        command(&mut p, now + 500_000_000, 0x55, &[]),
        Err(Error::UnsupportedCommand(0x55))
    );
}

#[test]
fn exact_application_enable_branch_accepts_every_nonzero_byte() {
    let mut p = pic();
    command(&mut p, 0, 6, &[]).unwrap();
    command(&mut p, 100_000_000, 0x10, &[64]).unwrap();
    let now = 200_000_000;
    for value in 1..=255 {
        command(&mut p, now, 0x15, &[value]).unwrap();
        assert!(p.voltage_enabled());
        assert_eq!(p.voltage_code(), Some(64));
        assert_eq!(p.next_deadline(), Some(now + 10_000_000_000));
        assert!(!p.read_ready(now).unwrap());
        command(&mut p, now, 0x15, &[0]).unwrap();
        assert!(!p.voltage_enabled());
    }
    // A newly started host's first prefix byte is data for the interrupted
    // enable command. Do not hide this real transient by rejecting55.
    for byte in [0x55, 0xaa, 0x15, 0x55, 0xaa, 0] {
        p.write_byte(now, byte).unwrap();
        p.stop();
    }
    assert!(p.voltage_enabled());
    for byte in [0; 16] {
        p.write_byte(now, byte).unwrap();
        p.stop();
    }
    command(&mut p, now, 7, &[]).unwrap();
    assert!(!p.voltage_enabled());
    assert_eq!(p.mode(), Mode::Loader);
}

#[test]
fn interrupted_flash_changes_do_not_commit_and_committed_cells_persist() {
    let mut p = pic();
    command(&mut p, 0, 1, &[0x0f, 0x80]).unwrap();
    command(&mut p, 0, 2, &[0; 16]).unwrap();
    command(&mut p, 0, 5, &[]).unwrap();
    assert_eq!(&p.flash_words()[0xf80..0xf88], &[0x3fff; 8]);
    p.reset(99_999_999).unwrap();
    assert_eq!(&p.flash_words()[0xf80..0xf88], &[0x3fff; 8]);
    let t = 600_000_000;
    command(&mut p, t, 1, &[0x0f, 0x80]).unwrap();
    command(&mut p, t, 2, &[0; 16]).unwrap();
    command(&mut p, t, 5, &[]).unwrap();
    p.advance(t + 100_000_000).unwrap();
    assert_eq!(&p.flash_words()[0xf80..0xf88], &[0; 8]);
    p.reset(t + 100_000_000).unwrap();
    assert_eq!(&p.flash_words()[0xf80..0xf88], &[0; 8]);
    let t = t + 600_000_000;
    command(&mut p, t, 1, &[0x0f, 0x80]).unwrap();
    command(&mut p, t, 4, &[]).unwrap();
    p.advance(t + 100_000_000).unwrap();
    assert_eq!(&p.flash_words()[0xf80..0xfa0], &[0x3fff; 32]);
}

#[test]
fn full_program_update_and_bad_image_rejection_use_archived_bytes() {
    let mut p = pic();
    let mut now = 0;
    command(&mut p, now, 1, &[3, 0]).unwrap();
    for _ in 0..APP_WORDS / 32 {
        command(&mut p, now, 4, &[]).unwrap();
        now += 100_000_000;
        p.advance(now).unwrap();
    }
    assert_eq!(command(&mut p, now, 6, &[]), Err(Error::UnknownApplication));
    command(&mut p, now, 1, &[3, 0]).unwrap();
    let bytes = Program::parse(APP).unwrap().word_bytes();
    for block in bytes.chunks_exact(16) {
        command(&mut p, now, 2, block).unwrap();
        command(&mut p, now, 5, &[]).unwrap();
        now += 100_000_000;
        p.advance(now).unwrap();
    }
    command(&mut p, now, 8, &[]).unwrap();
    assert_eq!(read(&mut p, now, 2), [0x0f, 0x80]);
    command(&mut p, now, 1, &[3, 0]).unwrap();
    let mut actual = Vec::new();
    for _ in 0..APP_WORDS / 8 {
        command(&mut p, now, 3, &[]).unwrap();
        actual.extend(read(&mut p, now, 16));
    }
    assert_eq!(actual, bytes);
    assert_eq!(sha256::Hash::hash(&actual).to_string(), APP_HASH);
    assert_eq!(p.flash_words()[APP_START], 0x3183);
    command(&mut p, now, 6, &[]).unwrap();
    assert_eq!(p.mode(), Mode::ApplicationContract);
}

#[test]
fn invalid_ranges_commands_and_overflow_do_not_change_flash() {
    let mut p = pic();
    let original = p.flash_words().to_vec();
    assert_eq!(command(&mut p, 0, 1, &[0x10, 0]), Err(Error::FlashRange));
    command(&mut p, 0, 1, &[3, 1]).unwrap();
    assert_eq!(command(&mut p, 0, 4, &[]), Err(Error::FlashRange));
    assert_eq!(command(&mut p, 0, 5, &[]), Err(Error::FlashRange));
    assert_eq!(
        command(&mut p, 0, 0x23, &[]),
        Err(Error::UnsupportedCommand(0x23))
    );
    command(&mut p, u64::MAX, 1, &[3, 0]).unwrap();
    assert_eq!(command(&mut p, u64::MAX, 4, &[]), Err(Error::Time));
    p.advance(u64::MAX).unwrap();
    assert_eq!(p.flash_words(), original);
    assert_eq!(p.reset(u64::MAX), Err(Error::Time));
    assert_eq!(p.advance(0), Err(Error::Time));
}
