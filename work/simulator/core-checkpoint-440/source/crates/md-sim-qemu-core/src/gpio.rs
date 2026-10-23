//! Fixed-direction AXI GPIO C interface. Pin values are already settled.
use md_sim_firmware::axi_gpio::{Direction, Gpio};

/// Return null for invalid configuration. Direction0 is input,1 is output.
/// The caller owns the returned state and must release it exactly once.
#[no_mangle]
pub extern "C" fn md_gpio_new(
    width: u32,
    direction: u32,
    reset_data: u32,
    reset_tri: u32,
    inputs: u32,
) -> *mut Gpio {
    let Some(width) = u8::try_from(width).ok() else {
        return std::ptr::null_mut();
    };
    let direction = match direction {
        0 => Direction::Input,
        1 => Direction::Output,
        _ => return std::ptr::null_mut(),
    };
    Gpio::new(width, direction, reset_data, reset_tri, inputs)
        .map_or(std::ptr::null_mut(), |gpio| Box::into_raw(Box::new(gpio)))
}

/// # Safety
/// `state` is a live unique pointer from `md_gpio_new`, or null.
#[no_mangle]
pub unsafe extern "C" fn md_gpio_free(state: *mut Gpio) {
    if !state.is_null() {
        drop(Box::from_raw(state));
    }
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_gpio_reset(state: *mut Gpio) {
    (*state).reset();
}

/// # Safety
/// `state` is live and exclusively borrowed; `value` is writable and aligned.
#[no_mangle]
pub unsafe extern "C" fn md_gpio_read(state: *const Gpio, offset: u32, value: *mut u32) -> bool {
    let Ok(offset) = u16::try_from(offset) else {
        return false;
    };
    match (*state).read(offset) {
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
pub unsafe extern "C" fn md_gpio_write(state: *mut Gpio, offset: u32, data: u32) -> bool {
    u16::try_from(offset).is_ok_and(|offset| (*state).write(offset, data, 15).is_ok())
}

/// # Safety
/// `state` is live and exclusively borrowed. Inputs have settled externally.
#[no_mangle]
pub unsafe extern "C" fn md_gpio_inputs(state: *mut Gpio, inputs: u32) {
    (*state).set_settled_inputs(inputs);
}

/// # Safety
/// `state` is live and exclusively borrowed.
#[no_mangle]
pub unsafe extern "C" fn md_gpio_outputs(state: *const Gpio) -> u32 {
    (*state).outputs()
}
