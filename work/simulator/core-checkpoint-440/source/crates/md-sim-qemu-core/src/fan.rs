//! Fan ABI. All instances are owned exclusively by the QEMU device thread.
use md_sim_firmware::fan::{Controller, InputEvent, MotorConfig};

#[repr(C)]
pub struct Config {
    minimum_rpm: u32,
    maximum_rpm: u32,
    slew_rpm_per_second: u32,
    pulses_per_revolution: u32,
    pwm_channel: u32,
    tach_channel: u32,
}

#[repr(C)]
pub struct AppliedInput {
    at_ns: u64,
    motor: u32,
    mask: u32,
    values: u32,
}

/// # Safety
/// `state` is exclusively borrowed; `json` holds `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn md_fan_input_plan(
    state: *mut Controller,
    json: *const u8,
    len: u32,
) -> bool {
    if state.is_null() || json.is_null() || len == 0 || len > 4096 {
        return false;
    }
    let Ok(events) =
        serde_json::from_slice::<Vec<InputEvent>>(std::slice::from_raw_parts(json, len as usize))
    else {
        return false;
    };
    (*state).install_input_plan(events).is_ok()
}

/// # Safety
/// `state` is exclusively borrowed; `event` is writable and aligned.
#[no_mangle]
pub unsafe extern "C" fn md_fan_take_input(
    state: *mut Controller,
    event: *mut AppliedInput,
) -> bool {
    let Some(input) = (*state).take_applied_input() else {
        return false;
    };
    *event = AppliedInput {
        at_ns: input.at_ns,
        motor: input.motor as u32,
        mask: u32::from(input.powered.is_some()) | (u32::from(input.stalled.is_some()) << 1),
        values: u32::from(input.powered.unwrap_or(false))
            | (u32::from(input.stalled.unwrap_or(false)) << 1),
    };
    true
}

/// # Safety
/// `configs` contains `count` readable aligned records, or is null for count0.
/// Caller owns the returned state. Null means invalid configuration.
#[no_mangle]
pub unsafe extern "C" fn md_fan_new(configs: *const Config, count: u32) -> *mut Controller {
    if count > 4 || (count != 0 && configs.is_null()) {
        return std::ptr::null_mut();
    }
    let configs = if count == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(configs, count as usize)
    };
    let configs: Vec<_> = configs
        .iter()
        .map(|c| MotorConfig {
            minimum_rpm: c.minimum_rpm,
            maximum_rpm: c.maximum_rpm,
            slew_rpm_per_second: c.slew_rpm_per_second,
            pulses_per_revolution: c.pulses_per_revolution,
            pwm_channel: c.pwm_channel as usize,
            tach_channel: (c.tach_channel != u32::MAX).then_some(c.tach_channel as usize),
        })
        .collect();
    Controller::new(&configs).map_or(std::ptr::null_mut(), |s| Box::into_raw(Box::new(s)))
}

/// # Safety
/// `state` is a unique owned pointer from md_fan_new, or null.
#[no_mangle]
pub unsafe extern "C" fn md_fan_free(state: *mut Controller) {
    if !state.is_null() {
        drop(Box::from_raw(state));
    }
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_reset(state: *mut Controller) {
    (*state).reset();
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_advance(state: *mut Controller, ns: u64) -> bool {
    (*state).advance(ns).is_ok()
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_advance_elapsed(state: *mut Controller, ns: u64) -> bool {
    (*state).advance_elapsed(ns).is_ok()
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_next_sample(state: *const Controller) -> u64 {
    (*state).next_sample_ns()
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_next_event(state: *const Controller) -> u64 {
    (*state).next_event_ns()
}

/// # Safety
/// `state` is live and exclusively borrowed; `value` is writable and aligned.
#[no_mangle]
pub unsafe extern "C" fn md_fan_read(
    state: *const Controller,
    address: u32,
    size: u32,
    value: *mut u32,
) -> bool {
    match (*state).read(address, size) {
        Ok(data) => {
            *value = data;
            true
        }
        Err(_) => false,
    }
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_write(
    state: *mut Controller,
    address: u32,
    size: u32,
    value: u32,
) -> bool {
    (*state).write(address, size, value).is_ok()
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_stalled(state: *mut Controller, motor: u32, value: bool) -> bool {
    (*state).set_stalled(motor as usize, value).is_ok()
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_fan_powered(state: *mut Controller, motor: u32, value: bool) -> bool {
    (*state).set_powered(motor as usize, value).is_ok()
}
