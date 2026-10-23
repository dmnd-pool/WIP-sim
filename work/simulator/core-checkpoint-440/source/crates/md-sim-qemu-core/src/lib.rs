//! QEMU C ABI. State machines remain in the tested Rust components.
//! MMIO is an atomic transaction at the caller's virtual timestamp. It does
//! not model AXI channel stalls or advance device clocks on a register poll.
pub use md_sim_axi_iic as iic;
mod fan;
mod gpio;
pub mod pcap;
mod pic16;
mod s9_chain;
mod timing;
pub use timing::{
    md_s9_advance_timed, md_s9_asic_pin_at, md_s9_idle_limit_timed, md_s9_peer_advance,
    md_s9_peer_deadline, S9Clock,
};
pub mod swdt;
use md_sim_firmware::bm1387::budget::{Budget, Shared};
use md_sim_firmware::bm1387::chain::Profile;
use md_sim_firmware::s9io::registers::{BusError, Registers, Revision};
pub use s9_chain::AsicTrace;
use std::ffi::{c_char, CString};

pub struct HashBudget(Shared);

/// Create one budget per run, shared by every mining controller.
/// The default selected by the board is10000 H/s. Lower caps are valid.
/// A null result means the rate cannot be represented.
#[no_mangle]
pub extern "C" fn md_hash_budget_new(hps: f64) -> *mut HashBudget {
    Budget::new(hps)
        .map(|budget| Box::into_raw(Box::new(HashBudget(budget))))
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// The pointer is live and owned, or null. Each controller retains its own
/// shared reference, so the creating board may release this after realize.
#[no_mangle]
pub unsafe extern "C" fn md_hash_budget_free(budget: *mut HashBudget) {
    if !budget.is_null() {
        drop(Box::from_raw(budget));
    }
}

pub struct S9State {
    registers: Registers,
    failed: bool,
    elapsed_q32: u128,
    peer: Option<s9_chain::Peer>,
    error: CString,
}

#[derive(Default)]
#[repr(C)]
pub struct S9Event {
    pub clocks: u64,
    pub flags: u32,
    pub tx_byte: u8,
    pub rx_byte: u8,
    pub irq: u8,
    pub txd: u8,
}

#[derive(Default)]
#[repr(C)]
pub struct S9Idle {
    pub receive: u64,
    pub transmit: u64,
    pub uart: u64,
    pub asic: u64,
}

/// # Safety
/// The returned pointer must be passed to `md_s9_free` exactly once. All
/// operations require exclusive access to this state on the device thread.
#[no_mangle]
pub extern "C" fn md_s9_new(build_id: u32) -> *mut S9State {
    new_state(build_id, Revision::Legacy0_2)
}

/// # Safety
/// Same ownership rules as `md_s9_new`. This selects the declared reference
/// contract; it does not certify correspondence with a physical bitstream.
#[no_mangle]
pub extern "C" fn md_s9_new_reference_v1(build_id: u32) -> *mut S9State {
    new_state(build_id, Revision::Reference1_0_2)
}

fn new_state(build_id: u32, revision: Revision) -> *mut S9State {
    Box::into_raw(Box::new(S9State {
        registers: Registers::with_revision(build_id, revision),
        failed: false,
        elapsed_q32: 0,
        peer: None,
        error: CString::default(),
    }))
}

/// Explicit BM1387 reference profile, not resolved physical board evidence.
/// A null result means the supplied topology or clock is unsupported.
/// # Safety
/// Successful ownership is the same as md_s9_new.
#[no_mangle]
pub extern "C" fn md_s9_new_bm1387(
    build_id: u32,
    chips: u16,
    cores: u16,
    oscillator_hz: u32,
    baud_divisor: u8,
    identity_reserved: u8,
    reply_byte_capacity: u32,
) -> *mut S9State {
    let profile = Profile {
        chips,
        cores_per_chip: cores,
        oscillator_hz,
        initial_baud_divisor: baud_divisor,
        identity_reserved,
    };
    let Ok(peer) = s9_chain::Peer::new(profile, reply_byte_capacity as usize) else {
        return std::ptr::null_mut();
    };
    let state = new_state(build_id, Revision::Reference1_0_2);
    unsafe { (*state).peer = Some(peer) };
    state
}

/// Source-derived BM1387 fixture with its explicit non-consecutive map.
/// This is not a claim about an exact physical S9 board revision.
/// # Safety
/// budget is a live shared budget pointer. It may be released after this
/// call. A successful state has the same ownership rules as md_s9_new.
#[no_mangle]
pub unsafe extern "C" fn md_s9_new_bm1387_reference_mining(
    build_id: u32,
    budget: *const HashBudget,
    reply_byte_capacity: u32,
    fixed_filter_zero_bits: u32,
) -> *mut S9State {
    if budget.is_null() || !(1..=32).contains(&fixed_filter_zero_bits) {
        return std::ptr::null_mut();
    }
    let Ok(slot) = Budget::attach(&(*budget).0) else {
        return std::ptr::null_mut();
    };
    let policy =
        md_sim_firmware::bm1387::filter::Policy::from_fixed_bits(fixed_filter_zero_bits as u8)
            .expect("validated filter bit count");
    let Ok(peer) = s9_chain::Peer::new_reference_mining(reply_byte_capacity as usize, slot, policy)
    else {
        return std::ptr::null_mut();
    };
    let state = new_state(build_id, Revision::Reference1_0_2);
    (*state).peer = Some(peer);
    state
}

fn fail(state: &mut S9State, error: impl Into<String>) -> bool {
    state.failed = true;
    state.error = CString::new(error.into()).expect("model error text has no NUL");
    false
}

/// Attach the separately declared temperature fixture before first power.
/// # Safety
/// state is a live, exclusive device pointer created by this library.
#[no_mangle]
pub unsafe extern "C" fn md_s9_attach_tmp421_reference(state: *mut S9State) -> bool {
    let state = &mut *state;
    let Some(peer) = state.peer.as_mut() else {
        return fail(state, "temperature fixture requires a BM1387 peer");
    };
    match peer.attach_reference_temperature() {
        Ok(()) => true,
        Err(error) => fail(state, error),
    }
}

/// Install a bounded frozen external-temperature scenario before first power.
/// # Safety
/// state is live and exclusive. For nonzero length, bytes points to at least
/// length readable bytes. No pointer is retained after this call.
#[no_mangle]
pub unsafe extern "C" fn md_s9_temperature_input_plan(
    state: *mut S9State,
    bytes: *const u8,
    length: usize,
) -> bool {
    let state = &mut *state;
    if bytes.is_null() || length == 0 || length > 4096 {
        return fail(state, "temperature input plan size or pointer invalid");
    }
    let Some(peer) = state.peer.as_mut() else {
        return fail(state, "temperature input plan requires a BM1387 peer");
    };
    match peer.install_temperature_plan(std::slice::from_raw_parts(bytes, length)) {
        Ok(()) => true,
        Err(error) => fail(state, error),
    }
}

/// # Safety
/// state is a live exclusive device pointer. The returned string is borrowed
/// until the next mutable operation; callers must not free it.
#[no_mangle]
pub unsafe extern "C" fn md_s9_error(state: *const S9State) -> *const c_char {
    (*state).error.as_ptr()
}

/// Apply power(pin0) or active-low reset(pin1, electrical level).
/// phase_ns is the elapsed fraction after the last evaluated core edge.
/// # Safety
/// state is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_s9_asic_pin(
    state: *mut S9State,
    pin: u32,
    level: bool,
    phase_ns: u32,
) -> bool {
    let state = &mut *state;
    if state.failed {
        return false;
    }
    if phase_ns >= state.registers.core_period_ns() {
        return fail(state, "ASIC pin phase outside core period");
    }
    let Some(ns) = timing::edge_ns(state).checked_add(u64::from(phase_ns)) else {
        return fail(state, "ASIC pin timestamp overflow");
    };
    let Some(peer) = state.peer.as_mut() else {
        return fail(state, "ASIC pin on a controller without a peer");
    };
    match peer.pin(ns, pin, level) {
        Ok(()) => true,
        Err(e) => fail(state, e),
    }
}

/// # Safety
/// state is live/exclusive. out is a valid writable aligned object.
#[no_mangle]
pub unsafe extern "C" fn md_s9_take_asic_trace(state: *mut S9State, out: *mut AsicTrace) -> bool {
    if let Some(event) = (*state).peer.as_mut().and_then(|p| p.take_trace()) {
        *out = event;
        true
    } else {
        false
    }
}

/// # Safety
/// `state` is a live pointer owned by this device thread.
#[no_mangle]
pub unsafe extern "C" fn md_s9_core_period_ns(state: *const S9State) -> u32 {
    (*state).registers.core_period_ns()
}

/// # Safety
/// `state` is a live unique pointer from `md_s9_new`, or null.
#[no_mangle]
pub unsafe extern "C" fn md_s9_free(state: *mut S9State) {
    if !state.is_null() {
        drop(Box::from_raw(state));
    }
}

/// # Safety
/// `state` is a live exclusively borrowed pointer from `md_s9_new`.
#[no_mangle]
pub unsafe extern "C" fn md_s9_reset(state: *mut S9State) {
    let state = &mut *state;
    state.registers.reset();
    state.failed = false;
    state.elapsed_q32 = 0;
    state.error = CString::default();
    if let Some(peer) = &mut state.peer {
        peer.reset();
    }
}

fn address(offset: u32, size: u32) -> Option<(u16, u32, u32)> {
    if offset >= 0x10000 || !matches!(size, 1 | 2 | 4) || offset % size != 0 {
        return None;
    }
    let shift = (offset & 3) * 8;
    let mask = u32::MAX >> (32 - size * 8);
    Some(((offset & 0x3ffc) as u16, shift, mask))
}

/// Return0 on success,1 for invalid access,2 for uninitialized FIFO data.
/// On error the caller's output remains unchanged.
///
/// # Safety
/// `state` is live and exclusively borrowed; `value` is writable and aligned.
#[no_mangle]
pub unsafe extern "C" fn md_s9_read(
    state: *mut S9State,
    offset: u32,
    size: u32,
    value: *mut u32,
) -> u32 {
    let Some((offset, shift, mask)) = address(offset, size) else {
        return 1;
    };
    match (*state).registers.read(offset) {
        Ok(word) => {
            *value = (word >> shift) & mask;
            0
        }
        Err(BusError::UninitializedRead) => 2,
        Err(BusError::OutsideWindow) => 1,
        Err(BusError::UnsupportedTransition) => 3,
    }
}

/// Return false for an invalid access. Hardware full-FIFO drops still
/// complete normally, as do writes to unimplemented register offsets.
///
/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_s9_write(
    state: *mut S9State,
    offset: u32,
    size: u32,
    value: u32,
) -> bool {
    let Some((offset, shift, mask)) = address(offset, size) else {
        return false;
    };
    let result = (*state).registers.write(
        offset,
        (value & mask) << shift,
        (((1 << size) - 1) << (shift / 8)) as u8,
    );
    if result == Err(BusError::UnsupportedTransition) {
        fail(&mut *state, "unsupported S9IO register transition");
    }
    result.is_ok()
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_s9_idle_limit(state: *const S9State, rxd: bool) -> u64 {
    if (*state).failed {
        0
    } else {
        let state = &*state;
        let rxd = state.peer.as_ref().map_or(rxd, |peer| peer.rxd());
        let idle = state.registers.idle_limit(rxd);
        state.peer.as_ref().map_or(idle, |peer| {
            idle.min(peer.idle_limit(
                timing::edge_ns(state),
                u64::from(state.registers.core_period_ns()),
            ))
        })
    }
}

/// Read-only component bounds. This does not advance clocks or read a FIFO.
///
/// # Safety
/// `state` is live and exclusively borrowed; `out` is writable and aligned.
#[no_mangle]
pub unsafe extern "C" fn md_s9_idle_details(state: *const S9State, rxd: bool, out: *mut S9Idle) {
    let state = &*state;
    let rxd = state.peer.as_ref().map_or(rxd, |peer| peer.rxd());
    let limits = state.registers.idle_limits(rxd);
    *out = S9Idle {
        receive: limits.receive,
        transmit: limits.transmit,
        uart: limits.uart,
        asic: state.peer.as_ref().map_or(u64::MAX, |peer| {
            peer.idle_limit(
                timing::edge_ns(state),
                u64::from(state.registers.core_period_ns()),
            )
        }),
    };
}

/// IRQ bits0..2; TXD bit3; framing error bit4; overrun bit5; failure bit6.
///
/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_s9_status(state: *const S9State) -> u32 {
    let state = &*state;
    let (frame, overrun) = state.registers.uart_errors();
    u32::from(state.registers.interrupts())
        | (u32::from(state.registers.txd()) << 3)
        | (u32::from(frame) << 4)
        | (u32::from(overrun) << 5)
        | (u32::from(state.failed) << 6)
}

/// Advance to the first pin/IRQ/serial/error event or the requested span.
/// Work per call is bounded to4096 evaluated edges, plus idle skips.
/// flags: bit0TX byte started,1RX byte received,2checksum error,3pin/IRQ
/// changed,4UART error changed,5ASIC trace available.
/// A model error latches failure and returns false.
///
/// # Safety
/// `state` is live and exclusively borrowed. `out` is writable and aligned.
#[no_mangle]
pub unsafe extern "C" fn md_s9_advance(
    state: *mut S9State,
    clocks: u64,
    rxd: bool,
    out: *mut S9Event,
) -> bool {
    let state = &mut *state;
    let clock = S9Clock {
        edge_ns: (state.elapsed_q32 >> 32) as u64,
        period_q32: u64::from(state.registers.core_period_ns()) << 32,
        fraction: state.elapsed_q32 as u32,
    };
    timing::advance(state, &clock, clocks, rxd, &mut *out)
}
