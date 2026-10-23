use super::pic16_qtest::Script;
use std::collections::BTreeMap;

fn read(s: &mut Script, address: u32, value: u32) {
    s.line(format!("readl 0x{address:x}"), format!("OK 0x{value:016x}"));
}
fn query_all(s: &mut Script) {
    for base in [0x43c00000u32, 0x43c10000, 0x43c20000] {
        s.line(format!("writel 0x{:x} 0x554", base + 0x1004), "OK");
    }
}
fn replies(s: &mut Script, base: u32) {
    read(s, base + 0x100c, 0x14);
    for _ in 0..63 {
        read(s, base + 0x1000, 0x00908713);
        read(s, base + 0x1000, 0x07000000);
    }
    read(s, base + 0x100c, 5);
    read(s, base + 0x18, 0);
}

// Independent51-bit bit-array checksum oracle. No model codec is called.
fn addressed_reply(address: u8) -> [u8; 7] {
    register_reply(0x1387_9000 | u32::from(address))
}

fn register_reply(word: u32) -> [u8; 7] {
    let bytes = word.to_be_bytes();
    let mut reply = [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0, 0];
    let mut c = [1u8; 5];
    for bit in 0..51 {
        let old = c;
        let input = (reply[bit / 8] >> (7 - bit % 8)) & 1;
        c = [
            old[4] ^ input,
            old[0],
            old[1] ^ old[4] ^ input,
            old[2],
            old[3],
        ];
    }
    reply[6] = c.iter().enumerate().map(|(i, b)| b << i).sum();
    reply
}

fn address_burst(s: &mut Script, base: u32) {
    // No clock advancement between writes. This fills the controller
    // command queue, then the16-byte UART FIFO while it transmits.
    for _ in 0..3 {
        s.line(format!("writel 0x{:x} 0x555", base + 0x1004), "OK");
    }
    for chip in 0..63u32 {
        let word = (chip * 4) << 16 | 0x541;
        s.line(format!("writel 0x{:x} 0x{word:x}", base + 0x1004), "OK");
    }
    read(s, base + 0x100c, 1);
    s.step(50_000_000);
    s.line(format!("writel 0x{:x} 0x554", base + 0x1004), "OK");
    s.step(50_000_000);
    read(s, base + 0x100c, 0x14);
    for chip in 0..63 {
        let reply = addressed_reply(chip * 4);
        read(
            s,
            base + 0x1000,
            u32::from_le_bytes(reply[..4].try_into().unwrap()),
        );
        read(s, base + 0x1000, u32::from(reply[6]) << 24);
    }
    read(s, base + 0x100c, 5);
    read(s, base + 0x18, 0);
}

// Each entry starts an actual bus operation, then reads its completed
// controller register through the serial reply and FPGA RX FIFO.
const TEMPERATURE_OPERATIONS: &[(u32, u32)] = &[
    (0x0198_fe00, 0x0198_fe55),
    (0x0198_ff00, 0x0198_ff21),
    (0x0199_0904, 0x0199_0904),
    (0x0198_0900, 0x0198_0904),
    (0x0198_0000, 0x0198_0059),
    (0x0198_0100, 0x0198_0181),
    (0x0198_1100, 0x0198_1100),
];

// Independent external stimulus. The normal reads below finish before6s.
// Later reads cross both input edges and a full240ms conversion interval.
pub const TEMPERATURE_PLAN: &str = r#"[{"at_ns":6000000000,"inputs":{"local_millicelsius":25000,"remote_millicelsius":105000,"diode":"Connected","supply_valid":true}},{"at_ns":8000000000,"inputs":{"local_millicelsius":25000,"remote_millicelsius":65000,"diode":"Connected","supply_valid":true}}]"#;
const PLANNED_READS: &[(u32, u32)] = &[(0x0198_0100, 0x0198_01a9), (0x0198_0100, 0x0198_0181)];

fn write_chip(s: &mut Script, base: u32, register: u8, value: u32) {
    let header = u32::from(register) << 24 | 0xf4_0948;
    s.line(format!("writel 0x{:x} 0x{header:x}", base + 0x1004), "OK");
    s.line(
        format!("writel 0x{:x} 0x{:x}", base + 0x1004, value.swap_bytes()),
        "OK",
    );
    s.step(1_000_000);
}

fn sensor_reads(s: &mut Script, base: u32, operations: &[(u32, u32)], wait_ns: u64) {
    for &(command, result) in operations {
        if wait_ns != 0 {
            s.step(wait_ns);
        }
        write_chip(s, base, 0x20, command);
        // Advance enough for a full sensor conversion after a range write.
        s.step(500_000_000);
        s.line(format!("writel 0x{:x} 0x20f40544", base + 0x1004), "OK");
        s.step(2_000_000);
        let reply = register_reply(result);
        read(
            s,
            base + 0x1000,
            u32::from_le_bytes(reply[..4].try_into().unwrap()),
        );
        read(s, base + 0x1000, u32::from(reply[6]) << 24);
        read(s, base + 0x100c, 5);
        read(s, base + 0x18, 0);
    }
}

pub fn transactions(
    burst: bool,
    temperature: bool,
    plan: bool,
    fabric: bool,
) -> (String, Vec<String>) {
    let mut s = Script::default();
    if fabric {
        s.line("writel 0xf8000008 0xdf0d", "OK");
        s.line("writel 0xf8000108 0x1e000", "OK"); // I/O PLL30.
        s.line("writel 0xf8000180 0x500", "OK"); // FCLK1 approximately200MHz.
    }
    let bases = [0x43c00000u32, 0x43c10000, 0x43c20000];
    for base in bases {
        read(&mut s, base, 0x00901002);
        s.line(format!("writel 0x{:x} 0x6b", base + 0x10), "OK");
        s.line(format!("writel 0x{:x} 8", base + 8), "OK");
        s.line(format!("writel 0x{:x} 4", base + 0x1008), "OK");
    }
    s.step(20_000); // The enabled controller first clears its FIFOs.
    query_all(&mut s);
    s.step(1_000_000);
    for base in bases {
        read(&mut s, base + 0x100c, 5); // No power: no replies.
    }
    s.step(500_000_000);
    for address in 0x55..=0x57 {
        s.command(address, 6, &[]);
        s.step(100_000_000);
        s.command(address, 0x17, &[]);
        s.read(address, &[3]);
        s.command(address, 0x10, &[64]);
        s.step(100_000_000);
        s.command(address, 0x15, &[1]);
    }
    query_all(&mut s);
    s.step(1_000_000);
    for base in bases {
        read(&mut s, base + 0x100c, 5); // Powered, still held in reset.
    }
    s.line("writel 0x41210000 0xe00", "OK");
    query_all(&mut s);
    s.step(50_000_000);
    for base in bases {
        replies(&mut s, base);
    }
    if fabric {
        // The ASIC oscillator remains25MHz. Halve FCLK1 and adjust only
        // the controller divisor to keep its programmed UART bit period.
        s.line("writel 0xf8000180 0xa00", "OK");
        for base in bases {
            s.line(format!("writel 0x{:x} 0x35", base + 0x10), "OK");
        }
        query_all(&mut s);
        s.step(50_000_000);
        for base in bases {
            replies(&mut s, base);
        }
    }
    if burst {
        address_burst(&mut s, bases[0]);
    }
    if temperature {
        write_chip(&mut s, bases[0], 0x1c, 0x4020_5ae0);
        sensor_reads(&mut s, bases[0], TEMPERATURE_OPERATIONS, 0);
        if plan {
            sensor_reads(&mut s, bases[0], PLANNED_READS, 2_000_000_000);
        }
    }
    // Two independent inhibit paths. Chain8 remains operational.
    s.command(0x55, 0x15, &[0]);
    s.line("writel 0x41210000 0xa00", "OK");
    query_all(&mut s);
    s.step(50_000_000);
    read(&mut s, bases[0] + 0x100c, 5);
    read(&mut s, bases[1] + 0x100c, 5);
    replies(&mut s, bases[2]);
    s.finish()
}

pub fn verify(log: &str, burst: bool, temperature: bool, plan: bool, fabric: bool) -> bool {
    let mut sent: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut commands: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for line in log
        .lines()
        .filter(|s| s.starts_with("md_sim_bm1387_event "))
    {
        let fields: BTreeMap<_, _> = line
            .split_whitespace()
            .filter_map(|s| s.split_once('='))
            .collect();
        let parsed = (|| {
            let link = fields.get("link")?.parse::<u32>().ok()?;
            let kind = fields.get("kind")?.parse::<u32>().ok()?;
            let value = u32::from_str_radix(fields.get("value")?.strip_prefix("0x")?, 16).ok()?;
            let extra = fields.get("extra")?.parse::<u32>().ok()?;
            Some((link, kind, value, extra))
        })();
        let Some((link @ 6..=8, kind, value, extra)) = parsed else {
            return false;
        };
        match kind {
            1 | 3 | 8 | 9 => {}
            2 => {
                let Ok(byte) = u8::try_from(value) else {
                    return false;
                };
                sent.entry(link).or_default().push(byte);
            }
            4 => commands.entry(link).or_default().push((value, extra)),
            _ => return false,
        }
    }
    (6..=8).all(|link| {
        let queries = (if link == 8 { 2 } else { 1 }) + usize::from(fabric);
        let mut expected_commands = vec![(0x54, 63); queries];
        let mut expected_bytes = [0x13, 0x87, 0x90, 0, 0, 0, 7].repeat(63 * queries);
        if burst && link == 6 {
            expected_commands.extend([(0x55, 0); 3]);
            expected_commands.extend([(0x41, 0); 63]);
            expected_commands.push((0x54, 63));
            expected_bytes.extend((0..63).flat_map(|chip| addressed_reply(chip * 4)));
        }
        if temperature && link == 6 {
            expected_commands.push((0x48, 0));
            for &(_, result) in TEMPERATURE_OPERATIONS.iter().chain(if plan {
                PLANNED_READS.iter()
            } else {
                [].iter()
            }) {
                expected_commands.extend([(0x48, 0), (0x44, 1)]);
                expected_bytes.extend(register_reply(result));
            }
        }
        commands.get(&link) == Some(&expected_commands) && sent.get(&link) == Some(&expected_bytes)
    })
}
