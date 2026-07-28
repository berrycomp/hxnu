#[repr(C)]
pub struct AlveoRegs {
    pub reset: u32,
    pub status: u32,
    pub config: u32,
}
static mut ALVEO_ADDR: u32 = 0xFDAB0000;
pub fn init() {
    let regs = unsafe { core::ptr::read_volatile(&ALVEO_ADDR) as *mut AlveoRegs };
    unsafe {
        core::ptr::write_volatile(&mut (*regs).reset, 0xDEADBEEF);
        for _ in 0..100 {
            if (core::ptr::read_volatile(&(*regs).status) & 0x1) != 0 {
                break;
            }
        }
        core::ptr::write_volatile(&mut (*regs).config, 0x00000001);
    }
}
