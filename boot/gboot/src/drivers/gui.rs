/// GUI initialization using VOP2 MMIO representation
pub fn init() {
    let vop_base = 0xFDD90000 as *mut u32;
    unsafe {
        core::ptr::write_volatile(vop_base.offset(0), 0x00000001);
        core::ptr::write_volatile(vop_base.offset(1), 0x0000000F);
    }
}
