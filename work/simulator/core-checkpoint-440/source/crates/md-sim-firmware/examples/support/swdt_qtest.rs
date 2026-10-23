//! Literal SWDT expectations from UG585 and the archived kernel sequence.
//! No model code is imported. IRQ lines are part of the ordered response.
pub fn transactions() -> (String, Vec<String>) {
    let mut commands = Vec::new();
    let mut responses = Vec::new();
    let mut now = 0u64;
    let mut add = |command: String, value: Option<u64>, irq: Option<bool>| {
        if let Some(high) = irq {
            responses.push(format!("IRQ {} 9", if high { "raise" } else { "lower" }));
        }
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
    add("irq_intercept_in gic".into(), None, None);
    for (address, value) in [
        (0xf8000008u32, 0xdf0d),
        (0xf8000100, 0x28000),
        (0xf8000120, 0x1f000200),
        (0xf80001c4, 1),
    ] {
        add(format!("writel {address:#x} {value:#x}"), None, None);
    }
    for (offset, value) in [(0, 0x1c0), (4, 0x3ffc), (8, 0), (12, 0)] {
        add(
            format!("readl {:#x}", 0xf8005000u32 + offset),
            Some(value),
            None,
        );
    }
    // Wrong keys cannot change the mode, count control or status.
    for (offset, value, expected) in [(0, 0xabd1ff, 0x1c0), (4, 0x91ffff, 0x3ffc), (8, 0x1998, 0)] {
        add(
            format!("writel {:#x} {value:#x}", 0xf8005000u32 + offset),
            None,
            None,
        );
        add(
            format!("readl {:#x}", 0xf8005000u32 + offset),
            Some(expected),
            None,
        );
    }
    // At111111110Hz:4095 decrements at /8 take294840.002948ns.
    // Samples bracket the sub-nanosecond PTIMER rounding uncertainty.
    for (mode, pulse_ns) in [(0x45, 36), (0xc5, 72), (0x145, 144), (0x1c5, 288)] {
        add("writel 0xf8005000 0xabc040".into(), None, None);
        add("writel 0xf8005004 0x920000".into(), None, None);
        add(
            format!("writel 0xf8005000 {:#x}", 0xabc000 | mode),
            None,
            None,
        );
        add("writel 0xf8005008 0x1999".into(), None, None);
        add("readl 0xf8005008".into(), Some(0), None);
        add("clock_step 294839".into(), None, None);
        add("readl 0xf800500c".into(), Some(0), None);
        add("clock_step 2".into(), None, Some(true));
        add("readl 0xf800500c".into(), Some(1), None);
        add(format!("clock_step {}", pulse_ns - 3), None, None);
        add("clock_step 4".into(), None, Some(false));
        add("readl 0xf800500c".into(), Some(1), None);
        add("writel 0xf800500c 0".into(), None, None);
        add("readl 0xf800500c".into(), Some(1), None);
        add("clock_step 600000".into(), None, None);
        add("readl 0xf800500c".into(), Some(1), None);
    }
    // WDEN gates outputs, not the counter. All four divisors expire.
    for (select, before) in [(0, 294839), (1, 2358719), (2, 18869759), (3, 150958080)] {
        add("writel 0xf8005000 0xabc044".into(), None, None);
        add(
            format!("writel 0xf8005004 {:#x}", 0x920000 | select),
            None,
            None,
        );
        add("writel 0xf8005008 0x1999".into(), None, None);
        add(format!("clock_step {before}"), None, None);
        add("readl 0xf800500c".into(), Some(0), None);
        add("clock_step 3".into(), None, None);
        add("readl 0xf800500c".into(), Some(1), None);
    }
    // A valid restart clears zero and resets the prescaler. An invalid key
    // cannot extend the deadline. Disabling IRQ during counting suppresses it.
    add("writel 0xf8005004 0x920000".into(), None, None);
    add("writel 0xf8005000 0xabc045".into(), None, None);
    add("writel 0xf8005008 0x1999".into(), None, None);
    add("clock_step 200000".into(), None, None);
    add("writel 0xf8005008 0x1999".into(), None, None);
    add("clock_step 200000".into(), None, None);
    add("readl 0xf800500c".into(), Some(0), None);
    add("writel 0xf8005008 0x1998".into(), None, None);
    add("writel 0xf8005000 0xabc041".into(), None, None);
    add("clock_step 94842".into(), None, None);
    add("readl 0xf800500c".into(), Some(1), None);
    // Gate CPU_1x and select an absent external input in separate runs.
    // With outputs disabled, elapsed wall time cannot become clock edges.
    // The remaining counter resumes after the selected clock returns.
    for (address, off, on) in [(0xf8000120u32, 0x17000200, 0x1f000200), (0xf8000304, 1, 0)] {
        add("writel 0xf8005000 0xabc040".into(), None, None);
        add("writel 0xf8005008 0x1999".into(), None, None);
        add("clock_step 100000".into(), None, None);
        add(format!("writel {address:#x} {off:#x}"), None, None);
        add("clock_step 1000000".into(), None, None);
        add(format!("writel {address:#x} {on:#x}"), None, None);
        add("clock_step 100000".into(), None, None);
        add("readl 0xf800500c".into(), Some(0), None);
        add("clock_step 100000".into(), None, None);
        add("readl 0xf800500c".into(), Some(1), None);
    }
    (commands.join("\n") + "\n", responses)
}
