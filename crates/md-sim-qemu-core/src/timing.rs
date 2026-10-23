//! Physical ASIC time is independent of the controller's fabric clock.
//! Core edges use Q32 nanoseconds. Serial peer events use integer nanoseconds;
//! a core edge is observed at its ceiling. Peers sample old levels on a tie.
use crate::{fail, S9Event, S9State};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct S9Clock {
    pub edge_ns: u64,
    pub period_q32: u64,
    pub fraction: u32,
}

impl S9Clock {
    fn start(&self) -> u128 {
        (u128::from(self.edge_ns) << 32) | u128::from(self.fraction)
    }
}

fn ceil_ns(value: u128) -> u64 {
    value.div_ceil(1 << 32) as u64
}

pub(crate) fn edge_ns(state: &S9State) -> u64 {
    ceil_ns(state.elapsed_q32)
}

fn peer_idle(state: &S9State, edge: u128, period: u64) -> u64 {
    state
        .peer
        .as_ref()
        .and_then(|p| p.next_deadline())
        .map_or(u64::MAX, |at| {
            // Safe skips end strictly before the peer's next event. Unlike
            // integer division by a nominal frequency, this retains phase.
            let bound = u128::from(at.saturating_sub(1)) << 32;
            (bound.saturating_sub(edge) / u128::from(period)).min(u128::from(u64::MAX)) as u64
        })
}

/// Read the idle bound at the supplied phase and period. A gated clock is
/// scheduled with peer_deadline instead. Sub-nanosecond edges are unsupported.
/// # Safety
/// state and clock are live, aligned, exclusively borrowed device objects.
#[no_mangle]
pub unsafe extern "C" fn md_s9_idle_limit_timed(
    state: *const S9State,
    clock: *const S9Clock,
    rxd: bool,
) -> u64 {
    let (state, clock) = (&*state, &*clock);
    if state.failed || clock.period_q32 < 1 << 32 || clock.start() < state.elapsed_q32 {
        return 0;
    }
    let rxd = state.peer.as_ref().map_or(rxd, |p| p.rxd());
    state
        .registers
        .idle_limit(rxd)
        .min(peer_idle(state, clock.start(), clock.period_q32))
}

/// Same event contract as md_s9_advance, with an explicit physical clock.
/// # Safety
/// state, clock and out are live, aligned, non-overlapping device objects.
#[no_mangle]
pub unsafe extern "C" fn md_s9_advance_timed(
    state: *mut S9State,
    clock: *const S9Clock,
    clocks: u64,
    rxd: bool,
    out: *mut S9Event,
) -> bool {
    advance(&mut *state, &*clock, clocks, rxd, &mut *out)
}

pub(crate) fn advance(
    state: &mut S9State,
    clock: &S9Clock,
    clocks: u64,
    rxd: bool,
    out: &mut S9Event,
) -> bool {
    *out = S9Event {
        irq: state.registers.interrupts(),
        txd: u8::from(state.registers.txd()),
        ..S9Event::default()
    };
    if state.failed {
        return false;
    }
    let start = clock.start();
    let end = u128::from(clocks)
        .checked_mul(u128::from(clock.period_q32))
        .and_then(|d| start.checked_add(d));
    if clock.period_q32 < 1 << 32
        || start < state.elapsed_q32
        || end.is_none_or(|n| n > u128::from(u64::MAX) << 32)
    {
        return fail(state, "invalid S9IO clock span or timestamp overflow");
    }
    state.elapsed_q32 = start;
    for _ in 0..4096 {
        let input = state.peer.as_ref().map_or(rxd, |p| p.rxd());
        let limit = peer_idle(state, state.elapsed_q32, clock.period_q32);
        let skipped = state
            .registers
            .skip_idle((clocks - out.clocks).min(limit), input);
        state.elapsed_q32 += u128::from(skipped) * u128::from(clock.period_q32);
        out.clocks += skipped;
        if out.clocks == clocks {
            break;
        }
        let next = state.elapsed_q32 + u128::from(clock.period_q32);
        let next_ns = ceil_ns(next);
        if let Some(peer) = &mut state.peer {
            // Processing a physical event before a retroactive core edge
            // would change tie ordering. Reject such a caller error.
            if next_ns <= peer.now_ns() {
                return fail(state, "core edge is not after processed ASIC time");
            }
            if let Err(e) = peer.advance(next_ns - 1) {
                return fail(state, e);
            }
        }
        let input = state.peer.as_ref().map_or(rxd, |p| p.rxd());
        let prior_errors = state.registers.uart_errors();
        let Ok(event) = state.registers.tick(input) else {
            return fail(state, "unsupported S9IO clocked transition");
        };
        state.elapsed_q32 = next;
        out.clocks += 1;
        if let Some(peer) = &mut state.peer {
            if let Err(e) = peer.input(next_ns, state.registers.txd()) {
                return fail(state, e);
            }
        }
        let irq = state.registers.interrupts();
        let txd = u8::from(state.registers.txd());
        out.flags = u32::from(event.uart.started.is_some())
            | (u32::from(event.uart.received.is_some()) << 1)
            | (u32::from(event.core.rx.checksum_error) << 2)
            | (u32::from(irq != out.irq || txd != out.txd) << 3)
            | (u32::from(state.registers.uart_errors() != prior_errors) << 4)
            | (u32::from(state.peer.as_ref().is_some_and(|p| p.pending())) << 5);
        out.tx_byte = event.uart.started.unwrap_or(0);
        out.rx_byte = event.uart.received.unwrap_or(0);
        out.irq = irq;
        out.txd = txd;
        if out.flags != 0 {
            break;
        }
    }
    true
}

/// Return the next ASIC event in nanoseconds since reset, or UINT64_MAX.
/// # Safety
/// state is a live exclusively borrowed device pointer.
#[no_mangle]
pub unsafe extern "C" fn md_s9_peer_deadline(state: *const S9State) -> u64 {
    (*state)
        .peer
        .as_ref()
        .and_then(|p| p.next_deadline())
        .unwrap_or(u64::MAX)
}

/// Advance only the peer, through at most its next event. The caller drains
/// traces and repeats until reached == until_ns. No fabric edge is created.
/// # Safety
/// state is live/exclusive; reached is valid writable aligned storage.
#[no_mangle]
pub unsafe extern "C" fn md_s9_peer_advance(
    state: *mut S9State,
    until_ns: u64,
    reached: *mut u64,
) -> bool {
    let state = &mut *state;
    if state.failed {
        return false;
    }
    if until_ns < edge_ns(state) {
        return fail(state, "ASIC time precedes core edge");
    }
    let Some(peer) = &mut state.peer else {
        *reached = until_ns;
        return true;
    };
    let at = until_ns.min(peer.next_deadline().unwrap_or(until_ns));
    match peer.advance(at) {
        Ok(()) => {
            *reached = at;
            true
        }
        Err(e) => fail(state, e),
    }
}

/// Timestamped power/reset pin update for a variable or stopped fabric clock.
/// # Safety
/// state is live and exclusively borrowed by the device thread.
#[no_mangle]
pub unsafe extern "C" fn md_s9_asic_pin_at(
    state: *mut S9State,
    pin: u32,
    level: bool,
    ns: u64,
) -> bool {
    let state = &mut *state;
    if state.failed {
        return false;
    }
    if ns < edge_ns(state) {
        return fail(state, "ASIC pin precedes core edge");
    }
    let Some(peer) = &mut state.peer else {
        return fail(state, "ASIC pin without peer");
    };
    match peer.pin(ns, pin, level) {
        Ok(()) => true,
        Err(e) => fail(state, e),
    }
}
