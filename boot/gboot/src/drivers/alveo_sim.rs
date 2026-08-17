// TCOL-COM (Tile Conservative Open License - Commercial IP)
// This file contains hardware IP simulation logic governed by the TCOL-COM license.

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

/// UART Registers for the Alveo 5G Modem
#[repr(C)]
#[cfg(target_arch = "aarch64")]
pub struct UartRegs {
    /// Receive Buffer / Transmit Holding Register / Divisor Latch Low
    pub rbr_thr_dll: u32,
    /// Interrupt Enable Register / Divisor Latch High
    pub ier_dlh: u32,
    /// FIFO Control Register
    pub fcr: u32,
    /// Line Control Register
    pub lcr: u32,
    /// Line Status Register
    pub lsr: u32,
}

#[cfg(target_arch = "aarch64")]
const ALVEO_ADDR: usize = 0xFEB50000;

/// Alveo 5G Modem Driver Struct
pub struct AlveoDriver {
    /// The current FSM state of the driver
    pub state: FsmState,
}

impl AlveoDriver {
    /// Creates a new AlveoDriver instance
    pub fn new() -> Self {
        Self { state: FsmState::Init }
    }

    /// Initializes the Alveo 5G Modem simulated over UART.
    /// On aarch64, it configures MMIO UART registers.
    #[cfg(target_arch = "aarch64")]
    pub fn init(&mut self) -> FsmState {
        self.state = FsmState::Polling;
        let regs = ALVEO_ADDR as *mut UartRegs;
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
            let mut ready = false;
            for _ in 0..100 {
                if (core::ptr::read_volatile(&(*regs).lsr) & 0x20) != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }
            core::ptr::write_volatile(&mut (*regs).rbr_thr_dll, 0x00000041);

            // Write 'T' (0x54)
            ready = false;
            for _ in 0..100 {
                if (core::ptr::read_volatile(&(*regs).lsr) & 0x20) != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }
            core::ptr::write_volatile(&mut (*regs).rbr_thr_dll, 0x00000054);
        }
        self.state = FsmState::Ready;
        FsmState::Ready
    }

    /// Initializes the Alveo 5G Modem on x86_64.
    /// Writes directly to COM1 port (0x3F8) using outb.
    /// Implements a bounded-poll FSM on the Line Status Register (0x3FD) before writing.
    #[cfg(target_arch = "x86_64")]
    pub fn init(&mut self) -> FsmState {
        self.state = FsmState::Polling;
        unsafe {
            let mut ready = false;
            for _ in 0..100 {
                let status: u8;
                core::arch::asm!("in al, dx", out("al") status, in("dx") 0x3FDu16);
                if (status & 0x20) != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }
            core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") b'A');
            
            ready = false;
            for _ in 0..100 {
                let status: u8;
                core::arch::asm!("in al, dx", out("al") status, in("dx") 0x3FDu16);
                if (status & 0x20) != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }
            core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") b'T');
        }
        self.state = FsmState::Ready;
        FsmState::Ready
    }
}
