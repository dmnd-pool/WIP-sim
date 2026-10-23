//! Real S9IO transmitter across a mid-byte FCLK1 gate and frequency change.
//! Pin events must stop during the gate. The ASIC packet is a literal archived
//! read-all command; no ASIC response or mining claim is made.
use serde::Serialize;
#[derive(Serialize)]
pub struct Case {
    pub script: String,
    pub responses: Vec<String>,
    pub windows: Vec<(u32, u64, u64, u64)>,
}

pub fn transactions() -> Case {
    let mut c = Case {
        script: String::new(),
        responses: vec![],
        windows: vec![],
    };
    let mut now = 0;
    fn write(c: &mut Case, address: u64, value: u32) {
        c.script += &format!("writel {address:#x} {value:#x}\n");
        c.responses.push("OK".into());
    }
    fn step(c: &mut Case, now: &mut u64, delta: u64) {
        *now += delta;
        c.script += &format!("clock_step {delta}\n");
        c.responses.push(format!("OK {now}"));
    }
    write(&mut c, 0xf8000008, 0xdf0d);
    write(&mut c, 0xf8000108, 0x10); // Bypass: exact PS reference period.
    for link in 5..=7 {
        let epoch = now;
        write(&mut c, 0xf8000180, 0x00100300); // PS /3, ~90ns.
        let base = 0x43c50000 + (link - 5) * 0x10000;
        write(&mut c, base + 0x10, 0);
        write(&mut c, base + 0x14, 257);
        write(&mut c, base + 8, 8);
        step(&mut c, &mut now, 10_000);
        write(&mut c, base + 0x1004, 0x00000554);
        step(&mut c, &mut now, 20_000);
        let stopped = now;
        write(&mut c, 0xf8000188, 1);
        step(&mut c, &mut now, 1_000_000);
        write(&mut c, 0xf8000180, 0x00200300); // Resume at PS /6, ~180ns.
        write(&mut c, 0xf8000188, 0);
        c.windows.push((link as u32, epoch, stopped, now));
        // Gating FCLK0 must not stop the FCLK1 consumer.
        write(&mut c, 0xf8000178, 1);
        step(&mut c, &mut now, 200_000);
        write(&mut c, base + 8, 0);
    }
    c
}
impl Case {
    pub fn verify(&self, log: &str) -> bool {
        let mut events = vec![];
        for line in log.lines().filter(|l| l.starts_with("md_sim_s9io_event ")) {
            let field = |p| line.split_whitespace().find_map(|v| v.strip_prefix(p));
            let parse = || {
                Some((
                    field("link=")?.parse::<u32>().ok()?,
                    field("virtual_ns=")?.parse::<u64>().ok()?,
                    u32::from_str_radix(field("flags=0x")?, 16).ok()?,
                    u8::from_str_radix(field("tx=0x")?, 16).ok()?,
                ))
            };
            let Some(e) = parse() else {
                return false;
            };
            events.push(e);
        }
        let period = u128::from((1_000_000_000u64 << 32) / 33_333_333) * 3;
        for &(link, epoch, stopped, resume) in &self.windows {
            if events
                .iter()
                .any(|e| e.0 == link && e.1 > stopped && e.1 < resume)
            {
                return false;
            }
            let bytes: Vec<_> = events
                .iter()
                .filter(|e| e.0 == link && e.2 & 1 != 0)
                .collect();
            if bytes.iter().map(|e| e.3).collect::<Vec<_>>() != [84, 5, 0, 0, 25] {
                return false;
            }
            if bytes[0].1 >= stopped || bytes[0].1 < epoch || bytes[1].1 <= resume {
                return false;
            }
            let first_edge = (u128::from(bytes[0].1 - epoch) << 32) / period;
            let stopped_edge = (u128::from(stopped - epoch) << 32) / period;
            let Some(remaining) = (first_edge + 161).checked_sub(stopped_edge) else {
                return false;
            };
            for (n, event) in bytes.iter().skip(1).enumerate() {
                let exact = (remaining + n as u128 * 161) * period * 2;
                let expected = resume + ((exact + u128::from(u32::MAX)) >> 32) as u64;
                if event.1 != expected {
                    return false;
                }
            }
        }
        true
    }
}
