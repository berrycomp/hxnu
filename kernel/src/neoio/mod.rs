// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
#![no_std]

pub mod iommu;

/// NeoIO: Hardware topology bridge for Neonix.
/// Converts ACPI/PCIe topologies into dynamic Flattened Device Tree (FDT) nodes
/// to support seamless virtualization and poor IOMMU/ACS systems.

use core::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
pub struct DeviceNode {
    pub vendor_id: u16,
    pub device_id: u16,
    pub is_npu: bool,
    pub is_gpu: bool,
}

#[repr(C)]
pub struct SharedTopologyTree {
    pub nodes: [Option<DeviceNode>; 32],
    pub node_count: usize,
}

pub struct NeoIoManager {
    is_active: AtomicBool,
    pub shared_tree: SharedTopologyTree,
}

impl NeoIoManager {
    pub const fn new() -> Self {
        const INIT_NODE: Option<DeviceNode> = None;
        Self {
            is_active: AtomicBool::new(false),
            shared_tree: SharedTopologyTree {
                nodes: [INIT_NODE; 32],
                node_count: 0,
            },
        }
    }

    /// Initializes NeoIO, parsing the initial ACPI tables and generating the base FDT.
    pub fn init(&self) {
        // Pseudo-code: Parse ACPI MADT/DMAR
        self.is_active.store(true, Ordering::SeqCst);
    }

    /// Dynamically called during a PCIe Hot-Plug event (via MSI-X)
    /// Injects a new Flattened Device Tree node into the live kernel without rebooting.
    pub fn inject_dynamic_fdt_node(&mut self, vendor_id: u16, device_id: u16, is_npu: bool, is_gpu: bool) {
        if !self.is_active.load(Ordering::SeqCst) {
            return;
        }
        
        if self.shared_tree.node_count < self.shared_tree.nodes.len() {
            self.shared_tree.nodes[self.shared_tree.node_count] = Some(DeviceNode {
                vendor_id,
                device_id,
                is_npu,
                is_gpu,
            });
            self.shared_tree.node_count += 1;
        }
    }
}

pub static NEOIO_MANAGER: NeoIoManager = NeoIoManager::new();
