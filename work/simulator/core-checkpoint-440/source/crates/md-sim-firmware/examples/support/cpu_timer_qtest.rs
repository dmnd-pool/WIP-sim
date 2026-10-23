//! SLCR/kernel clock contract. PS33333333Hz * PLL40 / ARMdiv2 /2
//! is333333330Hz. These counts are arithmetic oracles, not model output.
pub fn transactions(clock_tree: bool) -> (String, Vec<String>) {
    let mut commands = Vec::new();
    let mut expected = Vec::new();
    let mut now = 0u64;
    for (command, readback) in [
        ("writel 0xf8000008 0xdf0d", None),
        ("writel 0xf8000100 0x28000", None),
        ("writel 0xf8000120 0x1f000200", None),
        ("writel 0xf8f00208 0", None),
        ("writel 0xf8f00200 0", None),
        ("writel 0xf8f00204 0", None),
        ("writel 0xf8f00208 1", None),
        ("clock_step 1000000000", None),
        ("readl 0xf8f00200", Some(333_333_330u64)),
        ("writel 0xf8f00208 0", None),
        ("clock_step 1000000000", None),
        ("readl 0xf8f00200", Some(333_333_330)),
        ("writel 0xf8f00200 0", None),
        ("writel 0xf8f00204 0", None),
        ("writel 0xf8f00208 0x101", None),
        ("clock_step 1000000000", None),
        ("readl 0xf8f00200", Some(166_666_665)),
        // Gate the peripheral clock while the timer remains enabled.
        ("writel 0xf8000120 0x1d000200", None),
        ("clock_step 1000000000", None),
        ("readl 0xf8f00200", Some(166_666_665)),
        // Restore at half the source rate, keeping the timer prescaler2.
        ("writel 0xf8000120 0x1f000400", None),
        ("clock_step 1000000000", None),
        ("readl 0xf8f00200", Some(249_999_997)),
        ("writel 0xf8f00208 0", None),
        ("writel 0xf8000120 0x1f000200", None),
        // Private one-shot timer: half-time count, then expiry. The final
        // ten ns include the terminal tick and QEMU's ns quantization.
        ("writel 0xf8f00608 0", None),
        ("writel 0xf8f0060c 1", None),
        ("writel 0xf8f00600 333333330", None),
        ("writel 0xf8f00608 5", None),
        ("clock_step 500000000", None),
        ("readl 0xf8f00604", Some(166_666_665)),
        ("readl 0xf8f0060c", Some(0)),
        ("clock_step 500000010", None),
        ("readl 0xf8f00604", Some(0)),
        ("readl 0xf8f0060c", Some(1)),
        // Program and arm a private timer while its input clock is gated.
        ("writel 0xf8000120 0x1d000200", None),
        ("writel 0xf8f00608 0", None),
        ("writel 0xf8f0060c 1", None),
        ("writel 0xf8f00600 333333330", None),
        ("writel 0xf8f00608 5", None),
        ("clock_step 1000000000", None),
        ("readl 0xf8f00604", Some(333_333_330)),
        ("readl 0xf8f0060c", Some(0)),
        // Half the source rate, then a rate change while counting.
        ("writel 0xf8000120 0x1f000400", None),
        ("clock_step 1000000000", None),
        ("readl 0xf8f00604", Some(166_666_665)),
        ("writel 0xf8000120 0x1f000200", None),
        ("clock_step 500000010", None),
        ("readl 0xf8f00604", Some(0)),
        ("readl 0xf8f0060c", Some(1)),
        // A prescaled one-shot must stay expired across a clock change.
        ("writel 0xf8f00608 0", None),
        ("writel 0xf8f0060c 1", None),
        ("writel 0xf8f00600 1", None),
        ("writel 0xf8f00608 0x105", None),
        ("clock_step 20", None),
        ("readl 0xf8f0060c", Some(1)),
        ("writel 0xf8f0060c 1", None),
        ("writel 0xf8000120 0x1d000200", None),
        ("clock_step 1000", None),
        ("writel 0xf8000120 0x1f000200", None),
        ("clock_step 20", None),
        ("readl 0xf8f00604", Some(0)),
        ("readl 0xf8f0060c", Some(0)),
        // Global comparator uses >=. Three ticks take just over9ns.
        // Non-power-of-two auto-increment and intervening reads must retain
        // the original phase: thresholds3,6 occur at10ns and19ns.
        ("writel 0xf8f00208 0", None),
        ("writel 0xf8f00200 0", None),
        ("writel 0xf8f00204 0", None),
        ("writel 0xf8f00210 3", None),
        ("writel 0xf8f00214 0", None),
        ("writel 0xf8f00218 3", None),
        ("writel 0xf8f0020c 1", None),
        ("writel 0xf8f00208 0xf", None),
        ("clock_step 8", None),
        ("readl 0xf8f00200", Some(2)),
        ("readl 0xf8f0020c", Some(0)),
        ("clock_step 1", None),
        ("readl 0xf8f00200", Some(2)),
        ("readl 0xf8f0020c", Some(0)),
        ("clock_step 1", None),
        ("readl 0xf8f00200", Some(3)),
        ("readl 0xf8f0020c", Some(1)),
        ("readl 0xf8f00210", Some(6)),
        ("writel 0xf8f0020c 1", None),
        ("readl 0xf8f0020c", Some(0)),
        ("clock_step 8", None),
        ("readl 0xf8f00200", Some(5)),
        ("readl 0xf8f0020c", Some(0)),
        ("clock_step 1", None),
        ("readl 0xf8f00200", Some(6)),
        ("readl 0xf8f0020c", Some(1)),
        ("readl 0xf8f00210", Some(9)),
        ("writel 0xf8f00208 0", None),
        // The watchdog's counter shares the private peripheral clock.
        ("writel 0xf8f00628 0", None),
        ("writel 0xf8f0062c 1", None),
        ("writel 0xf8f00620 333333330", None),
        ("writel 0xf8f00628 5", None),
        ("clock_step 500000000", None),
        ("readl 0xf8f00624", Some(166_666_665)),
        ("clock_step 500000010", None),
        ("readl 0xf8f00624", Some(0)),
        ("readl 0xf8f0062c", Some(1)),
        // Upstream TTC uses133MHz and fractional16-bit counts. A two-tick
        // interval lasts15.0376ns. It must not repeatedly schedule the
        // callback at15ns before the interval actually ends.
        ("writel 0xf8001000 0", None),
        ("writel 0xf8001030 65535", None),
        ("writel 0xf800103c 65535", None),
        ("writel 0xf8001048 65535", None),
        ("writel 0xf8001024 1", None),
        ("writel 0xf800100c 0x12", None),
        (
            if clock_tree {
                "clock_step 18"
            } else {
                "clock_step 15"
            },
            None,
        ),
        ("readl 0xf8001054", Some(0)),
        ("clock_step 1", None),
        ("readl 0xf8001054", Some(1)),
        ("readl 0xf8001018", Some(0)),
        ("writel 0xf800100c 1", None),
    ] {
        commands.push(command.to_string());
        expected.push(if let Some(ns) = command.strip_prefix("clock_step ") {
            now += ns.parse::<u64>().unwrap();
            format!("OK {now}")
        } else if let Some(value) = readback {
            format!("OK 0x{value:016x}")
        } else {
            "OK".into()
        });
    }
    if clock_tree {
        // The archived DTB selects clkc6 (cpu_1x) for both TTC blocks.
        // PS33333333 * PLL40 / ARMdiv2 /6 =111111110Hz. Each
        // expected count below is the floor of elapsed ticks, with the
        // fractional counter retained across each rate or gate change.
        let mut add = |command: String, readback: Option<u64>| {
            expected.push(if let Some(ns) = command.strip_prefix("clock_step ") {
                now += ns.parse::<u64>().unwrap();
                format!("OK {now}")
            } else if let Some(value) = readback {
                format!("OK 0x{value:016x}")
            } else {
                "OK".into()
            });
            commands.push(command);
        };
        for base in [0xf8001000_u32, 0xf8002000] {
            for channel in 0..3 {
                let clock = base + channel * 4;
                let control = clock + 0xc;
                let count = clock + 0x18;
                for (command, expected_count) in [
                    ("writel 0xf8000120 0x1f000200".into(), None),
                    ("writel 0xf80001c4 1".into(), None),
                    (format!("writel {clock:#x} 0"), None),
                    (format!("writel {control:#x} 0x10"), None),
                    ("clock_step 1000".into(), None),
                    (format!("readl {count:#x}"), Some(111)),
                    ("clock_step 900".into(), None),
                    (format!("readl {count:#x}"), Some(211)),
                    ("writel 0xf8000120 0x17000200".into(), None),
                    ("clock_step 10000".into(), None),
                    (format!("readl {count:#x}"), Some(211)),
                    ("clock_step 10000".into(), None),
                    (format!("readl {count:#x}"), Some(211)),
                    ("writel 0xf8000120 0x1f000400".into(), None),
                    ("clock_step 1800".into(), None),
                    (format!("readl {count:#x}"), Some(311)),
                    ("writel 0xf80001c4 0".into(), None),
                    ("clock_step 1200".into(), None),
                    (format!("readl {count:#x}"), Some(411)),
                    (format!("writel {clock:#x} 1"), None),
                    ("clock_step 2400".into(), None),
                    (format!("readl {count:#x}"), Some(511)),
                    (format!("writel {control:#x} 1"), None),
                    ("clock_step 10000".into(), None),
                    (format!("readl {count:#x}"), Some(511)),
                    ("clock_step 10000".into(), None),
                    (format!("readl {count:#x}"), Some(511)),
                    (format!("writel {clock:#x} 0x20"), None),
                    (format!("writel {control:#x} 0"), None),
                    ("clock_step 10000".into(), None),
                    (format!("readl {count:#x}"), Some(511)),
                    (format!("writel {clock:#x} 0"), None),
                    (format!("writel {control:#x} 0x10"), None),
                    ("clock_step 2400".into(), None),
                    (format!("readl {count:#x}"), Some(199)),
                    (format!("writel {control:#x} 1"), None),
                ] {
                    add(command, expected_count);
                }
            }
        }
    }
    (commands.join("\n") + "\n", expected)
}
