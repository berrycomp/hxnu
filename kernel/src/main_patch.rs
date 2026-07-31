// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
use core::sync::atomic::{AtomicUsize, Ordering};

pub static HPS_HOOK_CALLED: AtomicUsize = AtomicUsize::new(0);

pub fn test_hps_hook(thread_id: u64, is_gpu: bool) {
    HPS_HOOK_CALLED.fetch_add(1, Ordering::SeqCst);
    crate::serial::write_str("HXNU: HPS Hook Called!\n");
}
