//! Supernova Driver Module
//!
//! This module implements the `no_std` driver for the Supernova hardware.
//! It provides structures and synchronization primitives required for GPU/accelerator interaction.

use core::sync::atomic::{AtomicBool, AtomicUsize, AtomicU32, Ordering};
use core::hint::spin_loop;
use core::ptr::{read_volatile, write_volatile};

/// The base address for the Supernova hardware MMIO.
const SUPERNOVA_MMIO_BASE: *mut SupernovaHardware = 0xFE00_0000 as *mut SupernovaHardware;

/// Genuine MMIO hardware registers mapped in memory.
#[repr(C)]
pub struct SupernovaHardware {
    /// Zero-latency doorbell register.
    pub doorbell: u32,
    /// Completion register.
    pub completion: u32,
    /// Acknowledgment register.
    pub ack: u32,
    /// Sequence register or reserved.
    pub _reserved: u32,
    /// A physical ring buffer or shared memory area for passing payloads to the hardware.
    pub ring_buffer: [u8; 4096],
}

/// The main driver structure for the Supernova hardware.
///
/// This struct holds the state and context required to communicate with
/// the Supernova device using genuine MMIO.
pub struct SupernovaDriver {
    /// Indicates whether the driver is currently initialized.
    pub is_initialized: AtomicBool,
    /// Software-side sequence tracker to eliminate Read-Modify-Write races.
    pub seq_tracker: AtomicU32,
    /// Internal pointer to the mapped hardware MMIO struct.
    mmio: AtomicUsize,
    /// Spinlock for thread-safe access to the MMIO device.
    lock: AtomicBool,
}

/// Guard structure for RAII-based spinlock management.
pub struct SupernovaGuard<'a> {
    lock: &'a AtomicBool,
}

impl<'a> Drop for SupernovaGuard<'a> {
    fn drop(&mut self) {
        self.lock.store(false, Ordering::Release);
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("sti", options(nomem, nostack));
        }
    }
}


impl SupernovaDriver {
    /// Creates a new instance of the `SupernovaDriver`.
    ///
    /// # Returns
    /// A new, uninitialized `SupernovaDriver` mapping to the base MMIO pointer.
    pub const fn new() -> Self {
        Self {
            is_initialized: AtomicBool::new(false),
            seq_tracker: AtomicU32::new(0),
            mmio: AtomicUsize::new(0xFE00_0000),
            lock: AtomicBool::new(false),
        }
    }

    /// Initializes the Supernova driver.
    ///
    /// This function sets up the necessary hardware state and prepares
    /// the device for operation.
    pub fn init(&self) {
        if let Some(offset) = crate::limine::hhdm_offset() {
            crate::arch::x86_64::map_kernel_region(
                0xFE00_0000 + offset,
                0xFE00_0000,
                0x2000,
                crate::arch::x86_64::FLAG_WRITE_THROUGH | crate::arch::x86_64::FLAG_CACHE_DISABLE,
            ).ok();
            self.mmio.store((0xFE00_0000_u64 + offset) as usize, Ordering::SeqCst);
        }
        self.is_initialized.store(true, Ordering::SeqCst);
    }

    fn mmio_ptr(&self) -> *mut SupernovaHardware {
        self.mmio.load(Ordering::Relaxed) as *mut SupernovaHardware
    }

    /// Rings the doorbell to notify the GSP of a new task.
    ///
    /// # Arguments
    /// * `task_id` - An identifier or payload for the task being submitted.
    pub fn ring_doorbell(&self, task_id: u32) {
        unsafe {
            write_volatile(&mut (*self.mmio_ptr()).doorbell, task_id);
        }
    }

    /// Waits for the GSP to complete the submitted task.
    ///
    /// This function employs a spin-wait loop for zero-latency execution,
    /// blocking until the completion register matches the expected task ID.
    ///
    /// # Arguments
    /// * `expected_task_id` - The task identifier we are waiting to finish.
    pub fn wait_for_gsp(&self, expected_task_id: u32) {
        unsafe {
            while read_volatile(&(*self.mmio_ptr()).completion) != expected_task_id {
                spin_loop();
            }
        }
    }
    /// Acknowledges the completion of a task to the GSP.
    ///
    /// # Arguments
    /// * `task_id` - The identifier of the task being acknowledged.
    pub fn ack_task(&self, task_id: u32) {
        unsafe {
            write_volatile(&mut (*self.mmio_ptr()).ack, task_id);
        }
    }

    /// Writes raw bytes to the shared memory ring buffer.
    fn write_to_ring_buffer(&self, offset: usize, data: &[u8]) -> bool {
        if offset + data.len() > 4096 {
            return false;
        }
        unsafe {
            let buffer_ptr = (*self.mmio_ptr()).ring_buffer.as_mut_ptr() as *mut u32;
            let u32_offset = offset / 4;
            for (i, chunk) in data.chunks(4).enumerate() {
                let mut bytes = [0u8; 4];
                bytes[..chunk.len()].copy_from_slice(chunk);
                let val = u32::from_ne_bytes(bytes);
                write_volatile(buffer_ptr.add(u32_offset + i), val);
            }
        }
        true
    }

    /// Acquires the driver lock
    pub fn lock(&self) -> SupernovaGuard<'_> {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("cli", options(nomem, nostack));
        }
        while self.lock.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            spin_loop();
        }
        SupernovaGuard { lock: &self.lock }
    }

    /// Routes a generic SXRC payload directly to the Supernova hardware interface.
    ///
    /// This function intercepts the unified `SxrcPayload` format and
    /// processes the encapsulated HEX2 or HEX4 tasks, leveraging
    /// the underlying GSP zero-latency communication primitives.
    ///
    /// # Arguments
    /// * `payload` - The unified SXRC payload to be offloaded.
    pub fn route_sxrc(&self, payload: crate::sxrc_core::SxrcPayload) {
        let _guard = self.lock();
        let (size, data) = match &payload {
            crate::sxrc_core::SxrcPayload::Hex2(p) => (p.size, &p.data[..]),
            crate::sxrc_core::SxrcPayload::Hex4(p) => (p.size, &p.data[..]),
        };

        if size > data.len() {
            return;
        }

        // Write the actual payload into memory before ringing the doorbell
        if self.write_to_ring_buffer(0, &data[..size]) {
            let route_id = size as u32;
            self.ring_doorbell(route_id);
        }
    }

    /// Routes a MaRTix payload to the appropriate compute backend.
    ///
    /// This function prepares the payload based on the selected compute backend
    /// and rings the GSP doorbell to signal the hardware.
    ///
    /// # Arguments
    /// * `payload` - The MaRTix payload containing the compute command and SXRC data.
    pub fn route_martix(&self, payload: crate::martix::MartixPayload) {
        let _guard = self.lock();
        let (size, data) = match &payload.payload {
            crate::sxrc_core::SxrcPayload::Hex2(p) => (p.size, &p.data[..]),
            crate::sxrc_core::SxrcPayload::Hex4(p) => (p.size, &p.data[..]),
        };

        if size > data.len() {
            return;
        }

        let backend_id = match payload.backend {
            crate::martix::ComputeBackend::Cuda => 1,
            crate::martix::ComputeBackend::Vulkan => 2,
            crate::martix::ComputeBackend::Metal => 3,
        };

        let command_id = match payload.command {
            crate::martix::RTCoreCommand::RayTrace => 0,
            crate::martix::RTCoreCommand::MatrixMultiply => 1,
        };

        // Write backend, command, and actual payload into memory before ringing doorbell
        let success1 = self.write_to_ring_buffer(0, &[backend_id as u8, command_id as u8, 0, 0]);
        let success2 = self.write_to_ring_buffer(4, &data[..size]);

        if success1 && success2 {
            let task_id = (backend_id << 16) | (size as u32 & 0xFFFF);
            self.ring_doorbell(task_id);
        }
    }

    /// Synchronizes operations on the device.
    ///
    /// This function orchestrates a complete synchronization cycle with the GSP:
    /// submitting a sync task, spin-waiting for its completion, and acknowledging it.
    pub fn synchronize(&self) {
        let _guard = self.lock();
        // Use software sequence tracker instead of reading from hardware to avoid RMW race
        let sync_task_id = self.seq_tracker.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.ring_doorbell(sync_task_id);
        self.wait_for_gsp(sync_task_id);
        self.ack_task(sync_task_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sxrc_core::{SxrcPayload, Hex2Payload};

    #[test]
    fn test_route_sxrc_oob_size_no_panic() {
        let driver = SupernovaDriver::new();
        let payload = SxrcPayload::Hex2(Hex2Payload {
            data: [0; 32],
            size: 100, // Invalid size, larger than 32
        });
        
        // This will now safely return early without panic or deadlock
        driver.route_sxrc(payload);
    }
}
