//! Down-counter expectations from the vendor TTC reset contract. Counts
//! change only on completed input ticks, not on fractional time advances.
pub fn transactions() -> (String, Vec<String>) {
    let mut commands = Vec::new();
    let mut responses = Vec::new();
    let mut now = 0;
    let mut add = |command: String, value: Option<u64>| {
        responses.push(if let Some(ns) = command.strip_prefix("clock_step ") {
            now += ns.parse::<u64>().unwrap();
            format!("OK {now}")
        } else if let Some(value) = value {
            format!("OK 0x{value:016x}")
        } else {
            "OK".into()
        });
        commands.push(command);
    };
    for (address, value) in [
        (0xf8000008u32, 0xdf0d),
        (0xf8000100, 0x28000),
        (0xf8000120, 0x1f000200),
        (0xf80001c4, 1),
    ] {
        add(format!("writel {address:#x} {value:#x}"), None);
    }
    for base in [0xf8001000u32, 0xf8002000] {
        for channel in 0..3 {
            let address = base + 4 * channel;
            add(format!("writel {address:#x} 0"), None);
            // Overflow down-counting resets to65535. Its111111110Hz clock
            // completes no tick at9ns, one at10ns and111 at1000ns.
            add(format!("writel {:#x} 0x34", address + 0xc), None);
            add(format!("readl {:#x}", address + 0x18), Some(65535));
            for (ns, value) in [(8, 65535), (1, 65535), (1, 65534), (990, 65424)] {
                add(format!("clock_step {ns}"), None);
                add(format!("readl {:#x}", address + 0x18), Some(value));
            }
            add(format!("readl {:#x}", address + 0x54), Some(0));
            // Direction changes preserve the counter and elapsed tick phase.
            for (control, before, after) in [(0x20, 65424, 65425), (0x24, 65425, 65424)] {
                add(format!("writel {:#x} {control:#x}", address + 0xc), None);
                add(format!("readl {:#x}", address + 0x18), Some(before));
                add("clock_step 7".into(), None);
                add(format!("readl {:#x}", address + 0x18), Some(before));
                add("clock_step 2".into(), None);
                add(format!("readl {:#x}", address + 0x18), Some(after));
            }
            // Interval down-counting resets to37. Matching30,20,10 raises
            // distinct status bits; the38th tick reloads37 and latches bit0.
            for (offset, value) in [(0x24, 37), (0x30, 30), (0x3c, 20), (0x48, 10)] {
                add(format!("writel {:#x} {value}", address + offset), None);
            }
            add(format!("writel {:#x} 0x3e", address + 0xc), None);
            add(format!("readl {:#x}", address + 0x18), Some(37));
            for (ns, value, status) in [(110, 25, 2), (100, 14, 4), (100, 3, 8), (40, 37, 1)] {
                add(format!("clock_step {ns}"), None);
                add(format!("readl {:#x}", address + 0x18), Some(value));
                add(format!("readl {:#x}", address + 0x54), Some(status));
                add(format!("readl {:#x}", address + 0x54), Some(0));
            }
            add(format!("writel {:#x} 0x2f", address + 0xc), None);
            add("clock_step 1000".into(), None);
            add(format!("readl {:#x}", address + 0x18), Some(37));
            // Reset while stopped retains the disable bit and interval top.
            add(format!("writel {:#x} 0x3f", address + 0xc), None);
            add(format!("readl {:#x}", address + 0xc), Some(0x2f));
            add("clock_step 1000".into(), None);
            add(format!("readl {:#x}", address + 0x18), Some(37));
        }
    }
    (commands.join("\n") + "\n", responses)
}
