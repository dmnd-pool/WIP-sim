//! Clocked8N1 UART from the archived S9IO0.2 HDL:16 samples per bit,
//! sixteen-byte FIFOs, three-stage input synchronization and12-bit divider.
//! `tick` is one FPGA clock. Host time and hash-rate caps do not enter it.
use std::collections::VecDeque;

const CAPACITY: usize = 16;

#[derive(Default, Clone, Copy)]
enum State {
    #[default]
    Idle,
    Start,
    Data,
    Stop,
}

#[derive(Default)]
struct Shift {
    state: State,
    data: u8,
    samples: u8,
    bits: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Output {
    pub txd: bool,
    pub read: Option<u8>,
    pub write_accepted: bool,
    /// Diagnostics for actual state transitions, not injected responses.
    pub started: Option<u8>,
    pub received: Option<u8>,
}

pub struct Uart {
    tx: Shift,
    rx: Shift,
    tx_fifo: VecDeque<u8>,
    rx_fifo: VecDeque<u8>,
    rx_sync: [bool; 3],
    txd: bool,
    division: u16,
    counter: u16,
    sample_enable: bool,
    frame_error: bool,
    overrun: bool,
}

impl Default for Uart {
    fn default() -> Self {
        Self {
            tx: Shift::default(),
            rx: Shift::default(),
            tx_fifo: VecDeque::new(),
            rx_fifo: VecDeque::new(),
            rx_sync: [true; 3],
            txd: true,
            division: 0,
            counter: 0,
            sample_enable: false,
            frame_error: false,
            overrun: false,
        }
    }
}

impl Uart {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn txd(&self) -> bool {
        self.txd
    }

    pub fn tx_full(&self) -> bool {
        self.tx_fifo.len() == CAPACITY
    }

    pub fn rx_empty(&self) -> bool {
        self.rx_fifo.is_empty()
    }

    pub fn rx_head(&self) -> Option<u8> {
        self.rx_fifo.front().copied()
    }

    pub fn errors(&self) -> (bool, bool) {
        (self.frame_error, self.overrun)
    }

    /// A changed divisor waits for both the serializer and TX FIFO to drain.
    pub fn sync_busy(&self, division: u16) -> bool {
        !self.tx_idle() && self.division != division & 0xfff
    }

    fn tx_idle(&self) -> bool {
        matches!(self.tx.state, State::Idle) && self.tx_fifo.is_empty()
    }

    /// Clocks with no shift, FIFO, pin or synchronizer change, assuming
    /// constant RXD and no FIFO strobes. Divider and oversample counters
    /// can advance up to, but not through, the next bit-state transition.
    pub(super) fn idle_limit(&self, division: u16, rxd: bool) -> u64 {
        if self.tx_idle()
            && matches!(self.rx.state, State::Idle)
            && self.rx_fifo.is_empty()
            && self.rx_sync == [true; 3]
            && self.txd
            && rxd
            && self.division == division & 0xfff
            && self.counter <= self.division
        {
            return u64::MAX;
        }
        let next_txd = match self.tx.state {
            State::Start => false,
            State::Data => self.tx.data & 1 != 0,
            _ => true,
        };
        if self.division != division & 0xfff
            || self.counter > self.division
            || !self.rx_fifo.is_empty()
            || self.rx_sync != [rxd; 3]
            || self.txd != next_txd
            || (matches!(self.rx.state, State::Idle) && !rxd)
        {
            return 0;
        }
        let tx_samples = if matches!(self.tx.state, State::Idle) {
            if self.tx_fifo.is_empty() {
                u64::MAX
            } else {
                0
            }
        } else {
            u64::from(15 - self.tx.samples)
        };
        let rx_samples = if matches!(self.rx.state, State::Idle) {
            u64::MAX
        } else {
            let last = if matches!(self.rx.state, State::Start) {
                7
            } else {
                15
            };
            u64::from(last - self.rx.samples)
        };
        self.first_sample()
            .saturating_add(tx_samples.min(rx_samples).saturating_mul(self.period()))
    }

    fn period(&self) -> u64 {
        u64::from(self.division) + 1
    }

    /// Zero-based edge at which the old, registered enable is first used.
    fn first_sample(&self) -> u64 {
        if self.sample_enable {
            0
        } else {
            u64::from(self.division - self.counter) + 1
        }
    }

    /// The caller has checked `idle_limit` and supplies no FIFO strobes.
    pub(super) fn advance_idle(&mut self, clocks: u64) {
        if clocks != 0 {
            let period = self.period();
            let first = self.first_sample();
            let samples = if clocks <= first {
                0
            } else {
                1 + (clocks - 1 - first) / period
            };
            // idle_limit prevents either counter from reaching a state
            // transition. Idle state machines do not consume samples.
            if !matches!(self.tx.state, State::Idle) {
                self.tx.samples += u8::try_from(samples).expect("bounded TX samples");
            }
            if !matches!(self.rx.state, State::Idle) {
                self.rx.samples += u8::try_from(samples).expect("bounded RX samples");
            }
            self.counter = ((u64::from(self.counter) + clocks % period) % period) as u16;
            self.sample_enable = self.counter == 0;
        }
    }

    /// FIFO clear leaves the bit state machines, divider and sticky errors
    /// intact, as in uart.vhd. Only IP reset clears those states.
    /// Fullness is sampled before concurrent reads, so a full write drops
    /// even when that same clock removes an older byte.
    pub fn tick(
        &mut self,
        division: u16,
        rxd: bool,
        write: Option<u8>,
        read: bool,
        clear: bool,
    ) -> Output {
        let idle = self.tx_idle();
        let tx_full = self.tx_full();
        let rx_full = self.rx_fifo.len() == CAPACITY;
        let started = if matches!(self.tx.state, State::Idle) && self.sample_enable {
            self.tx_fifo.front().copied()
        } else {
            None
        };
        // TX pin has one output register after its state/data registers.
        self.txd = match self.tx.state {
            State::Start => false,
            State::Data => self.tx.data & 1 != 0,
            _ => true,
        };
        self.tx.tick_tx(self.sample_enable, started);
        let (received, frame_error) = self.rx.tick_rx(self.sample_enable, self.rx_sync[0]);
        let result = if read { self.rx_fifo.pop_front() } else { None };
        if let Some(byte) = received {
            if rx_full {
                self.overrun = true;
            } else {
                self.rx_fifo.push_back(byte);
            }
        }
        self.frame_error |= frame_error;
        if started.is_some() {
            self.tx_fifo.pop_front();
        }
        let write_accepted = write.is_some() && !tx_full;
        if let Some(byte) = write.filter(|_| !tx_full) {
            self.tx_fifo.push_back(byte);
        }
        if clear {
            self.tx_fifo.clear();
            self.rx_fifo.clear();
        }
        self.rx_sync.rotate_left(1);
        self.rx_sync[2] = rxd;
        // mod_divider has a registered enable output, and compares against
        // division itself: one sample every(division+1) core clocks.
        self.sample_enable = self.counter == self.division;
        self.counter = if self.sample_enable {
            0
        } else {
            (self.counter + 1) & 0xfff
        };
        if idle {
            self.division = division & 0xfff;
        }
        Output {
            txd: self.txd,
            read: result,
            write_accepted,
            started,
            received,
        }
    }
}

impl Shift {
    fn tick_tx(&mut self, sample: bool, start: Option<u8>) {
        if matches!(self.state, State::Idle) {
            if let Some(byte) = start {
                self.data = byte;
                self.samples = 0;
                self.state = State::Start;
            }
            return;
        }
        if !sample {
            return;
        }
        self.samples = (self.samples + 1) & 15;
        if self.samples != 0 {
            return;
        }
        match self.state {
            State::Start => {
                self.state = State::Data;
                self.bits = 0;
            }
            State::Data => {
                self.data >>= 1;
                if self.bits == 7 {
                    self.state = State::Stop;
                } else {
                    self.bits += 1;
                }
            }
            State::Stop => self.state = State::Idle,
            State::Idle => unreachable!(),
        }
    }

    fn tick_rx(&mut self, sample: bool, bit: bool) -> (Option<u8>, bool) {
        if matches!(self.state, State::Idle) {
            if !bit {
                self.state = State::Start;
                self.samples = 0;
                self.bits = 0;
            }
            return (None, false);
        }
        if !sample {
            return (None, false);
        }
        let last = if matches!(self.state, State::Start) {
            7
        } else {
            15
        };
        if self.samples != last {
            self.samples += 1;
            return (None, false);
        }
        self.samples = 0;
        match self.state {
            State::Start => self.state = if bit { State::Idle } else { State::Data },
            State::Data => {
                self.data = (self.data >> 1) | (u8::from(bit) << 7);
                if self.bits == 7 {
                    self.state = State::Stop;
                } else {
                    self.bits += 1;
                }
            }
            State::Stop => {
                self.state = State::Idle;
                return (bit.then_some(self.data), !bit);
            }
            State::Idle => unreachable!(),
        }
        (None, false)
    }
}
