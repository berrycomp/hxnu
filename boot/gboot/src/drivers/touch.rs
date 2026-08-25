// THOL (Turkish Hybrid Open License)
// TCOL v1.1 / HPL (Turkish Hybrid Open License)
// This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).

/// The FSM State for hardware drivers
#[derive(PartialEq)]
pub enum FsmState {
    /// Initializing state
    Init,
    /// Polling state
    Polling,
    /// Ready state
    Ready,
    /// Error state
    Error,
}

/// I2C Registers for the Touch Controller
#[repr(C)]
#[cfg(target_arch = "aarch64")]
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
const TOUCH_ADDR: usize = 0xFEAB0000;

/// Touch Controller Driver Struct
pub struct TouchDriver {
    /// The current FSM state of the driver
    pub state: FsmState,
}

impl TouchDriver {
    /// Creates a new TouchDriver instance
    pub fn new() -> Self {
        Self { state: FsmState::Init }
    }

    /// Initializes the Touch Controller.
    /// On aarch64, performs an I2C transaction to query the touch controller.
    #[cfg(target_arch = "aarch64")]
    pub fn init(&mut self) -> FsmState {
        self.state = FsmState::Polling;
        let regs = TOUCH_ADDR as *mut I2cRegs;
        let mut ready = false;
        unsafe {
            // Set i2c_con to enable
            core::ptr::write_volatile(&mut (*regs).i2c_con, 0x00000001);

            // Write a touch query command (0x55) to txdata
            core::ptr::write_volatile(&mut (*regs).i2c_txdata, 0x00000055);

            // Bounded-poll i2c_sr
            for _ in 0..100000 {
                if (core::ptr::read_volatile(&(*regs).i2c_sr) & 0x1) != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }

            // Read rxdata
            let _data = core::ptr::read_volatile(&(*regs).i2c_rxdata);
        }
        self.state = FsmState::Ready;
        FsmState::Ready
    }

    /// Initializes the Touch Controller on x86_64.
    /// Reads from the PS/2 keyboard port directly to avoid unmapped MMIO errors.
    /// Implements a bounded-poll FSM on the Status Register (0x64) before reading from 0x60.
    #[cfg(target_arch = "x86_64")]
    pub fn init(&mut self) -> FsmState {
        self.state = FsmState::Polling;
        unsafe {
            // Wait for Input Buffer Empty (bit 1 is 0)
            let mut ready = false;
            for _ in 0..100000 {
                let status: u8;
                core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16);
                if (status & 0x02) == 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }

            // Send Self-Test command (0xAA)
            core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xAAu8);

            // Wait for Output Buffer Full (bit 0 is 1)
            ready = false;
            for _ in 0..100000 {
                let status: u8;
                core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16);
                if (status & 0x01) != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }

            // Read the response (0x55 expected)
            let response: u8;
            core::arch::asm!("in al, dx", out("al") response, in("dx") 0x60u16);
            if response != 0x55 {
                self.state = FsmState::Error;
                return FsmState::Error;
            }
        }
        self.state = FsmState::Ready;
        FsmState::Ready
    }
}
