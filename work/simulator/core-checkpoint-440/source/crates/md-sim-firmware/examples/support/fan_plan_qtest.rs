//! Analytic one-second motion windows through the compiled timed-input ABI.
pub const INPUT: &str = r#"[{"at_ns":2000000000,"motor":1,"powered":null,"stalled":true},{"at_ns":4000000000,"motor":1,"powered":null,"stalled":false},{"at_ns":6000000000,"motor":1,"powered":false,"stalled":null},{"at_ns":8000000000,"motor":1,"powered":true,"stalled":null}]"#;
pub const SCRIPT: &str = concat!(
    "writel 0x42800014 100\n",
    "clock_step 1999999999\nreadl 0x42800004\n",
    "clock_step 1\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
    "clock_step 1000000000\nreadl 0x42800004\n",
);
// 6000RPM=100RPS. A half-second acceleration contributes25 turns; the next
// half-second contributes50. The half-second coast contributes25 turns.
pub const EXPECTED: &[&str] = &[
    "OK",
    "OK 1999999999",
    "OK 0x000000000000004b",
    "OK 2000000000",
    "OK 0x0000000000000064",
    "OK 3000000000",
    "OK 0x0000000000000000",
    "OK 4000000000",
    "OK 0x0000000000000000",
    "OK 5000000000",
    "OK 0x000000000000004b",
    "OK 6000000000",
    "OK 0x0000000000000064",
    "OK 7000000000",
    "OK 0x0000000000000019",
    "OK 8000000000",
    "OK 0x0000000000000000",
    "OK 9000000000",
    "OK 0x000000000000004b",
    "OK 10000000000",
    "OK 0x0000000000000064",
];
pub fn verify(log: &str) -> bool {
    log.lines()
        .filter(|l| l.starts_with("md_sim_fan_input "))
        .eq([
            "md_sim_fan_input ns=2000000000 motor=1 mask=2 values=2",
            "md_sim_fan_input ns=4000000000 motor=1 mask=2 values=0",
            "md_sim_fan_input ns=6000000000 motor=1 mask=1 values=0",
            "md_sim_fan_input ns=8000000000 motor=1 mask=1 values=1",
        ])
}
