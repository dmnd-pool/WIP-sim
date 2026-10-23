//! Independent register expectations from AMD UG585 counter-control and
//! interrupt-status definitions. The selected clock is111111110Hz.
pub fn transactions() -> (String, Vec<String>) {
    let mut commands = Vec::new();
    let mut responses = Vec::new();
    let mut now = 0;
    let mut add = |command: String, value: Option<u64>| {
        let response = if let Some(ns) = command.strip_prefix("clock_step ") {
            now += ns.parse::<u64>().unwrap();
            format!("OK {now}")
        } else if let Some(value) = value {
            format!("OK 0x{value:016x}")
        } else {
            "OK".into()
        };
        commands.push(command);
        responses.push(response);
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
            for (offset, value) in [
                (0, 0),
                (0x30, 11),
                (0x3c, 19),
                (0x48, 73),
                (0x60, 0xe),
                // Reset, waveform disabled, matching disabled, increment.
                (0xc, 0x30),
            ] {
                add(format!("writel {:#x} {value:#x}", address + offset), None);
            }
            add("clock_step 1000".into(), None);
            add(format!("readl {:#x}", address + 0x18), Some(111));
            add(format!("readl {:#x}", address + 0x54), Some(0));
            // Enable matching and reset. All three comparisons must latch.
            add(format!("writel {:#x} 0x38", address + 0xc), None);
            add("clock_step 1000".into(), None);
            add(format!("readl {:#x}", address + 0x18), Some(111));
            add(format!("readl {:#x}", address + 0x54), Some(0xe));
            add(format!("readl {:#x}", address + 0x54), Some(0));
            // Disabling the mode suppresses new events, without clearing a
            // previously latched status. Counter reset does not acknowledge it.
            add(format!("writel {:#x} 0x38", address + 0xc), None);
            add("clock_step 100".into(), None);
            add(format!("writel {:#x} 0x30", address + 0xc), None);
            add("clock_step 1000".into(), None);
            add(format!("readl {:#x}", address + 0x54), Some(2));
            add(format!("readl {:#x}", address + 0x54), Some(0));
            // Event status is retained while the interrupt output is masked.
            add(format!("writel {:#x} 0", address + 0x60), None);
            add(format!("writel {:#x} 0x38", address + 0xc), None);
            add("clock_step 1000".into(), None);
            add(format!("readl {:#x}", address + 0x54), Some(0xe));
            // Stop counting and exercise documented bit6 in both controls.
            // The external clock and waveform outputs remain unconnected.
            add(format!("writel {:#x} 0x61", address + 0xc), None);
            add(format!("readl {:#x}", address + 0xc), Some(0x61));
            add(format!("writel {address:#x} 0x60"), None);
            add(format!("readl {address:#x}"), Some(0x60));
            add("clock_step 1000".into(), None);
            add(format!("readl {:#x}", address + 0x18), Some(111));
        }
    }
    (commands.join("\n") + "\n", responses)
}
