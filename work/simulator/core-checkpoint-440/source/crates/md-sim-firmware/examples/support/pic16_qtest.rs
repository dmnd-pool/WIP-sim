//! Literal firmware-driver transactions through AXI IIC. The oracle does
//! not call the PIC model. Values come from the pinned application and
//! explicit commands below.
#[derive(Default)]
pub(super) struct Script {
    commands: Vec<String>,
    expected: Vec<String>,
    now: u64,
}
impl Script {
    pub(super) fn line(&mut self, command: impl Into<String>, response: impl Into<String>) {
        self.commands.push(command.into());
        self.expected.push(response.into());
    }
    pub(super) fn step(&mut self, ns: u64) {
        self.now += ns;
        self.line(format!("clock_step {ns}"), format!("OK {}", self.now));
    }
    fn init(&mut self) {
        self.line("writel 0x41600040 0xa", "OK");
        self.line("writeb 0x41600120 15", "OK");
        self.line("writeb 0x41600100 1", "OK");
    }
    fn send(&mut self, address: u8, byte: u8) {
        self.init();
        self.line(
            format!("writew 0x41600108 {}", 0x100 | (u16::from(address) << 1)),
            "OK",
        );
        self.line(
            format!("writew 0x41600108 {}", 0x200 | u16::from(byte)),
            "OK",
        );
        self.step(180_000); // Nine100kHz clocks each for address and byte.
    }
    pub(super) fn command(&mut self, address: u8, command: u8, arguments: &[u8]) {
        for byte in [0x55, 0xaa, command].iter().chain(arguments) {
            self.send(address, *byte);
        }
    }
    pub(super) fn read(&mut self, address: u8, bytes: &[u8]) {
        self.init();
        self.line(
            format!("writew 0x41600108 {}", 0x101 | (u16::from(address) << 1)),
            "OK",
        );
        self.line(format!("writew 0x41600108 {}", 0x200 | bytes.len()), "OK");
        self.step(90_000 * (1 + bytes.len() as u64));
        for byte in bytes {
            self.line("readb 0x4160010c", format!("OK 0x{byte:016x}"));
        }
    }
    pub(super) fn finish(self) -> (String, Vec<String>) {
        (self.commands.join("\n") + "\n", self.expected)
    }
}

pub struct Transactions {
    pub script: String,
    pub responses: Vec<String>,
    pub power: Vec<(u64, u32)>,
}

impl Transactions {
    pub fn verify_power(&self, log: &str) -> bool {
        let mut actual = Vec::new();
        for line in log
            .lines()
            .filter(|line| line.starts_with("md_sim_pic_power "))
        {
            let field = |prefix| line.split_whitespace().find_map(|v| v.strip_prefix(prefix));
            if field("address=") != Some("0x57") {
                return false;
            }
            let ns = field("ns=").and_then(|s| s.parse().ok());
            let state = field("state=")
                .and_then(|s| s.strip_prefix("0x"))
                .and_then(|s| u32::from_str_radix(s, 16).ok());
            let (Some(ns), Some(state)) = (ns, state) else {
                return false;
            };
            actual.push((ns, state));
        }
        actual == self.power
    }
}

pub fn transactions() -> Transactions {
    let mut s = Script::default();
    let mut power = Vec::new();
    s.line("irq_intercept_out /machine/pic57 power-enable", "OK");
    // External machine reset causes a500ms busy interval. An early address
    // must NACK; the controller status exposes its real transmit-error bit.
    s.init();
    s.line("writew 0x41600108 0x1aa", "OK");
    s.step(90_000);
    s.line("readl 0x41600020", "OK 0x00000000000000d6");
    s.step(500_000_000);
    // Prefix parser survives a STOP after every byte. Read the first pinned
    // app word3183, including undefined extra-byte latch behavior.
    s.command(0x55, 1, &[3, 0]);
    s.command(0x55, 3, &[]);
    s.read(0x55, &[0x31, 0x31]);
    s.read(0x55, &[0x83]);
    s.command(0x55, 8, &[]);
    s.read(0x55, &[3]);
    s.read(0x55, &[8]);
    // Separate slave state: another controller still has pointer zero, then
    // accepts a distinct nonzero pointer. A missing device returning zeros
    // from an empty receive FIFO cannot satisfy the second check.
    s.command(0x56, 8, &[]);
    s.read(0x56, &[0]);
    s.read(0x56, &[0]);
    s.command(0x56, 1, &[0x0f, 0x80]);
    s.command(0x56, 8, &[]);
    s.read(0x56, &[0x0f]);
    s.read(0x56, &[0x80]);
    s.command(0x55, 8, &[]);
    s.read(0x55, &[3]);
    s.read(0x55, &[8]);
    // Runtime contract selected only after verifying current app storage.
    s.command(0x57, 6, &[]);
    s.step(100_000_000);
    // The guest's clear/enable sequence can straddle STOP. PG090 BNB
    // remains asserted while idle, so this ACKed55 must not be retried.
    s.init();
    s.line("writew 0x41600108 0x1ae", "OK");
    s.line("writew 0x41600108 0x255", "OK");
    s.line("readl 0x41600020", "OK 0x00000000000000d0");
    s.step(180_000);
    s.line("writel 0x41600020 0x10", "OK");
    s.line("writel 0x41600028 0x10", "OK");
    s.line("readl 0x41600020", "OK 0x00000000000000d4");
    s.send(0x57, 0xaa);
    s.send(0x57, 0x17);
    s.read(0x57, &[3]);
    s.command(0x57, 0x10, &[64]);
    power.push((s.now, 0x4002));
    s.step(100_000_000);
    // The archived application tests zero, not the range0..1.
    s.command(0x57, 0x15, &[0x55]);
    power.push((s.now, 0x4003));
    s.command(0x57, 0x18, &[]);
    s.read(0x57, &[64]);
    s.command(0x57, 0x16, &[]);
    s.command(0x57, 0x15, &[0]);
    power.push((s.now, 0x4002));
    // Power-off gates the exact app's software heartbeat counters. The
    // application still serves version commands after more than10seconds.
    s.step(20_000_000_000);
    s.command(0x57, 0x17, &[]);
    s.read(0x57, &[3]);
    s.command(0x57, 0x16, &[]);
    s.command(0x57, 0x15, &[1]);
    power.push((s.now, 0x4003));
    let cutoff_ns = s.now + 10_000_000_000;
    s.step(9_000_000_000);
    s.command(0x57, 0x17, &[]);
    s.read(0x57, &[3]);
    // A scheduled expiry must cut the actual power pin with no I2C access.
    s.step(2_000_000_000);
    power.push((cutoff_ns, 0x4002));
    s.command(0x57, 0x17, &[]);
    s.read(0x57, &[3]);
    s.command(0x57, 0x16, &[]);
    // Heartbeat alone must not raise the power output. Explicit ENABLE does.
    s.command(0x57, 0x15, &[1]);
    power.push((s.now, 0x4003));
    s.command(0x57, 0x15, &[0]);
    power.push((s.now, 0x4002));
    // Interrupted native restart: an old55 AA prefix precedes a new flush.
    // The exact app consumes command55 and one byte; EE goes to UART.
    for byte in [0x55, 0xaa, 0x55, 0xaa] {
        s.send(0x57, byte);
    }
    s.init();
    s.line("writew 0x41600108 0x1af", "OK");
    s.step(90_000);
    s.line("readl 0x41600020", "OK 0x00000000000000d6");
    for byte in [0; 17] {
        s.send(0x57, byte);
    }
    s.command(0x57, 0, &[0; 16]);
    s.command(0x57, 0x17, &[]);
    s.read(0x57, &[3]);
    s.command(0x57, 0x18, &[]);
    s.read(0x57, &[64]);
    let (script, responses) = s.finish();
    Transactions {
        script,
        responses,
        power,
    }
}
