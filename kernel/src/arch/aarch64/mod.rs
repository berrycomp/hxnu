/// AARCH64 (ARMv8.2-A / ARMv9) Architecture specific module
/// Targeting Orange Pi 5 (Rockchip RK3588)

#[cfg(target_arch = "aarch64")]
pub mod boot;

#[cfg(target_arch = "aarch64")]
pub mod serial;

#[cfg(target_arch = "aarch64")]
pub fn init() {
    // AARCH64 specific initialization
    serial::init_uart();
    crate::println!("HXNU Bare-Metal: AARCH64 (Rockchip RK3588) Init...");
}

#[cfg(target_arch = "aarch64")]
pub fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("wfe") }
    }
}
