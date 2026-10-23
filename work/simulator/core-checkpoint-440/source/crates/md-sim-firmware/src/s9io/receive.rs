//! Clocked receive state, serial CRC, FIFOs and IRQ latches from S9IO0.2.
//! Each tick is one core clock. UART input is the FIFO head before that
//! clock. The caller removes it only when `read_uart` is set.
use super::{
    crc::{Kind, SerialCrc},
    decode_response, Midstates, Response,
};
use std::collections::VecDeque;

pub const COMMAND_WORDS: usize = 256;
pub const WORK_WORDS: usize = 1024;

#[derive(Default, Clone, Copy)]
pub struct Control {
    pub enabled: bool,
    pub clear_command: bool,
    pub clear_work: bool,
    pub clear_errors: bool,
    pub read_command: bool,
    pub read_work: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Command,
    Work,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Output {
    pub read_uart: bool,
    /// A hardware write strobe. A full command FIFO can discard this write.
    pub write: Option<Destination>,
    pub checksum_error: bool,
    pub command_read: Option<u32>,
    pub work_read: Option<u32>,
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Wait,
    Read(u8),
    Calculate(u8),
    Check,
    Write(usize),
    Error,
}

pub struct ReceiveFifos {
    state: State,
    window: [u8; 7],
    crc: SerialCrc,
    errors: u32,
    command: VecDeque<u32>,
    work: VecDeque<u32>,
    command_irq: bool,
    work_irq: bool,
}

impl Default for ReceiveFifos {
    fn default() -> Self {
        Self {
            state: State::Idle,
            window: [0; 7],
            crc: SerialCrc::new(Kind::Response),
            errors: 0,
            command: VecDeque::new(),
            work: VecDeque::new(),
            command_irq: false,
            work_irq: false,
        }
    }
}

impl ReceiveFifos {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn errors(&self) -> u32 {
        self.errors
    }

    /// UART input must remain empty and CPU/clear strobes must be absent.
    pub(super) fn idle(&self, enabled: bool) -> bool {
        self.crc.ready()
            && if enabled {
                matches!(self.state, State::Wait | State::Read(_))
            } else {
                matches!(self.state, State::Idle) && !self.command_irq && !self.work_irq
            }
    }

    pub fn tick(
        &mut self,
        control: Control,
        uart: Option<u8>,
        last_work_id: u16,
        midstates: Midstates,
    ) -> Output {
        let mut output = Output::default();
        let work_full = self.work.len() == WORK_WORDS;
        let command_full = self.command.len() == COMMAND_WORDS;
        // Read and write decisions use pre-clock fullness. A simultaneous
        // read cannot make a full write succeed or read a just-arrived word.
        if control.read_command {
            output.command_read = self.command.pop_front();
        }
        if control.read_work {
            output.work_read = self.work.pop_front();
        }
        let mut crc_byte = None;
        let mut clear_crc = false;
        let mut crc_init = 0;
        if control.clear_command || control.read_command || !control.enabled {
            self.command_irq = false;
        }
        if control.clear_work || control.read_work || !control.enabled {
            self.work_irq = false;
        }
        match self.state {
            State::Idle if control.enabled => self.state = State::Wait,
            State::Wait if uart.is_some() => self.state = State::Read(0),
            State::Read(index) => {
                if let Some(byte) = uart {
                    output.read_uart = true;
                    self.window.rotate_left(1);
                    self.window[6] = byte;
                    self.state = if index == 6 {
                        clear_crc = true;
                        crc_init = if byte & 0x80 != 0 { 0x1b } else { 3 };
                        State::Calculate(0)
                    } else {
                        State::Read(index + 1)
                    };
                }
            }
            State::Calculate(index) if self.crc.ready() => {
                crc_byte = Some(self.window[usize::from(index)]);
                self.state = if index == 5 {
                    State::Check
                } else {
                    State::Calculate(index + 1)
                };
            }
            State::Check if self.crc.ready() => {
                self.state = if self.crc.value() == u16::from(self.window[6] & 31) {
                    State::Write(0)
                } else {
                    State::Error
                };
            }
            // Preserve the source defect: COMMAND writes also test WORK
            // fullness. Command-full discards data but still completes IRQ.
            State::Write(index) if !work_full => {
                let (queue, full, irq, words, destination) =
                    match decode_response(&self.window, last_work_id, midstates) {
                        Response::Command(words) => (
                            &mut self.command,
                            command_full,
                            &mut self.command_irq,
                            words,
                            Destination::Command,
                        ),
                        Response::Work(words) => (
                            &mut self.work,
                            work_full,
                            &mut self.work_irq,
                            words,
                            Destination::Work,
                        ),
                    };
                if !full {
                    queue.push_back(words[index]);
                }
                output.write = Some(destination);
                if index == 1 {
                    *irq = true;
                    self.state = State::Wait;
                } else {
                    self.state = State::Write(1);
                }
            }
            State::Error => {
                self.errors = self.errors.wrapping_add(1);
                output.checksum_error = true;
                self.state = State::Read(6);
            }
            _ => (),
        }
        self.crc.tick(crc_byte, clear_crc, crc_init);
        // FIFO clear wins over a concurrent write, but the core IRQ's
        // second-write assertion wins over its clear (source logic order).
        if control.clear_command {
            self.command.clear();
        }
        if control.clear_work {
            self.work.clear();
        }
        if control.clear_errors {
            self.errors = 0;
        }
        // Disable chooses the next Idle state. It does not cancel outputs
        // from the current clock or reset the CRC shift engine.
        if !control.enabled {
            self.state = State::Idle;
        }
        output
    }

    /// CPU read strobes are separate transactions in this component.
    /// They clear the IRQ even when more FIFO words remain.
    pub fn read_command(&mut self) -> Option<u32> {
        self.command_irq = false;
        self.command.pop_front()
    }

    pub fn read_work(&mut self) -> Option<u32> {
        self.work_irq = false;
        self.work.pop_front()
    }

    /// Receive bits only: IRQ_PENDING=4, FULL=1, EMPTY=0.
    pub fn command_status(&self) -> u32 {
        status(&self.command, COMMAND_WORDS, self.command_irq)
    }

    pub fn work_status(&self) -> u32 {
        status(&self.work, WORK_WORDS, self.work_irq)
    }
}

fn status(queue: &VecDeque<u32>, limit: usize, irq: bool) -> u32 {
    u32::from(irq) << 4 | u32::from(queue.len() == limit) << 1 | u32::from(queue.is_empty())
}
