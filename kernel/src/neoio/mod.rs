// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).

pub mod iommu;

/// NeoIO: Hardware topology bridge and 2nd stage firmware for Neonix.
/// Acts as the ultimate dictator for IOMMU virtualization, power management,
/// and unified hardware topology (ACPI/DTB abstraction).

use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::UnsafeCell;
use core::arch::asm;
use crate::power;

#[derive(Copy, Clone, PartialEq)]
pub enum TopologySource {
    Acpi,
    DeviceTreeBlob,
    HotPlugPcie,
}

#[derive(Copy, Clone)]
pub struct NeoNode {
    pub vendor_id: u16,
    pub device_id: u16,
    pub is_npu: bool,
    pub is_gpu: bool,
    pub source: TopologySource,
    pub iommu_domain_isolated: bool,
}

pub struct NeoDeviceTree {
    pub nodes: [Option<NeoNode>; 64],
    pub node_count: usize,
}

pub struct NeoIoManager {
    is_active: AtomicBool,
    lock: AtomicBool,
    tree: UnsafeCell<NeoDeviceTree>,
}

unsafe impl Sync for NeoIoManager {}

impl NeoIoManager {
    pub const fn new() -> Self {
        const INIT_NODE: Option<NeoNode> = None;
        Self {
            is_active: AtomicBool::new(false),
            lock: AtomicBool::new(false),
            tree: UnsafeCell::new(NeoDeviceTree {
                nodes: [INIT_NODE; 64],
                node_count: 0,
            }),
        }
    }

    /// Acquires the spinlock and disables local interrupts to prevent MSI-X deadlocks/races.
    fn lock_tree(&self) -> &mut NeoDeviceTree {
        // Disable interrupts to prevent MSI-X from preempting while we hold the lock
        unsafe { asm!("cli", options(nomem, nostack)); }
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        unsafe { &mut *self.tree.get() }
    }

    fn unlock_tree(&self) {
        self.lock.store(false, Ordering::Release);
        // Re-enable interrupts
        unsafe { asm!("sti", options(nomem, nostack)); }
    }

    pub fn get_tree_ptr(&self) -> *const NeoDeviceTree {
        self.tree.get() as *const NeoDeviceTree
    }

    pub fn init(&self) {
        let tree = self.lock_tree();
        // Pseudo-code: Parse ACPI MADT/DMAR or DTB
        self.is_active.store(true, Ordering::SeqCst);
        self.unlock_tree();
    }

    /// Dynamically called during a PCIe Hot-Plug event (via MSI-X)
    /// Injects a new Flattened Device Tree node into the live kernel securely.
    pub fn inject_dynamic_fdt_node(&self, vendor_id: u16, device_id: u16, is_npu: bool, is_gpu: bool) {
        if !self.is_active.load(Ordering::SeqCst) {
            return;
        }
        
        let tree = self.lock_tree();
        
        if tree.node_count < tree.nodes.len() {
            // Dictator policy: All hot-plugged devices are instantly isolated via IOMMU
            iommu::reconfigure_iommu_group(0, 0, 0); // Placeholder for actual BDF
            
            tree.nodes[tree.node_count] = Some(NeoNode {
                vendor_id,
                device_id,
                is_npu,
                is_gpu,
                source: TopologySource::HotPlugPcie,
                iommu_domain_isolated: true, // Default deny
            });
            tree.node_count += 1;
        } else {
            crate::tty::write_str("NeoIO: [ERROR] Device tree capacity exceeded during hot-plug!\n");
        }
        
        self.unlock_tree();
    }
    
    // --- 2nd Stage Firmware: Power Management Dictatorship ---
    
    pub fn system_reboot(&self) -> ! {
        crate::tty::write_str("NeoIO: Orchestrating system reboot...\n");
        // NeoIO can instruct devices to enter D3 (quiesce) here before rebooting
        let _ = power::reboot();
        power::halt_forever();
    }
    
    pub fn system_halt(&self) -> ! {
        crate::tty::write_str("NeoIO: Orchestrating system halt...\n");
        power::halt_forever();
    }
}

pub static NEOIO_MANAGER: NeoIoManager = NeoIoManager::new();
