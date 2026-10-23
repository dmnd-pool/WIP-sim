//! S9IO0.2 transmit state and FIFOs, from the archived Braiins HDL.
//! UART ready inputs belong to the enclosing clocked device. Serial CRC
//! engines advance on each core clock. External CRC readiness can stall them.
//! Each `tick` advances one core clock; it does not simulate a UART bit.
//! Registers arbitrate simultaneous CPU/core strobes. The AXI and UART
//! modules supply bus handshakes and serial timing.
use super::{
    crc::{Kind, SerialCrc},
    Error, Midstates,
};
use std::collections::VecDeque;

pub const COMMAND_WORDS: usize = 1024;
pub const WORK_WORDS: usize = 2048;
const TIMER_MASK: u32 = 0x00ff_ffff;

#[derive(Clone, Copy, Debug)]
pub struct Control {
    pub enabled: bool,
    pub midstates: Midstates,
    /// Hardware clocks, not mining attempts. Only the low24 bits exist.
    pub work_time: u32,
    pub reset_command_tx: bool,
    pub reset_command_rx: bool,
    pub reset_work_tx: bool,
    pub reset_work_rx: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Ready {
    pub uart: bool,
    pub crc5: bool,
    pub crc16: bool,
    pub uart_sync_busy: bool,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Output {
    pub byte: Option<u8>,
    pub work_time_ack: bool,
    pub clear_command_rx: bool,
    pub clear_work_rx: bool,
    pub clear_receive_errors: bool,
    pub clear_uart: bool,
    pub clear_command_tx: bool,
    pub clear_work_tx: bool,
}

#[derive(Default, Clone, Copy)]
enum State {
    #[default]
    Idle,
    Wait,
    Check,
    WorkHeader,
    WorkLength,
    WorkId,
    WorkOptions,
    WorkNonce(u8),
    WorkData {
        byte: u8,
        remaining: u8,
    },
    WorkMidstate {
        byte: u8,
        remaining: u8,
    },
    WorkChecksum(u8),
    CommandRead,
    CommandCheck,
    CommandData {
        byte: u8,
        remaining: u8,
    },
    CommandChecksum,
}

pub struct TransmitFifos {
    command: VecDeque<u32>,
    work: VecDeque<u32>,
    command_read: u32,
    work_read: u32,
    state: State,
    timer: u32,
    due: bool,
    last_work_id: u16,
    crc5: SerialCrc,
    crc16: SerialCrc,
}

impl Default for TransmitFifos {
    fn default() -> Self {
        Self {
            command: VecDeque::new(),
            work: VecDeque::new(),
            command_read: 0,
            work_read: 0,
            state: State::Idle,
            timer: 0,
            due: false,
            last_work_id: 0,
            crc5: SerialCrc::new(Kind::Command),
            crc16: SerialCrc::new(Kind::Work),
        }
    }
}

impl TransmitFifos {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// A write to a full hardware FIFO is discarded.
    pub fn write_command(&mut self, word: u32) -> bool {
        write(&mut self.command, COMMAND_WORDS, word)
    }

    pub fn write_work(&mut self, word: u32) -> bool {
        write(&mut self.work, WORK_WORDS, word)
    }

    pub fn last_work_id(&self) -> u16 {
        self.last_work_id
    }

    /// Transmit bits only: FULL=3, EMPTY=2. Merge with receive status.
    pub fn command_status(&self) -> u32 {
        flags(self.command.len(), COMMAND_WORDS)
    }

    /// Preserve the HDL's pointer-wrap behavior: a full2048-word FIFO has
    /// volume0 and asserts the pending IRQ for a nonzero threshold.
    /// The IRQ is a level, not a read-cleared latch.
    pub fn work_status(&self, threshold: u16) -> u32 {
        let volume = self.work.len() & (WORK_WORDS - 1);
        let pending = self.work.is_empty() || volume < usize::from(threshold & 0x7ff);
        flags(self.work.len(), WORK_WORDS) | (u32::from(pending) << 4)
    }

    pub fn work_irq(&self, threshold: u16, irq_enabled: bool) -> bool {
        irq_enabled && self.work_status(threshold) & 16 != 0
    }

    /// Checks for complete work may repeat without changing visible state.
    /// The caller must exclude pending reset requests in that case.
    pub(super) fn idle_limit(
        &self,
        enabled: bool,
        quiet_checks: bool,
        uart_full: bool,
        midstates: Midstates,
    ) -> u64 {
        if !self.crc5.ready() || !self.crc16.ready() {
            return 0;
        }
        if quiet_checks && self.waiting_for_work(midstates) {
            return u64::MAX;
        }
        // A CPU can be preempted between FIFO writes. An incomplete packet
        // must retain its words and state, but it cannot emit another byte
        // until the missing word arrives. Only the free timer advances.
        if enabled && self.waiting_for_word() {
            return u64::MAX;
        }
        // These states can only emit a byte when UART space is available.
        // The enclosing UART bounds the skip before any FIFO removal.
        // CRC engines must already be idle; the free timer still advances.
        if enabled
            && uart_full
            && matches!(
                self.state,
                State::WorkHeader
                    | State::WorkLength
                    | State::WorkId
                    | State::WorkOptions
                    | State::WorkNonce(_)
                    | State::WorkData { .. }
                    | State::WorkMidstate { .. }
                    | State::WorkChecksum(_)
                    | State::CommandData { .. }
                    | State::CommandChecksum
            )
        {
            return u64::MAX;
        }
        match self.state {
            State::Idle if !enabled => u64::MAX,
            State::Wait if enabled && !self.due => u64::from(self.timer) + 1,
            _ => 0,
        }
    }

    fn waiting_for_work(&self, midstates: Midstates) -> bool {
        matches!(self.state, State::Wait | State::Check)
            && self.command.is_empty()
            && self.work.len() < midstates.work_words()
    }

    fn waiting_for_word(&self) -> bool {
        match self.state {
            State::WorkLength | State::WorkNonce(_) | State::WorkData { .. } => {
                self.work.is_empty()
            }
            State::WorkMidstate { remaining, .. } => remaining != 0 && self.work.is_empty(),
            State::CommandData { remaining, .. } => remaining != 0 && self.command.is_empty(),
            _ => false,
        }
    }

    /// The caller has checked `idle_limit`. Queue checks retain exact timer
    /// and state phase, including the ACK priority over timer expiration.
    pub(super) fn advance_idle(
        &mut self,
        mut clocks: u64,
        reload: u32,
        quiet_checks: bool,
        midstates: Midstates,
    ) {
        if !quiet_checks || !self.waiting_for_work(midstates) {
            self.advance_free_timer(clocks, reload);
            return;
        }
        let period = u64::from(reload.wrapping_sub(1) & TIMER_MASK) + 1;
        while clocks != 0 {
            // This state recurs every timer period, except period1 needs
            // two edges: ACK selects Check, then Check selects Wait.
            if matches!(self.state, State::Wait) && self.due && u64::from(self.timer) == period - 1
            {
                clocks %= period.max(2);
                if clocks == 0 {
                    return;
                }
            }
            match self.state {
                State::Check => {
                    self.state = State::Wait;
                    self.advance_timer(reload, false);
                    clocks -= 1;
                }
                State::Wait if self.due => {
                    self.state = State::Check;
                    self.advance_timer(reload, true);
                    clocks -= 1;
                }
                State::Wait => {
                    let span = clocks.min(u64::from(self.timer) + 1);
                    self.advance_free_timer(span, reload);
                    clocks -= span;
                }
                _ => unreachable!("queue polling state checked above"),
            }
        }
    }

    fn advance_free_timer(&mut self, clocks: u64, reload: u32) {
        let first = u64::from(self.timer) + 1;
        if clocks < first {
            self.timer -= clocks as u32;
        } else {
            let period = u64::from(reload.wrapping_sub(1) & TIMER_MASK) + 1;
            self.timer = (period - 1 - (clocks - first) % period) as u32;
            self.due = true;
        }
    }

    /// Unsupported command lengths stop the component with an error. They
    /// are not converted to an invented hardware reply or successful send.
    pub fn tick(&mut self, control: Control, ready: Ready) -> Result<Output, Error> {
        let mut output = Output::default();
        let work_ready = ready.uart && ready.crc16 && self.crc16.ready();
        let command_ready = ready.uart && ready.crc5 && self.crc5.ready();
        let previous = self.state;
        let clear5 = matches!(previous, State::CommandRead);
        let mut clear16 = false;
        match self.state {
            State::Idle if control.enabled => {
                self.command.clear();
                self.work.clear();
                output.clear_command_tx = true;
                output.clear_work_tx = true;
                self.state = State::Wait;
                output.clear_command_rx = true;
                output.clear_work_rx = true;
                output.clear_receive_errors = true;
                output.clear_uart = true;
            }
            State::Idle => (),
            State::Wait if self.due && !ready.uart_sync_busy => {
                output.work_time_ack = true;
                output.clear_command_rx = control.reset_command_rx;
                output.clear_work_rx = control.reset_work_rx;
                if control.reset_command_tx {
                    self.command.clear();
                    output.clear_command_tx = true;
                }
                if control.reset_work_tx {
                    self.work.clear();
                    output.clear_work_tx = true;
                }
                self.state = State::Check;
            }
            State::Check => {
                if self.work.len() >= control.midstates.work_words() {
                    clear16 = true;
                    self.state = State::WorkHeader;
                } else if !self.command.is_empty() {
                    self.state = State::CommandRead;
                } else {
                    self.state = State::Wait;
                }
            }
            State::WorkHeader if work_ready => {
                output.byte = Some(0x21);
                self.state = State::WorkLength;
            }
            State::WorkLength if work_ready && !self.work.is_empty() => {
                output.byte = Some((22 + 32 * control.midstates.count()) as u8);
                self.work_read = self.work.pop_front().unwrap();
                self.state = State::WorkId;
            }
            State::WorkId if work_ready => {
                output.byte = Some((self.work_read & 0x7f) as u8);
                self.last_work_id = self.work_read as u16;
                self.state = State::WorkOptions;
            }
            State::WorkOptions if work_ready => {
                output.byte = Some(control.midstates.count() as u8);
                self.state = State::WorkNonce(0);
            }
            State::WorkNonce(byte) if work_ready && !self.work.is_empty() => {
                output.byte = Some(0);
                self.state = if byte == 3 {
                    self.work_read = self.work.pop_front().unwrap();
                    State::WorkData {
                        byte: 0,
                        remaining: 2,
                    }
                } else {
                    State::WorkNonce(byte + 1)
                };
            }
            State::WorkData { byte, remaining } if work_ready && !self.work.is_empty() => {
                output.byte = Some(self.work_read.to_le_bytes()[usize::from(byte)]);
                self.state = if byte == 3 {
                    self.work_read = self.work.pop_front().unwrap();
                    if remaining == 0 {
                        State::WorkMidstate {
                            byte: 0,
                            remaining: (8 * control.midstates.count() - 1) as u8,
                        }
                    } else {
                        State::WorkData {
                            byte: 0,
                            remaining: remaining - 1,
                        }
                    }
                } else {
                    State::WorkData {
                        byte: byte + 1,
                        remaining,
                    }
                };
            }
            State::WorkMidstate { byte, remaining }
                if work_ready && (!self.work.is_empty() || remaining == 0) =>
            {
                output.byte = Some(self.work_read.to_be_bytes()[usize::from(byte)]);
                self.state = if byte == 3 {
                    if remaining == 0 {
                        State::WorkChecksum(0)
                    } else {
                        self.work_read = self.work.pop_front().unwrap();
                        State::WorkMidstate {
                            byte: 0,
                            remaining: remaining - 1,
                        }
                    }
                } else {
                    State::WorkMidstate {
                        byte: byte + 1,
                        remaining,
                    }
                };
            }
            State::WorkChecksum(byte) if work_ready => {
                output.byte = Some(self.crc16.value().to_be_bytes()[usize::from(byte)]);
                self.state = if byte == 0 {
                    State::WorkChecksum(1)
                } else if !self.command.is_empty() {
                    State::CommandRead
                } else {
                    State::Wait
                };
            }
            State::CommandRead => {
                // The synchronous FIFO output retains its last word on an
                // empty read. Queue resets are applied only in Wait/Idle.
                if let Some(word) = self.command.pop_front() {
                    self.command_read = word;
                }
                self.state = State::CommandCheck;
            }
            State::CommandCheck => {
                let length = ((self.command_read >> 8) & 0xff) as u8;
                if !matches!(length, 5 | 9) {
                    return Err(Error::UnsupportedCommandLength);
                }
                self.state = State::CommandData {
                    byte: 0,
                    remaining: (length - 2) >> 2,
                };
            }
            State::CommandData { byte, remaining }
                if command_ready && (!self.command.is_empty() || remaining == 0) =>
            {
                output.byte = Some(self.command_read.to_le_bytes()[usize::from(byte)]);
                self.state = if byte == 3 {
                    if remaining == 0 {
                        State::CommandChecksum
                    } else {
                        self.command_read = self.command.pop_front().unwrap();
                        State::CommandData {
                            byte: 0,
                            remaining: remaining - 1,
                        }
                    }
                } else {
                    State::CommandData {
                        byte: byte + 1,
                        remaining,
                    }
                };
            }
            State::CommandChecksum if command_ready => {
                output.byte = Some(self.crc5.value() as u8);
                self.state = State::Wait;
            }
            _ => (),
        }
        let checksum_byte = matches!(previous, State::CommandChecksum | State::WorkChecksum(_));
        let command_byte = matches!(previous, State::CommandData { .. });
        self.crc5
            .tick(output.byte.filter(|_| command_byte), clear5, 0);
        self.crc16.tick(
            output.byte.filter(|_| !command_byte && !checksum_byte),
            clear16,
            0,
        );
        self.advance_timer(control.work_time, output.work_time_ack);
        Ok(self.finish(control, output))
    }

    fn finish(&mut self, control: Control, output: Output) -> Output {
        // In the HDL, disable selects Idle after this clock's outputs.
        // It does not clear LAST_WORK_ID or the free-running work timer.
        if !control.enabled {
            self.state = State::Idle;
        }
        output
    }

    fn advance_timer(&mut self, reload: u32, clear: bool) {
        if self.timer == 0 {
            self.timer = reload.wrapping_sub(1) & TIMER_MASK;
            self.due = true;
        } else {
            self.timer -= 1;
        }
        if clear {
            self.due = false;
        }
    }
}

fn write(queue: &mut VecDeque<u32>, capacity: usize, word: u32) -> bool {
    if queue.len() == capacity {
        false
    } else {
        queue.push_back(word);
        true
    }
}

fn flags(length: usize, capacity: usize) -> u32 {
    u32::from(length == 0) << 2 | u32::from(length == capacity) << 3
}
