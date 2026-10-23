//! Thin C interface for the checked configuration-port state.
use md_sim_firmware::bitstream::{port::Port, series7::frames::Part};
use std::ffi::{c_char, CString};

pub struct Pcap {
    port: Port,
    error: CString,
}

#[repr(C)]
pub struct PcapProgress {
    pub received: u64,
    pub omitted_preamble: u64,
    pub sha256: [u8; 65],
}

/// # Safety
/// Each pointer refers to its specified readable length. Part bytes and the
/// container are captured, hashed run inputs; neither is guest-controlled.
#[no_mangle]
pub unsafe extern "C" fn md_pcap_new(
    container: *const u8,
    size: usize,
    part: *const u8,
    part_size: usize,
) -> *mut Pcap {
    if size > 64 << 20 || part_size > 1 << 20 || size == 0 || part_size == 0 {
        return std::ptr::null_mut();
    }
    let part = std::slice::from_raw_parts(part, part_size);
    let result = Part::from_pinned_json(part, &md_sim_firmware::prepare::hash_bytes(part))
        .and_then(|part| Port::new(std::slice::from_raw_parts(container, size), &part));
    result
        .map(|port| {
            Box::into_raw(Box::new(Pcap {
                port,
                error: CString::default(),
            }))
        })
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// Pointer is live and owned, or null. Release once.
#[no_mangle]
pub unsafe extern "C" fn md_pcap_free(state: *mut Pcap) {
    if !state.is_null() {
        drop(Box::from_raw(state));
    }
}

/// # Safety
/// State is live and exclusively borrowed on the device thread.
#[no_mangle]
pub unsafe extern "C" fn md_pcap_reset(state: *mut Pcap) {
    (*state).port.reset();
    (*state).error = CString::default();
}

/// # Safety
/// State is live and exclusively borrowed on the device thread.
#[no_mangle]
pub unsafe extern "C" fn md_pcap_begin(state: *mut Pcap) {
    (*state).port.begin();
}

/// # Safety
/// State is live and exclusively borrowed on the device thread.
#[no_mangle]
pub unsafe extern "C" fn md_pcap_reject(state: *mut Pcap) {
    (*state).port.reject();
}

/// Return 0 on rejected data, 1 while receiving, 2 after the entire image.
/// # Safety
/// State and progress are live and exclusive. Bytes is readable for size.
#[no_mangle]
pub unsafe extern "C" fn md_pcap_write(
    state: *mut Pcap,
    bytes: *const u8,
    size: usize,
    progress: *mut PcapProgress,
) -> u32 {
    let state = &mut *state;
    let result = state
        .port
        .write_dma(std::slice::from_raw_parts(bytes, size));
    let mut hash = [0; 65];
    hash[..64].copy_from_slice(state.port.received_sha256().as_bytes());
    *progress = PcapProgress {
        received: state.port.received() as u64,
        omitted_preamble: state.port.omitted_preamble() as u64,
        sha256: hash,
    };
    match result {
        Ok(()) => {
            if state.port.complete() {
                2
            } else {
                1
            }
        }
        Err(e) => {
            state.error = CString::new(format!("{e:#}")).unwrap();
            0
        }
    }
}

/// # Safety
/// State is live. Returned string remains valid until the next state mutation.
#[no_mangle]
pub unsafe extern "C" fn md_pcap_error(state: *const Pcap) -> *const c_char {
    (*state).error.as_ptr()
}
