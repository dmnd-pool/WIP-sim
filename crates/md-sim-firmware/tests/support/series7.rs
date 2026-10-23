// Shared fixed configuration vectors. No production encoder is used.
const ID: u32 = 0x03722093;

pub fn small_database() -> Vec<u8> {
    br#"{"idcode":57811091,"global_clock_regions":{
      "top":{"rows":{"0":{"configuration_buses":{
        "CLB_IO_CLK":{"configuration_columns":{"0":{"frame_count":1},"10":{"frame_count":2},"2":{"frame_count":1}}},
        "BLOCK_RAM":{"configuration_columns":{"0":{"frame_count":1}}}
      }}}},
      "bottom":{"rows":{"0":{"configuration_buses":{
        "CLB_IO_CLK":{"configuration_columns":{"0":{"frame_count":1}}},
        "BLOCK_RAM":{"configuration_columns":{"0":{"frame_count":1}}}
      }}}}
    }}"#.to_vec()
}

pub fn payload() -> Vec<u32> {
    let mut words = Vec::new();
    let mut frame = [0x00110011; 101];
    frame[50] = 0x00111004; // Stored ECC1004 from the independent vector.
    for count in [4, 1, 1, 1] {
        for _ in 0..count {
            words.extend(frame);
        }
        words.extend([0; 202]);
    }
    words
}

// Fixed CRC vectors were computed separately from the production code with
// Project X-Ray's published 37-bit ICAP algorithm. Each negative fixture has
// a valid packet CRC, so a failure must come from frame/padding validation.
pub fn stream(payload: &[u32], far: u32, crc: u32) -> Vec<u8> {
    let mut data = vec![
        0xffffffff,
        0xbb,
        0x11220044,
        0xffffffff,
        0xaa995566,
        0x30008001,
        7,
        0x30018001,
        ID,
        0x30002001,
        far,
        0x30008001,
        1,
        0x30004000,
        0x50000000 | payload.len() as u32,
    ];
    data.extend_from_slice(payload);
    data.extend([
        0x30000001, crc, 0x30008001, 10, 0x30008001, 3, 0x30008001, 5, 0x30002001, 0x03be0000,
        0x3000c001, 0x501, 0x3000a001, 0x501, 0x30000001, 0xe3ad7ea5, 0x30008001, 13,
    ]);
    data.iter().flat_map(|v| v.to_be_bytes()).collect()
}
