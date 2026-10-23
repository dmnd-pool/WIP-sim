//! Owned PIC protocol state. This ABI does not execute PIC instructions.
use md_sim_firmware::pic16::{Error, Pic, Program, APP_HASH, APP_WORDS};

fn status(result: Result<(), Error>) -> u32 {
    match result {
        Ok(()) => 0,
        Err(Error::Busy | Error::NoReply) => 1,
        Err(Error::ProgramFormat) => 2,
        Err(Error::Time) => 3,
        Err(Error::Mode) => 4,
        Err(Error::FlashRange) => 5,
        Err(Error::UnknownApplication) => 6,
        Err(Error::UnknownPowerOnVoltage) => 7,
        Err(Error::UnsupportedCommand(command)) => 0x100 | u32::from(command),
    }
}

/// # Safety
/// `bytes` points to `length` readable bytes. The returned pointer is owned
/// by the caller and requires exclusive access until freed.
#[no_mangle]
pub unsafe extern "C" fn md_pic_new(bytes: *const u8, length: u32) -> *mut Pic {
    if bytes.is_null() || length as usize > APP_WORDS * 6 {
        return std::ptr::null_mut();
    }
    let bytes = std::slice::from_raw_parts(bytes, length as usize);
    let Ok(text) = std::str::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    let Ok(program) = Program::parse(text) else {
        return std::ptr::null_mut();
    };
    if program.sha256() != APP_HASH {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(Pic::with_erased_development_storage(&program)))
}

/// # Safety
/// `state` is an owned pointer from md_pic_new, or null.
#[no_mangle]
pub unsafe extern "C" fn md_pic_free(state: *mut Pic) {
    if !state.is_null() {
        drop(Box::from_raw(state));
    }
}

/// # Safety
/// `state` is live and exclusively borrowed. `now` is absolute virtual time.
#[no_mangle]
pub unsafe extern "C" fn md_pic_advance(state: *mut Pic, now: u64) -> u32 {
    status((*state).advance(now))
}

/// # Safety
/// `state` is live and exclusively borrowed. `now` is absolute virtual time.
#[no_mangle]
pub unsafe extern "C" fn md_pic_reset(state: *mut Pic, now: u64) -> u32 {
    status((*state).reset(now))
}

/// Clear only the read latch at a transaction boundary. Partial command
/// parsing survives STOP, as required by the original byte-write driver.
///
/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_pic_stop(state: *mut Pic) {
    (*state).stop();
}

/// Return0 for ACK,1 for ordinary NACK, or a model error code.
///
/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_pic_start(state: *mut Pic, now: u64, read: bool) -> u32 {
    (*state).stop();
    let result = if read {
        (*state).read_ready(now)
    } else {
        (*state).ready(now)
    };
    status(result.and_then(|ready| if ready { Ok(()) } else { Err(Error::Busy) }))
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_pic_send(state: *mut Pic, now: u64, byte: u8) -> u32 {
    status((*state).write_byte(now, byte))
}

/// Output is unchanged on error. Low8bits contain the byte; bit8 indicates
/// a defined response. Later bytes in one transaction are undefined latches.
///
/// # Safety
/// `state` is live and exclusively borrowed; `out` is aligned and writable.
#[no_mangle]
pub unsafe extern "C" fn md_pic_recv(state: *mut Pic, now: u64, out: *mut u32) -> u32 {
    status((*state).read_byte(now).map(|byte| {
        *out = u32::from(byte.value) | (u32::from(byte.defined) << 8);
    }))
}

/// Absolute virtual deadline, or UINT64_MAX when no timer is needed.
///
/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_pic_deadline(state: *const Pic) -> u64 {
    (*state).next_deadline().unwrap_or(u64::MAX)
}

/// Bit0 enable, bit1 known voltage, bits8..15 voltage code.
///
/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_pic_power(state: *const Pic) -> u32 {
    u32::from((*state).voltage_enabled())
        | (u32::from((*state).voltage_code().is_some()) << 1)
        | (u32::from((*state).voltage_code().unwrap_or(0)) << 8)
}
