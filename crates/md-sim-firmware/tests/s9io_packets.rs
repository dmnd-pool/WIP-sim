use md_sim_firmware::s9io::{
    command_frame, response_words, work_frame, Error, Midstates, Receiver, Response,
};
use serde::Deserialize;
use std::collections::BTreeMap;

type Arrays = BTreeMap<String, Vec<u32>>;

#[derive(Deserialize)]
struct Reference {
    member_sha256: String,
    tasks: BTreeMap<String, Arrays>,
}

fn reference() -> Reference {
    let reference: Reference =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    assert_eq!(
        reference.member_sha256,
        "8bd72eecd9e5706fdcbdd756f9b8084eaffabf0e72a1624ce09c11a11b973889"
    );
    reference
}

fn uart(arrays: &Arrays, name: &str) -> Vec<u8> {
    arrays[name]
        .iter()
        .map(|v| u8::try_from(*v).unwrap())
        .collect()
}

#[test]
fn transmit_bytes_match_archived_hdl_testbench() {
    let r = reference();
    for name in ["tc_send_cmd5", "tc_send_cmd9"] {
        let arrays = &r.tasks[name];
        for (key, expected) in arrays.iter().filter(|(key, _)| key.starts_with("uart")) {
            let input = &arrays[&key.replace("uart", "fifo")];
            let actual = command_frame(input).unwrap();
            assert_eq!(
                actual.iter().copied().map(u32::from).collect::<Vec<_>>(),
                *expected,
                "{name}/{key}"
            );
        }
    }
    for (name, mode) in [
        ("tc_send_work_midstate1", Midstates::One),
        ("tc_send_work_midstate4", Midstates::Four),
    ] {
        let arrays = &r.tasks[name];
        assert_eq!(
            work_frame(&arrays["fifo_data1"], mode).unwrap(),
            uart(arrays, "uart_data1"),
            "{name}"
        );
    }
}

#[test]
fn receive_bytes_and_extended_ids_match_archived_hdl_testbench() {
    let r = reference();
    for name in ["tc_cmd_response", "tc_work_response"] {
        let arrays = &r.tasks[name];
        for key in arrays.keys().filter(|key| key.starts_with("uart")) {
            let bytes: [u8; 7] = uart(arrays, key).try_into().unwrap();
            let expected = arrays[&key.replace("uart", "fifo")]
                .clone()
                .try_into()
                .unwrap();
            let expected = if name == "tc_cmd_response" {
                Response::Command(expected)
            } else {
                Response::Work(expected)
            };
            assert_eq!(
                response_words(&bytes, 0x7f, Midstates::One),
                Ok(expected),
                "{name}/{key}"
            );
        }
    }
    for index in 1..=5 {
        let arrays = &r.tasks[&format!("tc_work_id_{index}")];
        let mode = if index == 5 {
            Midstates::Four
        } else {
            Midstates::One
        };
        let last_id = arrays["fifo_data1"][0] as u16;
        // Also check each work that establishes the source test's last ID.
        assert_eq!(
            work_frame(&arrays["fifo_data1"], mode).unwrap(),
            uart(arrays, "uart_data1")
        );
        let bytes = uart(arrays, "uart_data2").try_into().unwrap();
        assert_eq!(
            response_words(&bytes, last_id, mode),
            Ok(Response::Work(
                arrays["fifo_data2"].clone().try_into().unwrap()
            )),
            "extended ID case{index}"
        );
    }
}

#[test]
fn corrupted_stream_resynchronizes_with_the_recorded_error_counts() {
    let r = reference();
    let arrays = &r.tasks["tc_error_counter_1"];
    let mut receiver = Receiver::default();
    for byte in uart(arrays, "uart_data1") {
        assert!(!matches!(
            receiver.byte(byte, 0x7f, Midstates::One),
            Some(Ok(_))
        ));
    }
    assert_eq!(receiver.errors(), 1);
    for byte in uart(arrays, "uart_data2") {
        assert!(!matches!(
            receiver.byte(byte, 0x7f, Midstates::One),
            Some(Ok(_))
        ));
    }
    assert_eq!(receiver.errors(), 8);
    receiver.clear_errors();
    assert_eq!(receiver.errors(), 0);
    receiver.reset();
    let arrays = &r.tasks["tc_error_counter_2"];
    let mut received = vec![];
    for byte in uart(arrays, "uart_data1")
        .into_iter()
        .chain(uart(arrays, "uart_data2"))
    {
        if let Some(Ok(response)) = receiver.byte(byte, 0x7f, Midstates::One) {
            received.push(response);
        }
    }
    assert_eq!(receiver.errors(), 4);
    assert_eq!(received, vec![Response::Work([0x83ea0372, 0x97003900])]);
    receiver.reset();
    assert_eq!(receiver.errors(), 0);
    // Reset must discard a partial response, not only the error counter.
    receiver.byte(0x13, 0, Midstates::One);
    receiver.byte(0x87, 0, Midstates::One);
    receiver.reset();
    let bytes = [0x13, 0x87, 0x90, 0xf4, 0, 0, 0x1c];
    let replies: Vec<_> = bytes
        .into_iter()
        .filter_map(|byte| receiver.byte(byte, 0, Midstates::One))
        .collect();
    assert_eq!(
        replies,
        vec![Ok(Response::Command([0xf4908713, 0x1c000000]))]
    );
}

#[test]
fn invalid_lengths_and_real_hardware_flag_behavior_are_explicit() {
    assert_eq!(command_frame(&[]), Err(Error::Length));
    assert_eq!(command_frame(&[0x00000948]), Err(Error::Length));
    assert_eq!(
        command_frame(&[0x00000d48, 0, 0]),
        Err(Error::UnsupportedCommandLength)
    );
    for mode in [Midstates::One, Midstates::Two, Midstates::Four] {
        let words = vec![0; mode.work_words()];
        assert_eq!(
            work_frame(&words[..words.len() - 1], mode),
            Err(Error::Length)
        );
        let frame = work_frame(&words, mode).unwrap();
        assert_eq!(frame.len(), 22 + 32 * mode.count());
        assert_eq!(usize::from(frame[1]), frame.len());
        assert_eq!(usize::from(frame[3]), mode.count());
    }
    let original = [0x72, 3, 0xea, 0x83, 0, 3, 0x98];
    // Unlike BM1397's codec, this FPGA ignores reserved receive flag bits.
    for flags in [0x20, 0x40, 0x60] {
        let mut frame = original;
        frame[6] |= flags;
        assert_eq!(
            response_words(&frame, 0x7f, Midstates::One),
            Ok(Response::Work([
                0x83ea0372,
                u32::from(frame[6]) << 24 | 0x300
            ]))
        );
    }
    // Extended ID wraps at16 bits, and low midstate bits do not select the
    // previous generation when they refer to the most recently sent work.
    assert_eq!(
        response_words(&original, 0, Midstates::One),
        Ok(Response::Work([0x83ea0372, 0x98ff8300]))
    );
    assert_eq!(
        response_words(&original, 0, Midstates::Four),
        Ok(Response::Work([0x83ea0372, 0x98000300]))
    );
    for bit in 0..56 {
        if bit == 53 || bit == 54 {
            continue; // the two documented ignored bits
        }
        let mut corrupt = original;
        corrupt[bit / 8] ^= 1 << (bit % 8);
        assert_eq!(
            response_words(&corrupt, 0x7f, Midstates::One),
            Err(Error::Checksum),
            "bit{bit}"
        );
    }
}
