//! A semantic view of an already received core-register-0 word.
//! Field extraction follows the exact archived T17 formatter at0x238f0,
//! SHA-256 8d3b1b4e80ef91084c23018fc6055b6bd5249b0e0f91119919f2fb20ef0f8245.
//! This view does not complete a mailbox operation or generate a reply.
//! The caller must supply its pending read and expected ASIC address. The
//! reported core field is preserved separately from the encoded selector:
//! their mapping to physical cores remains unresolved.
use super::{core_control::Control, Reply};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Request,
    Reply,
    Address,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Core0Readback {
    pub raw: u32,
    pub asic_address: u8,
    pub requested_selector: u16,
    pub reported_core: u16,
    pub data: u8,
    pub ccdly_sel: u8,
    pub pwth_sel: u8,
    /// Raw named bits, not a claim that a physical clock or mode is active.
    pub hash_clken_bit: bool,
    pub mmen_bit: bool,
    pub reserved_bit: bool,
    pub swpf_mode_bit: bool,
}

impl Core0Readback {
    pub fn from_reply(expected_address: u8, request: Control, reply: Reply) -> Result<Self, Error> {
        request.encode().map_err(|_| Error::Request)?;
        let Control::Read {
            selector,
            register: 0,
        } = request
        else {
            return Err(Error::Request);
        };
        let Reply::Register {
            value,
            asic_address,
            register: 0x40,
        } = reply
        else {
            return Err(Error::Reply);
        };
        if asic_address != expected_address {
            return Err(Error::Address);
        }
        let data = value as u8;
        Ok(Self {
            raw: value,
            asic_address,
            requested_selector: selector,
            reported_core: (value >> 16) as u16,
            data,
            ccdly_sel: data >> 6,
            pwth_sel: (data >> 4) & 3,
            hash_clken_bit: data & 8 != 0,
            mmen_bit: data & 4 != 0,
            reserved_bit: data & 2 != 0,
            swpf_mode_bit: data & 1 != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read() -> Control {
        Control::Read {
            selector: 1,
            register: 0,
        }
    }

    fn reply(raw: u32) -> Reply {
        Reply::Register {
            value: raw,
            asic_address: 12,
            register: 0x40,
        }
    }

    #[test]
    fn retained_vendor_vectors_preserve_raw_fields_and_reserved_bits() {
        // Fixed predictions recorded in analysis442. The independent original
        // formatter oracle checks these fields across5192 execution cases.
        for (raw, core, data, fields) in [
            (0, 0, 0, [0, 0, 0, 0, 0, 0]),
            (0x74, 0, 0x74, [1, 3, 0, 1, 0, 0]),
            (0x01230074, 0x123, 0x74, [1, 3, 0, 1, 0, 0]),
            (0xabcdff74, 0xabcd, 0x74, [1, 3, 0, 1, 0, 0]),
            (0xffffffff, 0xffff, 0xff, [3, 3, 1, 1, 1, 1]),
            (0x10008, 1, 8, [0, 0, 1, 0, 0, 0]),
            (0x10004, 1, 4, [0, 0, 0, 1, 0, 0]),
        ] {
            let view = Core0Readback::from_reply(12, read(), reply(raw)).unwrap();
            assert_eq!((view.raw, view.reported_core, view.data), (raw, core, data));
            assert_eq!(view.requested_selector, 1);
            assert_eq!(view.asic_address, 12);
            assert_eq!(
                [
                    view.ccdly_sel,
                    view.pwth_sel,
                    u8::from(view.hash_clken_bit),
                    u8::from(view.mmen_bit),
                    u8::from(view.reserved_bit),
                    u8::from(view.swpf_mode_bit)
                ],
                fields
            );
        }
    }

    #[test]
    fn reply_requires_matching_address_and_pending_core0_read() {
        assert_eq!(
            Core0Readback::from_reply(8, read(), reply(0x74)),
            Err(Error::Address)
        );
        for request in [
            Control::Write {
                register: 0,
                value: 0x74,
            },
            Control::Read {
                selector: 0,
                register: 1,
            },
            Control::Read {
                selector: 512,
                register: 0,
            },
        ] {
            assert_eq!(
                Core0Readback::from_reply(12, request, reply(0x74)),
                Err(Error::Request)
            );
        }
        assert_eq!(
            Core0Readback::from_reply(
                12,
                read(),
                Reply::Register {
                    value: 0x74,
                    asic_address: 12,
                    register: 0x3c,
                }
            ),
            Err(Error::Reply)
        );
        assert_eq!(
            Core0Readback::from_reply(
                12,
                read(),
                Reply::Job {
                    nonce: 0,
                    midstate_tag: 0,
                    job_id: 0,
                    midstate_index: 0,
                }
            ),
            Err(Error::Reply)
        );
    }
}
