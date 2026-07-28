#[repr(C)]
pub struct Vop2Regs {
    pub sys_ctrl: u32,
    pub sys_status: u32,
    pub bg_color: u32,
    pub dsp_ctrl: u32,
}

static mut GUI_ADDR: u32 = 0xFDD90000;

pub fn init() {
    let regs = unsafe { core::ptr::read_volatile(&GUI_ADDR) as *mut Vop2Regs };
    unsafe {
        // Write enable to sys_ctrl
        core::ptr::write_volatile(&mut (*regs).sys_ctrl, 0x00000001);
        
        // Bounded-poll sys_status for ready bit (e.g. bit 0)
        for _ in 0..100 {
            if (core::ptr::read_volatile(&(*regs).sys_status) & 0x1) != 0 {
                break;
            }
        }
        
        // Set bg_color to 0xFF000000 (black)
        core::ptr::write_volatile(&mut (*regs).bg_color, 0xFF000000);
        
        // Set dsp_ctrl to active
        core::ptr::write_volatile(&mut (*regs).dsp_ctrl, 0x00000001);
    }
}
