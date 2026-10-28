//! Private, fixed-size transport for one existing Icarus simulation.
//! This protocol does not establish CPU synchronization or physical timing.
use std::io;
#[cfg(test)]
use std::io::Read;

pub const FRAME_BYTES: usize = 128;
const REQUEST_MAGIC: &[u8; 8] = b"MDPLQ01\0";
const RESPONSE_MAGIC: &[u8; 8] = b"MDPLR01\0";
pub const DRIVE_CLOCKS: u64 = 0x0f;
pub const DRIVE_FABRIC_RESET: u64 = 0x10;
pub const DRIVE_AXI_RESET: u64 = 0x20;
pub const DRIVE_EXTERNAL_INPUTS: u64 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Op {
    Read32 = 1,
    Write32 = 2,
    Advance = 3,
    Drive = 4,
    Stop = 5,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl TryFrom<u64> for Op {
    type Error = io::Error;
    fn try_from(value: u64) -> io::Result<Self> {
        match value {
            1 => Ok(Self::Read32),
            2 => Ok(Self::Write32),
            3 => Ok(Self::Advance),
            4 => Ok(Self::Drive),
            5 => Ok(Self::Stop),
            _ => Err(invalid("unsupported live-PL operation")),
        }
    }
}

/// VPI vector convention: bval=1 marks an unknown; aval distinguishes X from Z.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Logic {
    pub aval: u32,
    pub bval: u32,
}

impl Logic {
    #[cfg(test)]
    pub fn pack(self) -> u64 {
        u64::from(self.aval) | (u64::from(self.bval) << 32)
    }
    pub fn unpack(value: u64, width: u8) -> io::Result<Self> {
        if !(1..=32).contains(&width) {
            return Err(invalid("invalid four-state width"));
        }
        let result = Self {
            aval: value as u32,
            bval: (value >> 32) as u32,
        };
        let mask = u32::MAX >> (32 - width);
        if (result.aval | result.bval) & !mask != 0 {
            return Err(invalid("four-state value exceeds declared width"));
        }
        Ok(result)
    }
    pub fn known(self) -> io::Result<u32> {
        if self.bval != 0 {
            Err(invalid("unknown/high-impedance result"))
        } else {
            Ok(self.aval)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub sequence: u64,
    pub at_ps: u64,
    pub operation: Op,
    pub args: [u64; 12],
}

impl Request {
    pub fn validate(&self) -> io::Result<()> {
        if self.sequence == 0 {
            return Err(invalid("zero command sequence"));
        }
        let used = match self.operation {
            Op::Read32 | Op::Write32 => {
                if self.args[0] > u64::from(u32::MAX) || self.args[0] & 3 != 0 {
                    return Err(invalid("only aligned 32-bit AXI accesses are supported"));
                }
                if self.operation == Op::Write32 {
                    if self.args[1] > u64::from(u32::MAX) || self.args[2] > 15 {
                        return Err(invalid("write data/strobe exceeds interface width"));
                    }
                    3
                } else {
                    1
                }
            }
            Op::Drive => {
                let selected = self.args[8];
                let allowed =
                    DRIVE_CLOCKS | DRIVE_FABRIC_RESET | DRIVE_AXI_RESET | DRIVE_EXTERNAL_INPUTS;
                if selected == 0 || selected & !allowed != 0 {
                    return Err(invalid("invalid batch-drive selection"));
                }
                // QEMU supplies a period in nanoseconds with32 fractional bits.
                // Zero gates a clock. This component rejects sub-nanosecond clocks.
                for index in 0..4 {
                    let value = self.args[index];
                    if selected & (1 << index) == 0 && value != 0 {
                        return Err(invalid("unselected clock contains data"));
                    }
                    if value != 0 && value < 1 << 32 {
                        return Err(invalid("sub-nanosecond clock unsupported"));
                    }
                }
                for (flag, value, bound) in [
                    (DRIVE_FABRIC_RESET, self.args[4], 15),
                    (DRIVE_AXI_RESET, self.args[5], 1),
                ] {
                    if value > bound || (selected & flag == 0 && value != 0) {
                        return Err(invalid("invalid or unselected reset value"));
                    }
                }
                Logic::unpack(self.args[6], 16)?;
                if self.args[7] > 0xffff
                    || (selected & DRIVE_EXTERNAL_INPUTS == 0 && (self.args[6] | self.args[7]) != 0)
                {
                    return Err(invalid("invalid or unselected external input value"));
                }
                // args6 is packed4-state input data. args7 is the changed-pin mask.
                9
            }
            Op::Advance | Op::Stop => 0,
        };
        if self.args[used..].iter().any(|value| *value != 0) {
            return Err(invalid("nonzero reserved command data"));
        }
        Ok(())
    }
    pub fn encode(&self) -> io::Result<[u8; FRAME_BYTES]> {
        self.validate()?;
        Ok(frame(
            REQUEST_MAGIC,
            self.sequence,
            self.at_ps,
            self.operation as u64,
            self.args,
        ))
    }
    #[cfg(test)]
    pub fn decode(bytes: &[u8; FRAME_BYTES]) -> io::Result<Self> {
        let (sequence, at_ps, operation, args) = unframe(REQUEST_MAGIC, bytes)?;
        let request = Self {
            sequence,
            at_ps,
            operation: operation.try_into()?,
            args,
        };
        request.validate()?;
        Ok(request)
    }
}

fn frame(
    magic: &[u8; 8],
    sequence: u64,
    time: u64,
    kind: u64,
    args: [u64; 12],
) -> [u8; FRAME_BYTES] {
    let mut output = [0; FRAME_BYTES];
    output[..8].copy_from_slice(magic);
    for (slot, value) in output[8..]
        .chunks_exact_mut(8)
        .zip([sequence, time, kind].into_iter().chain(args))
    {
        slot.copy_from_slice(&value.to_le_bytes());
    }
    output
}

fn unframe(magic: &[u8; 8], input: &[u8; FRAME_BYTES]) -> io::Result<(u64, u64, u64, [u64; 12])> {
    if input[..8] != magic[..] {
        return Err(invalid("live-PL frame version differs"));
    }
    let mut words = input[8..]
        .chunks_exact(8)
        .map(|word| u64::from_le_bytes(word.try_into().unwrap()));
    Ok((
        words.next().unwrap(),
        words.next().unwrap(),
        words.next().unwrap(),
        std::array::from_fn(|_| words.next().unwrap()),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Kind {
    Complete = 1,
    Irq = 2,
    Output = 3,
    Failure = 4,
    Ready = 5,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Completion args: value4state, AXI response4state, AXI ID, IRQ4state,
/// output4state, next known clock edge in ps, clock-edge-known flag, operation,
/// actual AXI clock cycles consumed, then three reserved zero words.
/// The known clock edge is not a claim about the HDL kernel's earliest event.
pub struct Response {
    pub sequence: u64,
    pub at_ps: u64,
    pub kind: Kind,
    pub args: [u64; 12],
}

impl Response {
    pub fn decode(bytes: &[u8; FRAME_BYTES]) -> io::Result<Self> {
        let (sequence, at_ps, kind, args) = unframe(RESPONSE_MAGIC, bytes)?;
        let kind = match kind {
            1 => Kind::Complete,
            2 => Kind::Irq,
            3 => Kind::Output,
            4 => Kind::Failure,
            5 => Kind::Ready,
            _ => return Err(invalid("unknown live-PL response kind")),
        };
        match kind {
            Kind::Complete => {
                Logic::unpack(args[0], 32)?;
                Logic::unpack(args[1], 2)?;
                if args[2] > 0xfff {
                    return Err(invalid("AXI ID width"));
                }
                Logic::unpack(args[3], 20)?;
                Logic::unpack(args[4], 19)?;
                if args[6] > 1
                    || (args[6] == 0 && args[5] != 0)
                    || (args[6] == 1 && args[5] <= at_ps)
                {
                    return Err(invalid("invalid next known clock boundary"));
                }
                let op = Op::try_from(args[7])?;
                if (matches!(op, Op::Read32 | Op::Write32) && !(1..=1024).contains(&args[8]))
                    || (!matches!(op, Op::Read32 | Op::Write32) && args[8] != 0)
                {
                    return Err(invalid("invalid reported AXI cycle count"));
                }
                if args[9..].iter().any(|v| *v != 0) {
                    return Err(invalid("reserved completion data"));
                }
            }
            Kind::Irq | Kind::Output => {
                Logic::unpack(args[0], if kind == Kind::Irq { 20 } else { 19 })?;
                if args[1..].iter().any(|v| *v != 0) {
                    return Err(invalid("reserved event data"));
                }
            }
            Kind::Ready => {
                Logic::unpack(args[3], 20)?;
                Logic::unpack(args[4], 19)?;
                if sequence != 0
                    || at_ps != 0
                    || args
                        .iter()
                        .enumerate()
                        .any(|(i, v)| i != 3 && i != 4 && *v != 0)
                {
                    return Err(invalid("invalid transport-ready receipt"));
                }
            }
            Kind::Failure => {
                if args[0] == 0 || args[0] > 16 || args[1..].iter().any(|v| *v != 0) {
                    return Err(invalid("invalid failure receipt"));
                }
            }
        }
        Ok(Self {
            sequence,
            at_ps,
            kind,
            args,
        })
    }
}

/// A complete command must finish before another command can be admitted.
/// This is local PL ordering, not permission for a CPU to advance past an IRQ.
#[derive(Default)]
pub struct Order {
    sequence: u64,
    ready: bool,
    observed_ps: u64,
    pending: Option<Request>,
    stopped: bool,
    failed: bool,
}

impl Order {
    pub fn admit(&mut self, request: &Request) -> io::Result<()> {
        request.validate()?;
        if !self.ready
            || self.failed
            || self.stopped
            || self.pending.is_some()
            || self.sequence.checked_add(1) != Some(request.sequence)
            || request.at_ps <= self.observed_ps
        {
            return Err(invalid("live-PL command is not the next future operation"));
        }
        self.sequence = request.sequence;
        self.pending = Some(request.clone());
        Ok(())
    }
    pub fn observe(&mut self, response: &Response) -> io::Result<bool> {
        let checked = (|| {
            // Revalidate typed values too: public construction cannot bypass wire checks.
            Response::decode(&frame(
                RESPONSE_MAGIC,
                response.sequence,
                response.at_ps,
                response.kind as u64,
                response.args,
            ))?;
            if self.failed || self.stopped || response.at_ps < self.observed_ps {
                return Err(invalid("live-PL time/order violation"));
            }
            if !self.ready {
                if response.sequence != 0
                    || response.at_ps != 0
                    || !matches!(response.kind, Kind::Ready | Kind::Irq | Kind::Output)
                {
                    return Err(invalid("invalid startup observation"));
                }
                self.ready = response.kind == Kind::Ready;
                return Ok(false);
            }
            let request = self
                .pending
                .as_ref()
                .ok_or_else(|| invalid("response without a pending command"))?;
            if response.kind == Kind::Ready || response.sequence != request.sequence {
                return Err(invalid("response identity or admission time differs"));
            }
            if response.kind == Kind::Failure {
                return Err(invalid("actual PL execution failed"));
            }
            if response.kind == Kind::Complete {
                if response.at_ps < request.at_ps {
                    return Err(invalid("completion predates requested input"));
                }
                if response.args[7] != request.operation as u64 {
                    return Err(invalid("completion operation differs"));
                }
                if matches!(request.operation, Op::Read32 | Op::Write32) {
                    Logic::unpack(response.args[0], 32)?.known()?;
                    if Logic::unpack(response.args[1], 2)?.known()? != 0 {
                        return Err(invalid("AXI error response"));
                    }
                    if response.args[2] != request.sequence & 0xfff {
                        return Err(invalid("AXI response ID differs"));
                    }
                }
            }
            Ok(response.kind == Kind::Complete)
        })();
        match checked {
            Ok(done) => {
                self.observed_ps = response.at_ps;
                if done {
                    self.stopped = self.pending.take().unwrap().operation == Op::Stop;
                }
                Ok(done)
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
}

/// Transport is private and exact-size. Caller owns the deadline and process.
#[cfg(test)]
pub fn read_frame(reader: &mut impl Read) -> io::Result<[u8; FRAME_BYTES]> {
    let mut bytes = [0; FRAME_BYTES];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Convert the accumulated Q33 half-edge position to the1ps Icarus lattice.
/// Round the absolute edge up; never round each period and accumulate drift.
#[cfg(test)]
pub fn edge_ps(position_q33: u128) -> io::Result<u64> {
    let scaled = position_q33
        .checked_mul(1000)
        .and_then(|v| v.checked_add((1 << 33) - 1))
        .ok_or_else(|| invalid("clock edge overflow"))?;
    u64::try_from(scaled >> 33).map_err(|_| invalid("clock edge exceeds timestamp range"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(sequence: u64, at_ps: u64, operation: Op) -> Request {
        Request {
            sequence,
            at_ps,
            operation,
            args: [0; 12],
        }
    }
    fn order() -> Order {
        let mut o = Order::default();
        assert!(!o
            .observe(&Response {
                sequence: 0,
                at_ps: 0,
                kind: Kind::Ready,
                args: [0; 12]
            })
            .unwrap());
        o
    }
    fn complete(r: &Request, at_ps: u64) -> Response {
        let mut args = [0; 12];
        args[2] = r.sequence & 0xfff;
        args[7] = r.operation as u64;
        args[8] = if matches!(r.operation, Op::Read32 | Op::Write32) {
            7
        } else {
            0
        };
        Response::decode(&frame(
            RESPONSE_MAGIC,
            r.sequence,
            at_ps,
            Kind::Complete as u64,
            args,
        ))
        .unwrap()
    }
    #[test]
    fn literal_wire_layout_and_reserved_values() {
        let mut r = request(7, 0x1122334455667788, Op::Write32);
        r.args[..3].copy_from_slice(&[0x43c00010, 0x11223344, 3]);
        let b = r.encode().unwrap();
        assert_eq!(
            &b[..32],
            b"MDPLQ01\0\x07\0\0\0\0\0\0\0\x88\x77\x66\x55\x44\x33\x22\x11\x02\0\0\0\0\0\0\0"
        );
        assert_eq!(
            &b[32..56],
            &[
                0x10, 0, 0xc0, 0x43, 0, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 3, 0, 0, 0, 0,
                0, 0, 0
            ]
        );
        assert_eq!(Request::decode(&b).unwrap(), r);
        for offset in 56..128 {
            let mut changed = b;
            changed[offset] = 1;
            assert!(Request::decode(&changed).is_err());
        }
        for address in [1, 2, 3, 0x1_0000_0000] {
            r.args[0] = address;
            assert!(r.encode().is_err());
        }
    }
    #[test]
    fn four_state_preserves_x_and_z() {
        for (packed, expected) in [
            (0, (0, 0)),
            (1, (1, 0)),
            (1 << 32, (0, 1)),
            ((1 << 32) | 1, (1, 1)),
        ] {
            let v = Logic::unpack(packed, 1).unwrap();
            assert_eq!((v.aval, v.bval), expected);
            assert_eq!(v.pack(), packed);
            assert_eq!(v.known().is_ok(), expected.1 == 0);
        }
        assert!(Logic::unpack(2, 1).is_err());
        assert!(Logic::unpack(2 << 32, 1).is_err());
    }
    #[test]
    fn later_real_completion_rejects_time_reversal_or_duplicate() {
        let r = request(1, 10, Op::Read32);
        let mut o = order();
        o.admit(&r).unwrap();
        assert!(!o
            .observe(&Response {
                sequence: 1,
                at_ps: 5,
                kind: Kind::Irq,
                args: [0; 12]
            })
            .unwrap());
        assert!(o.observe(&complete(&r, 50)).unwrap());
        assert!(o.admit(&request(2, 50, Op::Advance)).is_err());
        assert!(o.admit(&request(2, 49, Op::Advance)).is_err());
        o.admit(&request(2, 51, Op::Advance)).unwrap();
        assert!(o.observe(&complete(&r, 52)).is_err());
        assert!(o.admit(&request(3, 60, Op::Advance)).is_err());
    }
    #[test]
    fn error_and_unknown_reads_latch_failure() {
        for (data, response) in [(1 << 32, 0), (0, 1), (0, 1 << 32)] {
            let r = request(1, 10, Op::Read32);
            let mut o = order();
            o.admit(&r).unwrap();
            let mut c = complete(&r, 20);
            c.args[0] = data;
            c.args[1] = response;
            assert!(o.observe(&c).is_err());
            assert!(o.observe(&complete(&r, 21)).is_err());
        }
    }
    #[test]
    fn fragments_and_every_truncated_frame_are_checked() {
        let r = request(1, 10, Op::Advance);
        let bytes = r.encode().unwrap();
        for size in 0..FRAME_BYTES {
            assert!(read_frame(&mut &bytes[..size]).is_err());
        }
        struct Fragment<'a>(&'a [u8]);
        impl Read for Fragment<'_> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                let n = out.len().min(3).min(self.0.len());
                out[..n].copy_from_slice(&self.0[..n]);
                self.0 = &self.0[n..];
                Ok(n)
            }
        }
        assert_eq!(read_frame(&mut Fragment(&bytes)).unwrap(), bytes);
    }
    #[test]
    fn absolute_edge_rounding_has_no_per_period_drift() {
        let period_q32 = (1u128 << 32) / 3 * 10;
        for edge in 1..10000u128 {
            let q = edge * period_q32;
            let ps = edge_ps(q).unwrap() as u128;
            assert!(ps * (1 << 33) >= q * 1000);
            assert!(ps == 0 || (ps - 1) * (1 << 33) < q * 1000);
        }
        assert!(edge_ps(u128::MAX).is_err());
    }
}
