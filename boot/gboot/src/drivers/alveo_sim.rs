/// Alveo Sim initialization via custom IP MMIO
pub fn init() {
    let alveo_base = 0xFDAB0000 as *mut u32;
    unsafe {
        core::ptr::write_volatile(alveo_base.offset(0), 0xDEADBEEF);
        core::ptr::write_volatile(alveo_base.offset(1), 0x00000001);
    }
}
