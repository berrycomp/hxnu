#[repr(C)]
pub struct UartRegs {
    pub rbr_thr_dll: u32,
    pub ier_dlh: u32,
    pub fcr: u32,
    pub lcr: u32,
    pub lsr: u32,
}

static mut ALVEO_ADDR: u32 = 0xFDAB0000;

pub fn init() {
    let regs = unsafe { core::ptr::read_volatile(&ALVEO_ADDR) as *mut UartRegs };
    unsafe {
        // Set lcr to DLAB (0x80)
        core::ptr::write_volatile(&mut (*regs).lcr, 0x00000080);
        
        // Set baud divisor (dll=1, dlh=0)
        core::ptr::write_volatile(&mut (*regs).rbr_thr_dll, 0x00000001);
        core::ptr::write_volatile(&mut (*regs).ier_dlh, 0x00000000);
        
        // Clear lcr DLAB (e.g. 8-bit data, 1 stop bit, no parity -> 0x03)
        core::ptr::write_volatile(&mut (*regs).lcr, 0x00000003);
        
        // Enable FIFOs in fcr (0xC7)
        core::ptr::write_volatile(&mut (*regs).fcr, 0x000000C7);
        
        // Write 'A' (0x41)
        for _ in 0..100 {
            if (core::ptr::read_volatile(&(*regs).lsr) & 0x20) != 0 {
                break;
            }
        }
        core::ptr::write_volatile(&mut (*regs).rbr_thr_dll, 0x00000041);

        // Write 'T' (0x54)
        for _ in 0..100 {
            if (core::ptr::read_volatile(&(*regs).lsr) & 0x20) != 0 {
                break;
            }
        }
        core::ptr::write_volatile(&mut (*regs).rbr_thr_dll, 0x00000054);
    }
}
