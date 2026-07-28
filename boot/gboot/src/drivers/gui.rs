#[repr(C)]
pub struct Vop2Regs {
    pub reset: u32,
    pub status: u32,
    pub config: u32,
}
static mut GUI_ADDR: u32 = 0xFDD90000;
pub fn init() {
    let regs = unsafe { core::ptr::read_volatile(&GUI_ADDR) as *mut Vop2Regs };
    unsafe {
        core::ptr::write_volatile(&mut (*regs).reset, 0x00000001);
        for _ in 0..100 {
            if (core::ptr::read_volatile(&(*regs).status) & 0x1) != 0 {
                break;
            }
        }
        core::ptr::write_volatile(&mut (*regs).config, 0x0000000F);
    }
}
