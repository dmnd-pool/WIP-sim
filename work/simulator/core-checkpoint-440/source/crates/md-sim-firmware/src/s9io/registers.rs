//! Functional register transactions for the archived S9IO0.2 AXI wrapper.
//! Offsets and byte strobes follow s9io_v0_2_S00_AXI.vhd. `cycle` supplies
//! simultaneous bus/core strobes; the `axi` module supplies bus handshakes.
//! `tick` connects both core state machines to the actual clocked UART.
//! `tick_core` exposes the UART FIFO boundary for component tests.
use super::{
    receive::{self, ReceiveFifos},
    transmit::{self, Control, Ready, TransmitFifos},
    uart::{self, Uart},
    Error, Midstates,
};

pub const VERSION: u16 = 0x0000;
pub const BUILD_ID: u16 = 0x0004;
pub const CONTROL: u16 = 0x0008;
pub const BAUD: u16 = 0x0010;
pub const WORK_TIME: u16 = 0x0014;
pub const ERRORS: u16 = 0x0018;
pub const COMMAND_RX: u16 = 0x1000;
pub const COMMAND_TX: u16 = 0x1004;
pub const COMMAND_CONTROL: u16 = 0x1008;
pub const COMMAND_STATUS: u16 = 0x100c;
pub const WORK_RX: u16 = 0x2000;
pub const WORK_RX_CONTROL: u16 = 0x2008;
pub const WORK_RX_STATUS: u16 = 0x200c;
pub const WORK_TX: u16 = 0x3004;
pub const WORK_TX_CONTROL: u16 = 0x3008;
pub const WORK_TX_STATUS: u16 = 0x300c;
pub const WORK_TX_THRESHOLD: u16 = 0x3010;
pub const LAST_WORK_ID: u16 = 0x3014;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusError {
    OutsideWindow,
    /// The HDL's FIFO read register has no reset assignment. Do not invent
    /// its value before any successful read.
    UninitializedRead,
    /// The selected reference contract reports a destructive transition,
    /// but its internal recovery behavior is not implemented.
    UnsupportedTransition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Revision {
    Legacy0_2,
    /// BM1387 interface from the pinned DCENT_OS S9 driver contract.
    /// Shared FIFO internals are an explicit inference from legacy HDL.
    /// This is not a validated implementation of the selected FPGA image.
    Reference1_0_2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Write {
    Register,
    Fifo { accepted: bool },
    Ignored,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CoreOutput {
    pub tx: transmit::Output,
    pub rx: receive::Output,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Output {
    pub core: CoreOutput,
    pub uart: uart::Output,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Cycle {
    pub device: Output,
    pub read: Option<Result<u32, BusError>>,
    pub write: Option<Result<Write, BusError>>,
}

pub struct Registers {
    revision: Revision,
    build_id: u32,
    transmitted: bool,
    control: u32,
    command_control: u32,
    work_rx_control: u32,
    work_tx_control: u32,
    baud: u32,
    work_time: u32,
    threshold: u32,
    command_read: Option<u32>,
    work_read: Option<u32>,
    tx: TransmitFifos,
    rx: ReceiveFifos,
    uart: Uart,
}

/// Read-only clock bounds for diagnosing a stalled interface.
pub struct IdleLimits {
    pub receive: u64,
    pub transmit: u64,
    pub uart: u64,
}

impl Registers {
    /// Build ID is an explicit input. A filename is not a build identity.
    /// This version is the HDL's literal00900200, which conflicts with the
    /// archive's SVD/testbench00090002. No compatibility override is applied.
    pub fn new(build_id: u32) -> Self {
        Self::with_revision(build_id, Revision::Legacy0_2)
    }

    pub fn with_revision(build_id: u32, revision: Revision) -> Self {
        Self {
            revision,
            build_id,
            transmitted: false,
            control: 0,
            command_control: 0,
            work_rx_control: 0,
            work_tx_control: 0,
            baud: 0,
            work_time: 1,
            threshold: 0,
            command_read: None,
            work_read: None,
            tx: TransmitFifos::default(),
            rx: ReceiveFifos::default(),
            uart: Uart::default(),
        }
    }

    pub fn reset(&mut self) {
        // FPGA IP reset clears FIFO pointers, not synchronous read outputs.
        let command_read = self.command_read;
        let work_read = self.work_read;
        *self = Self::with_revision(self.build_id, self.revision);
        self.command_read = command_read;
        self.work_read = work_read;
    }

    pub fn read(&mut self, offset: u16) -> Result<u32, BusError> {
        Ok(match address(offset)? {
            VERSION => match self.revision {
                Revision::Legacy0_2 => 0x00900200,
                Revision::Reference1_0_2 => 0x00901002,
            },
            BUILD_ID => self.build_id,
            CONTROL => self.control,
            BAUD => self.baud,
            WORK_TIME => self.work_time,
            ERRORS => self.rx.errors(),
            COMMAND_RX => {
                if let Some(word) = self.rx.read_command() {
                    self.command_read = Some(word);
                }
                self.command_read.ok_or(BusError::UninitializedRead)?
            }
            WORK_RX => {
                if let Some(word) = self.rx.read_work() {
                    self.work_read = Some(word);
                }
                self.work_read.ok_or(BusError::UninitializedRead)?
            }
            COMMAND_CONTROL => self.command_control,
            COMMAND_STATUS => self.tx.command_status() | self.rx.command_status(),
            WORK_RX_CONTROL => self.work_rx_control,
            WORK_RX_STATUS => self.rx.work_status(),
            WORK_TX_CONTROL => self.work_tx_control,
            WORK_TX_STATUS => self.tx.work_status(self.threshold as u16),
            WORK_TX_THRESHOLD => self.threshold,
            LAST_WORK_ID => u32::from(self.tx.last_work_id()),
            _ => 0,
        })
    }

    /// Each set strobe selects one byte. Ordinary registers retain the
    /// other bytes. FIFO writes use zero in unselected lanes, including a
    /// zero word when no lane is selected. Full FIFO writes are discarded.
    pub fn write(&mut self, offset: u16, value: u32, strobes: u8) -> Result<Write, BusError> {
        let mask = (0..4)
            .filter(|byte| strobes & (1 << byte) != 0)
            .fold(0, |mask, byte| mask | (0xff << (8 * byte)));
        let offset = address(offset)?;
        if offset == CONTROL && self.revision == Revision::Reference1_0_2 && self.transmitted {
            let next = (self.control & !mask) | (value & mask);
            if self.control & 8 != 0 && next & 8 == 0 && (self.control ^ next) & 6 != 0 {
                return Err(BusError::UnsupportedTransition);
            }
        }
        let target = match offset {
            COMMAND_TX => {
                return Ok(Write::Fifo {
                    accepted: self.tx.write_command(value & mask),
                });
            }
            WORK_TX => {
                return Ok(Write::Fifo {
                    accepted: self.tx.write_work(value & mask),
                });
            }
            CONTROL => &mut self.control,
            COMMAND_CONTROL => &mut self.command_control,
            WORK_RX_CONTROL => &mut self.work_rx_control,
            WORK_TX_CONTROL => &mut self.work_tx_control,
            BAUD => &mut self.baud,
            WORK_TIME => &mut self.work_time,
            WORK_TX_THRESHOLD => &mut self.threshold,
            _ => return Ok(Write::Ignored),
        };
        *target = (*target & !mask) | (value & mask);
        Ok(Write::Register)
    }

    pub fn uart_divisor(&self) -> u16 {
        (self.baud & 0xfff) as u16
    }

    /// The reference S9 contract uses a 200MHz serializer. The legacy HDL
    /// uses 50MHz. Dynamic fabric clock/reset coupling is not supplied here.
    pub fn core_period_ns(&self) -> u32 {
        match self.revision {
            Revision::Legacy0_2 => 20,
            Revision::Reference1_0_2 => 5,
        }
    }

    fn unsupported_mode(&self) -> bool {
        self.revision == Revision::Reference1_0_2 && self.control & 0x18 == 0x18
    }

    pub fn midstates(&self) -> Midstates {
        match (self.control >> 1) & 3 {
            0 => Midstates::One,
            1 => Midstates::Two,
            _ => Midstates::Four,
        }
    }

    /// One core/UART clock. All inputs are sampled before either state
    /// machine advances, including LAST_WORK_ID for receive word two.
    pub fn tick(&mut self, rxd: bool) -> Result<Output, Error> {
        Ok(self.cycle(rxd, None, None)?.device)
    }

    /// Clocks before the next externally visible change, if bus strobes are
    /// absent and RXD stays constant. Querying does not advance any state.
    pub fn idle_limit(&self, rxd: bool) -> u64 {
        let limits = self.idle_limits(rxd);
        limits.receive.min(limits.transmit).min(limits.uart)
    }

    pub fn idle_limits(&self, rxd: bool) -> IdleLimits {
        let enabled = self.control & 8 != 0;
        IdleLimits {
            receive: if self.unsupported_mode() || self.control & 1 != 0 || !self.rx.idle(enabled) {
                0
            } else {
                u64::MAX
            },
            transmit: self.tx.idle_limit(
                enabled,
                self.quiet_checks(),
                self.uart.tx_full(),
                self.midstates(),
            ),
            uart: self.uart.idle_limit(self.uart_divisor(), rxd),
        }
    }

    fn quiet_checks(&self) -> bool {
        self.control & 8 != 0
            && (self.command_control | self.work_rx_control | self.work_tx_control) & 3 == 0
    }

    /// Advance clocks with no externally visible change. Internal queue
    /// queue checks and timer ACKs retain their exact phase. No reset request
    /// may be pending when those ACKs are skipped. Bus strobes must be absent
    /// and RXD must remain constant. Return the clocks advanced; the caller
    /// must tick the next event. The FPGA clock rate is unchanged.
    pub fn skip_idle(&mut self, clocks: u64, rxd: bool) -> u64 {
        let clocks = clocks.min(self.idle_limit(rxd));
        self.tx.advance_idle(
            clocks,
            self.work_time,
            self.quiet_checks(),
            self.midstates(),
        );
        self.uart.advance_idle(clocks);
        clocks
    }

    /// Core, UART and optional CPU strobes on one edge. FIFO decisions use
    /// pre-edge fullness. Register writes take effect after core sampling.
    pub fn cycle(
        &mut self,
        rxd: bool,
        read: Option<u16>,
        write: Option<(u16, u32, u8)>,
    ) -> Result<Cycle, Error> {
        let division = self.uart_divisor();
        let ready = Ready {
            uart: !self.uart.tx_full(),
            uart_sync_busy: self.uart.sync_busy(division),
            crc5: true,
            crc16: true,
        };
        let command_full = self.tx.command_status() & 8 != 0;
        let work_full = self.tx.work_status(0) & 8 != 0;
        let read_address = read.map(address);
        let fifo_read = match read_address {
            Some(Ok(COMMAND_RX | WORK_RX)) => read_address.unwrap().ok(),
            _ => None,
        };
        let mut result = if fifo_read.is_none() {
            read.map(|offset| self.read(offset))
        } else {
            None
        };
        let core = self.clock_core(ready, self.uart.rx_head(), fifo_read)?;
        let mut uart = self.uart.tick(
            division,
            rxd,
            core.tx.byte,
            core.rx.read_uart,
            core.tx.clear_uart,
        );
        self.transmitted |= uart.started.is_some() && self.serial_output_enabled();
        if let Some(offset) = fifo_read {
            result = Some(self.fifo_output(offset));
        }
        let written = write.map(|(offset, value, strobes)| {
            let blocked = match address(offset)? {
                COMMAND_TX => command_full || core.tx.clear_command_tx,
                WORK_TX => work_full || core.tx.clear_work_tx,
                _ => false,
            };
            if blocked {
                Ok(Write::Fifo { accepted: false })
            } else {
                self.write(offset, value, strobes)
            }
        });
        if !self.serial_output_enabled() {
            uart.txd = false;
            uart.started = None;
        }
        Ok(Cycle {
            device: Output { core, uart },
            read: result,
            write: written,
        })
    }

    pub fn uart_errors(&self) -> (bool, bool) {
        self.uart.errors()
    }

    pub fn txd(&self) -> bool {
        // The reference S9 driver's reset contract uses ENABLE-low as an
        // ASIC BREAK. The legacy HDL's UART output remains unchanged.
        self.serial_output_enabled() && self.uart.txd()
    }

    fn serial_output_enabled(&self) -> bool {
        self.revision != Revision::Reference1_0_2 || self.control & 8 != 0
    }

    pub fn tick_core(&mut self, ready: Ready, uart: Option<u8>) -> Result<CoreOutput, Error> {
        self.clock_core(ready, uart, None)
    }

    fn clock_core(
        &mut self,
        ready: Ready,
        uart: Option<u8>,
        fifo_read: Option<u16>,
    ) -> Result<CoreOutput, Error> {
        if self.unsupported_mode() {
            return Err(Error::UnsupportedAsicMode);
        }
        let control = Control {
            enabled: self.control & 8 != 0,
            midstates: self.midstates(),
            work_time: self.work_time,
            reset_command_tx: self.command_control & 2 != 0,
            reset_command_rx: self.command_control & 1 != 0,
            reset_work_tx: self.work_tx_control & 2 != 0,
            reset_work_rx: self.work_rx_control & 1 != 0,
        };
        let last_work_id = self.tx.last_work_id();
        let output = self.tx.tick(control, ready)?;
        let rx = self.rx.tick(
            receive::Control {
                enabled: control.enabled,
                clear_command: output.clear_command_rx,
                clear_work: output.clear_work_rx,
                clear_errors: output.clear_receive_errors || self.control & 1 != 0,
                read_command: fifo_read == Some(COMMAND_RX),
                read_work: fifo_read == Some(WORK_RX),
            },
            uart,
            last_work_id,
            self.midstates(),
        );
        if let Some(word) = rx.command_read {
            self.command_read = Some(word);
        }
        if let Some(word) = rx.work_read {
            self.work_read = Some(word);
        }
        self.control &= !1;
        if output.work_time_ack {
            self.command_control &= !3;
            self.work_rx_control &= !3;
            self.work_tx_control &= !3;
        }
        Ok(CoreOutput { tx: output, rx })
    }

    /// Synchronous FIFO read outputs, without another read strobe.
    pub(super) fn fifo_output(&self, offset: u16) -> Result<u32, BusError> {
        let value = match offset {
            COMMAND_RX => self.command_read,
            WORK_RX => self.work_read,
            _ => return Err(BusError::OutsideWindow),
        };
        value.ok_or(BusError::UninitializedRead)
    }

    /// Bit0 work transmit, bit1 work receive, bit2 command receive.
    pub fn interrupts(&self) -> u8 {
        u8::from(
            self.tx
                .work_irq(self.threshold as u16, self.work_tx_control & 4 != 0),
        ) | (u8::from(self.rx.work_status() & 16 != 0 && self.work_rx_control & 4 != 0) << 1)
            | (u8::from(self.rx.command_status() & 16 != 0 && self.command_control & 4 != 0) << 2)
    }
}

fn address(offset: u16) -> Result<u16, BusError> {
    if offset < 0x4000 {
        // The AXI wrapper selects address bits13:2, ignoring the low bits.
        Ok(offset & !3)
    } else {
        Err(BusError::OutsideWindow)
    }
}
