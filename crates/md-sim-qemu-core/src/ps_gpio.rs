//! Narrow C ABI for the Zynq-7000 PS GPIO state core.
use md_sim_firmware::ps_gpio::{Outputs, PsGpio, Word};

pub struct MdPsGpioState(PsGpio);

#[no_mangle]
pub extern "C" fn md_ps_gpio_new() -> *mut MdPsGpioState {
    Box::into_raw(Box::new(MdPsGpioState(PsGpio::new())))
}

/// # Safety
/// The pointer is null or owns a live state returned by md_ps_gpio_new.
/// The device thread serializes every call; release exactly once.
#[no_mangle]
pub unsafe extern "C" fn md_ps_gpio_free(state: *mut MdPsGpioState) {
    if !state.is_null() {
        drop(Box::from_raw(state));
    }
}

/// # Safety
/// A non-null pointer must be live and exclusively borrowed for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ps_gpio_bus_reset(state: *mut MdPsGpioState) {
    if let Some(state) = state.as_mut() {
        state.0.bus_interface_reset();
    }
}

/// # Safety
/// State is null or live; output is null or writable and does not alias state.
/// Failed calls leave the output unchanged. Unknown read bits remain explicit.
#[no_mangle]
pub unsafe extern "C" fn md_ps_gpio_read(
    state: *const MdPsGpioState,
    offset: u32,
    output: *mut Word,
) -> bool {
    let (Some(state), Some(output)) = (state.as_ref(), output.as_mut()) else {
        return false;
    };
    match state.0.read32(offset) {
        Ok(value) => {
            *output = value;
            true
        }
        Err(_) => false,
    }
}

/// # Safety
/// State is null or live and exclusively borrowed for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ps_gpio_write(
    state: *mut MdPsGpioState,
    offset: u32,
    value: u32,
) -> bool {
    state
        .as_mut()
        .is_some_and(|s| s.0.write32(offset, value).is_ok())
}

/// # Safety
/// State is null or live and exclusively borrowed for this call. The caller
/// supplies actual settled input observations, not invented board defaults.
#[no_mangle]
pub unsafe extern "C" fn md_ps_gpio_inputs(
    state: *mut MdPsGpioState,
    bank: u32,
    value: Word,
    changed_mask: u32,
) -> bool {
    let (Some(state), Ok(bank)) = (state.as_mut(), u8::try_from(bank)) else {
        return false;
    };
    state
        .0
        .set_settled_inputs(bank, value, changed_mask)
        .is_ok()
}

/// # Safety
/// State is null or live; output is null or writable and does not alias state.
#[no_mangle]
pub unsafe extern "C" fn md_ps_gpio_outputs(
    state: *const MdPsGpioState,
    bank: u32,
    output: *mut Outputs,
) -> bool {
    let (Some(state), Ok(bank), Some(output)) =
        (state.as_ref(), u8::try_from(bank), output.as_mut())
    else {
        return false;
    };
    match state.0.outputs(bank) {
        Ok(value) => {
            *output = value;
            true
        }
        Err(_) => false,
    }
}

/// # Safety
/// State is null or live. Null produces an explicitly unknown IRQ, not success.
#[no_mangle]
pub unsafe extern "C" fn md_ps_gpio_irq(state: *const MdPsGpioState) -> Word {
    state.as_ref().map_or(
        Word {
            value: 0,
            known: !1,
        },
        |s| s.0.irq(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_bounds_unknown_reads_and_failed_output_retention() {
        assert_eq!(std::mem::size_of::<Word>(), 8);
        assert_eq!(std::mem::align_of::<Word>(), 4);
        assert_eq!(std::mem::size_of::<Outputs>(), 16);
        assert_eq!(std::mem::offset_of!(Word, known), 4);
        assert_eq!(std::mem::offset_of!(Outputs, direction), 8);
        assert_eq!(std::mem::offset_of!(Outputs, enable), 12);
        unsafe {
            let state = md_ps_gpio_new();
            assert!(!state.is_null());
            let mut value = Word {
                value: 0xfeedface,
                known: u32::MAX,
            };
            assert!(!md_ps_gpio_read(state, 1, &mut value));
            assert_eq!(value.value, 0xfeedface);
            assert!(!md_ps_gpio_read(std::ptr::null(), 0x40, &mut value));
            assert!(!md_ps_gpio_read(state, 0x40, std::ptr::null_mut()));
            assert!(md_ps_gpio_read(state, 0x40, &mut value));
            assert_eq!(value.known, 0);
            assert!(!md_ps_gpio_inputs(
                state,
                256,
                Word {
                    value: 0,
                    known: u32::MAX
                },
                1
            ));
            assert!(!md_ps_gpio_write(state, 0x304, 0));
            assert!(md_ps_gpio_write(state, 0x214, u32::MAX));
            md_ps_gpio_bus_reset(state);
            assert!(md_ps_gpio_read(state, 0x20c, &mut value));
            assert_eq!(value.determined(), Some(u32::MAX));
            assert_eq!(md_ps_gpio_irq(state).determined(), Some(0));
            assert_eq!(md_ps_gpio_irq(std::ptr::null()).known & 1, 0);
            md_ps_gpio_free(state);
            md_ps_gpio_free(std::ptr::null_mut());
        }
    }
}
