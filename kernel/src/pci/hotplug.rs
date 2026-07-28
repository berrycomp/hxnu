#![no_std]

use crate::neoio::{NEOIO_MANAGER, iommu::reconfigure_iommu_group};
use crate::pci::oculink::OculinkBridge;

/// PCIe Hot-Plug Event Handler (MSI-X)
/// Bypasses ACPI standard hotplug in favor of zero-latency direct hardware interrupts from HPS.

/// Registered MSI-X Interrupt Service Routine (ISR) for Hot-Plug.
#[no_mangle]
pub extern "C" fn hps_msix_hotplug_handler(vector: u32) {
    // 1. Identify the PCIe Bus/Device that triggered the Hot-Plug
    let target_bus = 0x01; // Mock
    let target_device = 0x00; // Mock
    
    // 2. Hardware isolation via NeoIO
    reconfigure_iommu_group(target_bus, target_device, 0);

    // 3. Inject new FDT topology dynamically
    NEOIO_MANAGER.inject_dynamic_fdt_node(0x10DE, 0x2204); // e.g., Vendor ID

    // 4. If it's the CM5 over OCuLink, trigger Kernel-to-Kernel handshake
    if OculinkBridge::handshake_hxnu_peer(target_bus, target_device, 0) {
        OculinkBridge::establish_peer_ipc();
    }
}
