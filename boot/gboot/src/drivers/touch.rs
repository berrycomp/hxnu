/// Touch controller initialization via I2C MMIO
pub fn init() {
    let i2c_base = 0xFEAB0000 as *mut u32;
    unsafe {
        core::ptr::write_volatile(i2c_base.offset(0), 0x00000001);
        core::ptr::write_volatile(i2c_base.offset(1), 0x00000003);
    }
}
