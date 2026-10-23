//! Independent clock observations. No simulator model is called here.
//! Periods use the QEMU clock ABI (32 fractional bits), not rounded Hz.
use serde::Serialize;

#[derive(Serialize)]
pub struct Case {
    pub script: String,
    pub responses: Vec<String>,
    pub periods: Vec<[u64; 4]>,
    pub reset_levels: Vec<[bool; 4]>,
}

pub fn transactions() -> Case {
    let mut case = Case {
        script: String::new(),
        responses: vec![],
        periods: vec![],
        reset_levels: vec![],
    };
    let ps = (1_000_000_000u64 << 32) / 33_333_333;
    let mut period = [ps / 26 * 24; 4];
    fn write(case: &mut Case, address: u32, value: u32) {
        case.script += &format!("writel {address:#x} {value:#x}\n");
        case.responses.push("OK".into());
        case.script += &format!("readl {address:#x}\n");
        case.responses.push(format!("OK 0x{value:016x}"));
    }
    case.script += "writel 0xf8000008 0xdf0d\n";
    case.responses.push("OK".into());
    // Equal I/O and ARM PLL rates initially make the independent per-output
    // divider checks simple. DDR uses a different rate for source selection.
    for (address, value, clocks) in [
        (0xf8000108, 30 << 12, [ps / 30 * 24; 4]),
        (0xf8000100, 30 << 12, [ps / 30 * 24; 4]),
        (0xf8000104, 20 << 12, [ps / 30 * 24; 4]),
    ] {
        write(&mut case, address, value);
        case.periods.push(clocks);
    }
    for channel in 0..4usize {
        let address = 0xf8000170 + 16 * channel as u32;
        for (source, parent) in [(0, ps / 30), (1, ps / 30), (2, ps / 30), (3, ps / 20)] {
            for (d0, d1, factor) in [
                (5, 4, 20),
                (2, 3, 6),
                (63, 63, 3969),
                (0, 0, 1),
                (0, 2, 2),
                (1, 0, 1),
            ] {
                write(&mut case, address, source << 4 | d0 << 8 | d1 << 20);
                period[channel] = parent * factor;
                // Before its first configuration, a channel retains the I/O
                // PLL30 /24 reset divider. Other channels retain their settings.
                for p in period.iter_mut().skip(channel + 1) {
                    *p = ps / 30 * 24;
                }
                case.periods.push(period);
            }
        }
        write(&mut case, address + 8, 1);
        let mut stopped = period;
        stopped[channel] = 0;
        case.periods.push(stopped);
        write(&mut case, address + 8, 0);
        case.periods.push(period);
    }
    // Each channel now selects DDR /1. PLL reset/power-down and bypass must
    // reach every output, including changes without a fabric-register write.
    for (ctrl, expected) in [
        (20 << 12 | 1, 0),
        (20 << 12 | 2, 0),
        (20 << 12 | 0x10, ps),
        (20 << 12, ps / 20),
    ] {
        write(&mut case, 0xf8000104, ctrl);
        case.periods.push([expected; 4]);
    }
    for value in [0, 1, 2, 4, 8, 15, 0x01f33f0f] {
        write(&mut case, 0xf8000240, value);
        case.reset_levels
            .push(std::array::from_fn(|n| value & (1 << n) == 0));
    }
    // Locked writes produce neither clock changes nor reset changes.
    case.script += "writel 0xf8000004 0x767b\nwritel 0xf8000170 0\nwritel 0xf8000240 0\nreadl 0xf8000170\nreadl 0xf8000240\n";
    case.responses.extend([
        "OK".into(),
        "OK".into(),
        "OK".into(),
        "OK 0x0000000000000130".into(),
        "OK 0x0000000001f33f0f".into(),
    ]);
    case
}

impl Case {
    pub fn verify(&self, log: &str) -> bool {
        fn groups<T: Copy + Default>(items: Vec<(usize, T)>) -> Option<Vec<[T; 4]>> {
            if items.len() % 4 != 0 {
                return None;
            }
            items
                .chunks_exact(4)
                .map(|chunk| {
                    let mut group = [T::default(); 4];
                    for (n, (channel, value)) in chunk.iter().enumerate() {
                        if n != *channel {
                            return None;
                        }
                        group[n] = *value;
                    }
                    Some(group)
                })
                .collect()
        }
        let mut periods = vec![];
        let mut resets = vec![];
        for line in log.lines() {
            let parsed =
                |prefix: &str| line.split_whitespace().find_map(|v| v.strip_prefix(prefix));
            if line.starts_with("md_sim_zynq_fclk ") {
                let Some((channel, value)) = parsed("channel=")
                    .and_then(|v| v.parse().ok())
                    .zip(parsed("period=").and_then(|v| v.parse().ok()))
                else {
                    return false;
                };
                periods.push((channel, value));
            } else if line.starts_with("md_sim_zynq_freset ") {
                let Some((channel, value)) = parsed("channel=")
                    .and_then(|v| v.parse().ok())
                    .zip(parsed("high=").and_then(|v| v.parse::<u8>().ok()))
                else {
                    return false;
                };
                if value > 1 {
                    return false;
                }
                resets.push((channel, value == 1));
            }
        }
        // Initial clock/reset propagation precedes the finite test stream.
        // Compare the complete suffix, so a locked write that changes an
        // output adds a record and fails this assertion.
        groups(periods).is_some_and(|p| p.ends_with(&self.periods))
            && groups(resets).is_some_and(|r| r.ends_with(&self.reset_levels))
    }
}
