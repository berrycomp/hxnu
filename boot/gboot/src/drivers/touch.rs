/// I2C Registers for the Touch Controller
#[repr(C)]
pub struct I2cRegs {
    /// I2C control register
    pub i2c_con: u32,
    /// I2C transmit data register
    pub i2c_txdata: u32,
    /// I2C receive data register
    pub i2c_rxdata: u32,
    /// I2C status register
    pub i2c_sr: u32,
}

#[cfg(target_arch = "aarch64")]
static mut TOUCH_ADDR: u32 = 0xFEAB0000;

/// Initializes the Touch Controller.
/// On aarch64, performs an I2C transaction to query the touch controller.
#[cfg(target_arch = "aarch64")]
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

/// Initializes the Touch Controller on x86_64.
/// Reads from the PS/2 keyboard port directly to avoid unmapped MMIO errors.
#[cfg(target_arch = "x86_64")]
pub fn init() {
    unsafe {
        let _data: u8;
        core::arch::asm!("in al, dx", out("al") _data, in("dx") 0x60u16);
    }
}
