// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).
/// HPS Bridge (Bare-Metal)
/// Provides a zero-copy FFI interface for .hxext kernel modules (like HPS)
/// to access the internal hardware scheduler's SharedRingBuffer.

use crate::hsched::{HSCHED, SharedRingBuffer};

/// FFI interface for .hxext modules to get a pointer to the shared ring buffer.
/// Since HPS is compiled as a kernel extension, it can directly call this 
/// symbol and read/write to the buffer with zero syscall latency.
#[unsafe(no_mangle)]
pub extern "C" fn hps_get_shared_buffer() -> *const SharedRingBuffer {
    &HSCHED.shared_buffer as *const SharedRingBuffer
}
