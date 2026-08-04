// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).

//! Supernova Driver Module
//!
//! This module implements the `no_std` driver for the Supernova hardware.
//! It provides structures and synchronization primitives required for GPU/accelerator interaction.

use core::sync::atomic::{AtomicBool, AtomicUsize, AtomicU32, Ordering};
use core::hint::spin_loop;
use core::ptr::{read_volatile, write_volatile};

/// The physical base address for the Supernova hardware MMIO.
pub const SUPERNOVA_PHYS_BASE: u64 = 0xFE00_0000;

/// Returns the virtual MMIO base pointer in HHDM.
pub fn supernova_mmio_base() -> *mut SupernovaHardware {
    let hhdm = crate::limine::hhdm_offset().unwrap_or(0);
    (hhdm + SUPERNOVA_PHYS_BASE) as *mut SupernovaHardware
}

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
    /// A new `SupernovaDriver` initialized with HHDM virtual base address.
    pub fn new() -> Self {
        let base_virt = if let Some(offset) = crate::limine::hhdm_offset() {
            (offset + SUPERNOVA_PHYS_BASE) as usize
        } else {
            SUPERNOVA_PHYS_BASE as usize
        };
        Self {
            is_initialized: AtomicBool::new(false),
            seq_tracker: AtomicU32::new(0),
            mmio: AtomicUsize::new(base_virt),
            lock: AtomicBool::new(false),
        }
    }

    /// Initializes the Supernova driver and maps the MMIO region globally.
    pub fn init(&self) {
        if let Some(offset) = crate::limine::hhdm_offset() {
            let virt_addr = offset + SUPERNOVA_PHYS_BASE;
            crate::arch::x86_64::map_kernel_region(
                virt_addr,
                SUPERNOVA_PHYS_BASE,
                0x2000,
                crate::arch::x86_64::FLAG_WRITE_THROUGH
                    | crate::arch::x86_64::FLAG_CACHE_DISABLE
                    | crate::arch::x86_64::FLAG_GLOBAL
                    | crate::arch::x86_64::FLAG_WRITABLE,
            ).ok();
            self.mmio.store(virt_addr as usize, Ordering::SeqCst);
        }
        self.is_initialized.store(true, Ordering::SeqCst);
    }

    fn mmio_ptr(&self) -> *mut SupernovaHardware {
        let addr = self.mmio.load(Ordering::Relaxed);
        if addr < 0x1000_0000_0000 {
            if let Some(offset) = crate::limine::hhdm_offset() {
                let virt = (offset + SUPERNOVA_PHYS_BASE) as usize;
                self.mmio.store(virt, Ordering::Relaxed);
                return virt as *mut SupernovaHardware;
            }
        }
        addr as *mut SupernovaHardware
    }

    /// Rings the doorbell to notify the GSP of a new task.
    pub fn ring_doorbell(&self, task_id: u32) {
        unsafe {
            write_volatile(&mut (*self.mmio_ptr()).doorbell, task_id);
        }
    }

    /// Waits for the GSP to complete the submitted task.
    pub fn wait_for_gsp(&self, expected_task_id: u32) {
        unsafe {
            while read_volatile(&(*self.mmio_ptr()).completion) != expected_task_id {
                spin_loop();
            }
        }
    }

    /// Acknowledges the completion of a task to the GSP.
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

    /// Acquires the driver lock.
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
    pub fn route_sxrc(&self, payload: crate::sxrc_core::SxrcPayload) {
        let _guard = self.lock();
        let (size, data) = match &payload {
            crate::sxrc_core::SxrcPayload::Hex2(p) => (p.size, &p.data[..]),
            crate::sxrc_core::SxrcPayload::Hex4(p) => (p.size, &p.data[..]),
        };

        if size > data.len() {
            return;
        }

        if self.write_to_ring_buffer(0, &data[..size]) {
            let route_id = size as u32;
            self.ring_doorbell(route_id);
        }
    }

    /// Synchronizes operations on the device.
    pub fn synchronize(&self) {
        let _guard = self.lock();
        let sync_task_id = self.seq_tracker.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.ring_doorbell(sync_task_id);
        self.wait_for_gsp(sync_task_id);
        self.ack_task(sync_task_id);
    }
}
