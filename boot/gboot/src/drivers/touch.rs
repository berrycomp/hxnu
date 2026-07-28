#[repr(C)]
pub struct TouchRegs {
    pub reset: u32,
    pub status: u32,
    pub config: u32,
}
static mut TOUCH_ADDR: u32 = 0xFEAB0000;
pub fn init() {
    let regs = unsafe { core::ptr::read_volatile(&TOUCH_ADDR) as *mut TouchRegs };
    unsafe {
        core::ptr::write_volatile(&mut (*regs).reset, 0x00000001);
        for _ in 0..100 {
            if (core::ptr::read_volatile(&(*regs).status) & 0x1) != 0 {
                break;
            }
        }
        core::ptr::write_volatile(&mut (*regs).config, 0x00000003);
    }
}
