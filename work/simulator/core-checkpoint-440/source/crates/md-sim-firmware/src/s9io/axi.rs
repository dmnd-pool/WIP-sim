//! Clocked AXI4-Lite wrapper from archived s9io_v0_2_S00_AXI.vhd.
//! `tick` returns post-edge pins; the handshakes at that edge use old ready
//! and valid outputs. The source's outstanding-response defects are retained.
use super::{
    registers::{BusError, Cycle, Registers, COMMAND_RX, WORK_RX},
    Error,
};

#[derive(Clone, Copy)]
pub struct Input {
    pub reset: bool,
    pub rxd: bool,
    pub awvalid: bool,
    pub awaddr: u16,
    pub wvalid: bool,
    pub wdata: u32,
    pub wstrb: u8,
    pub bready: bool,
    pub arvalid: bool,
    pub araddr: u16,
    pub rready: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            reset: false,
            rxd: true,
            awvalid: false,
            awaddr: 0,
            wvalid: false,
            wdata: 0,
            wstrb: 0,
            bready: false,
            arvalid: false,
            araddr: 0,
            rready: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pins {
    pub awready: bool,
    pub wready: bool,
    pub bvalid: bool,
    pub bresp: u8,
    pub arready: bool,
    pub rvalid: bool,
    pub rresp: u8,
    /// An uninitialized synchronous FIFO output is unknown, not invented0.
    pub rdata: Result<u32, BusError>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Output {
    pub pins: Pins,
    /// No clocked device outputs are returned on an asserted IP reset.
    pub cycle: Option<Cycle>,
}

pub struct Axi {
    registers: Registers,
    awready: bool,
    awaddr: u16,
    wready: bool,
    bvalid: bool,
    arready: bool,
    araddr: u16,
    rvalid: bool,
    rdata: u32,
}

impl Axi {
    pub fn new(build_id: u32) -> Self {
        Self {
            registers: Registers::new(build_id),
            awready: false,
            awaddr: 0,
            wready: false,
            bvalid: false,
            arready: false,
            araddr: u16::MAX,
            rvalid: false,
            rdata: 0,
        }
    }

    pub fn pins(&self) -> Pins {
        let address = self.araddr & 0x3ffc;
        Pins {
            awready: self.awready,
            wready: self.wready,
            bvalid: self.bvalid,
            bresp: 0,
            arready: self.arready,
            rvalid: self.rvalid,
            rresp: 0,
            rdata: match address {
                COMMAND_RX | WORK_RX => self.registers.fifo_output(address),
                _ => Ok(self.rdata),
            },
        }
    }

    pub fn interrupts(&self) -> u8 {
        self.registers.interrupts()
    }

    pub fn txd(&self) -> bool {
        self.registers.txd()
    }

    /// Clocks before the next required evaluation; this query changes no state.
    /// All bus input valid/ready strobes and reset must remain deasserted.
    /// RXD must stay at the supplied level until the returned span ends.
    /// Pending bus handshakes always require ordinary clocked evaluation.
    pub fn idle_limit(&self, rxd: bool) -> u64 {
        if self.awready || self.wready || self.bvalid || self.arready || self.rvalid {
            0
        } else {
            self.registers.idle_limit(rxd)
        }
    }

    /// Advance at most `idle_limit` clocks with constant RXD and no bus strobes.
    pub fn skip_idle(&mut self, clocks: u64, rxd: bool) -> u64 {
        self.registers
            .skip_idle(clocks.min(self.idle_limit(rxd)), rxd)
    }

    pub fn tick(&mut self, input: Input) -> Result<Output, Error> {
        if input.reset {
            self.registers.reset();
            self.awready = false;
            self.awaddr = 0;
            self.wready = false;
            self.bvalid = false;
            self.arready = false;
            self.araddr = u16::MAX;
            self.rvalid = false;
            self.rdata = 0;
            return Ok(Output {
                pins: self.pins(),
                cycle: None,
            });
        }
        let write = self.awready && self.wready && input.awvalid && input.wvalid;
        let read = self.arready && input.arvalid && !self.rvalid;
        let cycle = self.registers.cycle(
            input.rxd,
            read.then_some(self.araddr & 0x3ffc),
            write.then_some((self.awaddr & 0x3ffc, input.wdata, input.wstrb)),
        )?;
        if read {
            // FIFO data comes from the live mux, not this holding register.
            self.rdata = if matches!(self.araddr & 0x3ffc, COMMAND_RX | WORK_RX) {
                0
            } else {
                cycle
                    .read
                    .expect("read strobe present")
                    .expect("masked AXI address")
            };
        }
        if write && !self.bvalid {
            self.bvalid = true;
        } else if input.bready && self.bvalid {
            self.bvalid = false;
        }
        if read {
            self.rvalid = true;
        } else if self.rvalid && input.rready {
            self.rvalid = false;
        }
        // Source aw_en is initialized to1 and is never assigned0. Therefore
        // pending BVALID does not suppress a new address/data-ready pulse.
        let address_ready = !self.awready && input.awvalid && input.wvalid;
        if address_ready {
            self.awaddr = input.awaddr;
        }
        self.awready = address_ready;
        self.wready = !self.wready && input.wvalid && input.awvalid;
        // Source accepts ARADDR while RVALID is pending. Its output FIFO mux
        // can change RDATA under backpressure. Preserve and test this defect.
        let read_ready = !self.arready && input.arvalid;
        if read_ready {
            self.araddr = input.araddr;
        }
        self.arready = read_ready;
        Ok(Output {
            pins: self.pins(),
            cycle: Some(cycle),
        })
    }
}
