// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
/// NeoIO Bridge (Bare-Metal)
/// Provides a zero-copy FFI interface for .hxext kernel modules (like HPS)
/// to access the internal hardware topology (Flattened Device Tree / ACPI nodes).

use crate::neoio::{NEOIO_MANAGER, SharedTopologyTree};

/// FFI interface for .hxext modules to get a pointer to the shared topology tree.
/// HPS uses this to discover GPUs, NPUs, and AVX-512 capabilities dynamically.
#[unsafe(no_mangle)]
pub extern "C" fn hps_get_topology_tree() -> *const SharedTopologyTree {
    unsafe {
        &raw const NEOIO_MANAGER.shared_tree as *const SharedTopologyTree
    }
}
