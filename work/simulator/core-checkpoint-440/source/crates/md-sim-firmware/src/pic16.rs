//! S9 PIC command contract, separate from PIC instruction execution.
//! Only the exact archived application has a known runtime contract.
//! Development storage is explicitly erased outside that application.
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

pub const APP_START: usize = 0x300;
pub const APP_WORDS: usize = 3200;
pub const FLASH_WORDS: usize = 4096;
pub const APP_HASH: &str = "d138aefed416e18a7c5f2a374ef99148c745f7d576b3821e615225fffad1d35b";
const RESET_NS: u64 = 500_000_000;
const COMMAND_NS: u64 = 100_000_000;
// Driver-derived estimate. This contract does not execute Timer0 instructions.
const HEARTBEAT_NS: u64 = 10_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    ProgramFormat,
    Time,
    Busy,
    UnsupportedCommand(u8),
    Mode,
    FlashRange,
    UnknownApplication,
    NoReply,
    UnknownPowerOnVoltage,
}

pub struct Program {
    words: Vec<u16>,
}
impl Program {
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > APP_WORDS * 6 {
            return Err(Error::ProgramFormat);
        }
        let mut words = Vec::with_capacity(APP_WORDS);
        for line in text.lines() {
            if line.len() != 4 || !line.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::ProgramFormat);
            }
            let word = u16::from_str_radix(line, 16).map_err(|_| Error::ProgramFormat)?;
            if word > 0x3fff || words.len() == APP_WORDS {
                return Err(Error::ProgramFormat);
            }
            words.push(word);
        }
        if words.len() != APP_WORDS {
            return Err(Error::ProgramFormat);
        }
        Ok(Self { words })
    }
    pub fn word_bytes(&self) -> Vec<u8> {
        self.words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }
    pub fn sha256(&self) -> String {
        format!("{:x}", Sha256::digest(self.word_bytes()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Loader,
    ApplicationContract,
}

enum Pending {
    Erase { address: usize },
    Write { address: usize, words: [u16; 8] },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadByte {
    pub value: u8,
    /// Upstream only defines the first byte of each I2C read transaction.
    /// Later bytes repeat the output latch as an explicit model assumption.
    pub defined: bool,
}

pub struct Pic {
    flash: Vec<u16>,
    mode: Mode,
    now: u64,
    busy_until: u64,
    pending: Option<Pending>,
    pointer: usize,
    buffer: [u8; 16],
    buffer_valid: bool,
    prefix: u8,
    command: Option<(u8, usize, Vec<u8>)>,
    replies: VecDeque<u8>,
    read_latch: Option<u8>,
    voltage: Option<u8>,
    enabled: bool,
    heartbeat_deadline: Option<u64>,
    heartbeat_remaining: u64,
}

impl Pic {
    /// This is a derived baseline, not a factory dump. The loader is modeled
    /// by its command contract; its code and calibration are not supplied.
    pub fn with_erased_development_storage(program: &Program) -> Self {
        let mut flash = vec![0x3fff; FLASH_WORDS];
        flash[APP_START..APP_START + APP_WORDS].copy_from_slice(&program.words);
        Self {
            flash,
            mode: Mode::Loader,
            now: 0,
            busy_until: 0,
            pending: None,
            pointer: 0,
            buffer: [0; 16],
            buffer_valid: false,
            prefix: 0,
            command: None,
            replies: VecDeque::new(),
            read_latch: None,
            voltage: None,
            enabled: false,
            heartbeat_deadline: None,
            heartbeat_remaining: HEARTBEAT_NS,
        }
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn voltage_enabled(&self) -> bool {
        self.enabled
    }
    pub fn voltage_code(&self) -> Option<u8> {
        self.voltage
    }
    pub fn flash_words(&self) -> &[u16] {
        &self.flash
    }
    pub fn next_deadline(&self) -> Option<u64> {
        [
            Some(self.busy_until).filter(|n| *n > self.now),
            self.heartbeat_deadline,
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn later(&self, delay: u64) -> Result<u64, Error> {
        self.now.checked_add(delay).ok_or(Error::Time)
    }
    pub fn advance(&mut self, now: u64) -> Result<(), Error> {
        if now < self.now {
            return Err(Error::Time);
        }
        self.now = now;
        if now >= self.busy_until {
            if let Some(pending) = self.pending.take() {
                match pending {
                    Pending::Erase { address } => {
                        self.flash[address..address + 32].fill(0x3fff);
                        self.pointer = address + 32;
                    }
                    Pending::Write { address, words } => {
                        for (old, new) in self.flash[address..address + 8].iter_mut().zip(words) {
                            *old &= new;
                        }
                        self.pointer = address + 8;
                    }
                }
            }
        }
        if self
            .heartbeat_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            // Exact app words0f35..0f3a cut power and clear the counter,
            // then feed the hardware WDT and continue the application loop.
            // There is no CPU reset or automatic power recovery on this path.
            self.enabled = false;
            self.heartbeat_deadline = None;
            self.heartbeat_remaining = HEARTBEAT_NS;
        }
        Ok(())
    }
    pub fn ready(&mut self, now: u64) -> Result<bool, Error> {
        self.advance(now)?;
        Ok(now >= self.busy_until)
    }
    /// External reset aborts a pending operation. Flash already committed
    /// persists. Precise interrupted physical cells remain uncalibrated.
    pub fn reset(&mut self, now: u64) -> Result<(), Error> {
        let deadline = now.checked_add(RESET_NS).ok_or(Error::Time)?;
        self.advance(now)?;
        self.mode = Mode::Loader;
        self.pending = None;
        self.pointer = 0;
        self.buffer_valid = false;
        self.prefix = 0;
        self.command = None;
        self.replies.clear();
        self.read_latch = None;
        self.voltage = None;
        self.enabled = false;
        self.heartbeat_deadline = None;
        self.heartbeat_remaining = HEARTBEAT_NS;
        self.busy_until = deadline;
        Ok(())
    }
    pub fn write_byte(&mut self, now: u64, byte: u8) -> Result<(), Error> {
        if !self.ready(now)? {
            return Err(Error::Busy);
        }
        if let Some((command, count, mut data)) = self.command.take() {
            data.push(byte);
            if data.len() == count {
                return self.execute(command, &data);
            }
            self.command = Some((command, count, data));
        } else if self.prefix < 2 {
            let expected = if self.prefix == 0 { 0x55 } else { 0xaa };
            // Preserve the documented 55,55,AA mismatch; do not rematch55.
            self.prefix = if byte == expected { self.prefix + 1 } else { 0 };
        } else {
            self.prefix = 0;
            let count = match byte {
                1 => 2,
                2 => 16,
                0x10 | 0x15 => 1,
                // The exact application consumes one data byte for these
                // invalid commands, then clears its parser. Its EE output
                // goes to UART TX1REG, not to the I2C reply register.
                // A killed host can leave 55 AA before the next flush's 55.
                // This is not a loader contract or a catch-all for unknown
                // commands. See the pinned application parser analysis.
                0 | 0x55 if self.mode == Mode::ApplicationContract => 1,
                _ => 0,
            };
            if count == 0 {
                return self.execute(byte, &[]);
            }
            self.command = Some((byte, count, Vec::with_capacity(count)));
        }
        Ok(())
    }
    pub fn read_ready(&mut self, now: u64) -> Result<bool, Error> {
        Ok(self.ready(now)? && !self.replies.is_empty())
    }
    pub fn read_byte(&mut self, now: u64) -> Result<ReadByte, Error> {
        if !self.ready(now)? {
            return Err(Error::Busy);
        }
        if let Some(value) = self.read_latch {
            return Ok(ReadByte {
                value,
                defined: false,
            });
        }
        let value = self.replies.pop_front().ok_or(Error::NoReply)?;
        self.read_latch = Some(value);
        Ok(ReadByte {
            value,
            defined: true,
        })
    }
    pub fn stop(&mut self) {
        self.read_latch = None;
    }

    fn execute(&mut self, command: u8, data: &[u8]) -> Result<(), Error> {
        self.replies.clear();
        self.read_latch = None;
        if self.mode == Mode::ApplicationContract && matches!(command, 0 | 0x55) {
            // Words039e..03a3,07a8..07ab defer to the next data byte;
            // words04f4..050b fall through to04f0..04f3: state=0. EE is
            // written to the unconnected diagnostic UART, not I2C. This
            // contract does not model that UART or manufacture a reply.
            return Ok(());
        }
        if command == 0 {
            return Ok(());
        } // Driver's explicit parser flush.
        if command == 7 {
            return self.reset(self.now);
        }
        if (1..=8).contains(&command) && self.mode != Mode::Loader {
            return Err(Error::Mode);
        }
        match command {
            1 => {
                let address = usize::from(u16::from_be_bytes([data[0], data[1]]));
                if address >= FLASH_WORDS {
                    return Err(Error::FlashRange);
                }
                self.pointer = address;
            }
            2 => {
                self.buffer.copy_from_slice(data);
                self.buffer_valid = true;
            }
            3 => {
                let words = self
                    .flash
                    .get(self.pointer..self.pointer + 8)
                    .ok_or(Error::FlashRange)?;
                self.replies
                    .extend(words.iter().flat_map(|w| w.to_be_bytes()));
                self.pointer += 8;
            }
            4 => {
                if self.pointer % 32 != 0 || self.pointer + 32 > FLASH_WORDS {
                    return Err(Error::FlashRange);
                }
                let deadline = self.later(COMMAND_NS)?;
                self.pending = Some(Pending::Erase {
                    address: self.pointer,
                });
                self.busy_until = deadline;
            }
            5 => {
                if !self.buffer_valid || self.pointer % 8 != 0 || self.pointer + 8 > FLASH_WORDS {
                    return Err(Error::FlashRange);
                }
                let words = std::array::from_fn(|i| {
                    u16::from_be_bytes([self.buffer[i * 2], self.buffer[i * 2 + 1]])
                });
                if words.iter().any(|w| *w > 0x3fff) {
                    return Err(Error::ProgramFormat);
                }
                let deadline = self.later(COMMAND_NS)?;
                self.pending = Some(Pending::Write {
                    address: self.pointer,
                    words,
                });
                self.busy_until = deadline;
                self.buffer_valid = false;
            }
            6 => {
                let bytes: Vec<_> = self.flash[APP_START..APP_START + APP_WORDS]
                    .iter()
                    .flat_map(|w| w.to_be_bytes())
                    .collect();
                if format!("{:x}", Sha256::digest(bytes)) != APP_HASH {
                    return Err(Error::UnknownApplication);
                }
                let deadline = self.later(COMMAND_NS)?;
                self.mode = Mode::ApplicationContract;
                self.busy_until = deadline;
                self.enabled = false;
                self.heartbeat_deadline = None;
                self.heartbeat_remaining = HEARTBEAT_NS;
            }
            8 => self.replies.extend((self.pointer as u16).to_be_bytes()),
            0x10 | 0x15..=0x18 => {
                if self.mode != Mode::ApplicationContract {
                    return Err(Error::Mode);
                }
                match command {
                    0x10 => {
                        let deadline = self.later(COMMAND_NS)?;
                        self.voltage = Some(data[0]);
                        self.busy_until = deadline;
                    }
                    0x15 => {
                        // Words0478..0481 branch only on STATUS.Z after
                        // reading the payload: every nonzero byte enables.
                        // An interrupted command can consume55 from a new
                        // host's flush here. Preserve that actual behavior.
                        if data[0] != 0 && self.voltage.is_none() {
                            return Err(Error::UnknownPowerOnVoltage);
                        }
                        let enabled = data[0] != 0;
                        if enabled != self.enabled {
                            // Words0478..0481 control active-low RA2. The
                            // loop at0f0c..0f0e skips both software counters
                            // while disabled; ENABLE does not clear either.
                            if enabled {
                                self.heartbeat_deadline =
                                    Some(self.later(self.heartbeat_remaining)?);
                            } else if let Some(deadline) = self.heartbeat_deadline.take() {
                                self.heartbeat_remaining = deadline - self.now;
                            }
                            self.enabled = enabled;
                        }
                    }
                    0x16 => {
                        // Command16 dispatches to0abc: clear both software
                        // counters, leave RA2 unchanged. See PIC16.md.
                        let deadline =
                            self.enabled.then(|| self.later(HEARTBEAT_NS)).transpose()?;
                        self.heartbeat_deadline = deadline;
                        self.heartbeat_remaining = HEARTBEAT_NS;
                    }
                    0x17 => self.replies.push_back(3), // Known image's source contract; no CPU execution claim.
                    0x18 => self
                        .replies
                        .push_back(self.voltage.ok_or(Error::UnknownPowerOnVoltage)?),
                    _ => unreachable!(),
                }
            }
            _ => return Err(Error::UnsupportedCommand(command)),
        }
        Ok(())
    }
}
