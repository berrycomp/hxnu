#[repr(C)]
pub struct I2cRegs {
    pub i2c_con: u32,
    pub i2c_txdata: u32,
    pub i2c_rxdata: u32,
    pub i2c_sr: u32,
}

static mut TOUCH_ADDR: u32 = 0xFEAB0000;

pub fn init() {
    let regs = unsafe { core::ptr::read_volatile(&TOUCH_ADDR) as *mut I2cRegs };
    unsafe {
        // Set i2c_con to enable
        core::ptr::write_volatile(&mut (*regs).i2c_con, 0x00000001);
        
        // Write a touch query command (0x55) to txdata
        core::ptr::write_volatile(&mut (*regs).i2c_txdata, 0x00000055);
        
        // Bounded-poll i2c_sr
        for _ in 0..100 {
            if (core::ptr::read_volatile(&(*regs).i2c_sr) & 0x1) != 0 {
                break;
            }
        }
        
        // Read rxdata
        let _data = core::ptr::read_volatile(&(*regs).i2c_rxdata);
    }
}
