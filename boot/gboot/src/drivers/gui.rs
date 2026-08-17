// TCOL v1.1 / HPL (HXNU Public License)
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

/// The VOP2 Display Controller Registers
#[repr(C)]
#[cfg(target_arch = "aarch64")]
pub struct Vop2Regs {
    /// System control register for enabling the display
    pub sys_ctrl: u32,
    /// System status register for polling readiness
    pub sys_status: u32,
    /// Background color register
    pub bg_color: u32,
    /// Display control register
    pub dsp_ctrl: u32,
}

#[cfg(target_arch = "aarch64")]
const GUI_ADDR: usize = 0xFDD90000;

/// GUI Driver Struct
pub struct GuiDriver {
    /// The current FSM state of the driver
    pub state: FsmState,
}

impl GuiDriver {
    /// Creates a new GuiDriver instance
    pub fn new() -> Self {
        Self { state: FsmState::Init }
    }

    /// Initializes the GUI driver depending on the architecture.
    /// On aarch64, configures the VOP2 display controller.
    #[cfg(target_arch = "aarch64")]
    pub fn init(&mut self) -> FsmState {
        self.state = FsmState::Polling;
        let regs = GUI_ADDR as *mut Vop2Regs;
        let mut ready = false;
        unsafe {
            // Write enable to sys_ctrl
            core::ptr::write_volatile(&mut (*regs).sys_ctrl, 0x00000001);

            // Bounded-poll sys_status for ready bit (e.g. bit 0)
            for _ in 0..100 {
                if (core::ptr::read_volatile(&(*regs).sys_status) & 0x1) != 0 {
                    ready = true;
                    break;
                }
            }

            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }

            // Set bg_color to 0xFF000000 (black)
            core::ptr::write_volatile(&mut (*regs).bg_color, 0xFF000000);

            // Set dsp_ctrl to active
            core::ptr::write_volatile(&mut (*regs).dsp_ctrl, 0x00000001);
        }
        self.state = FsmState::Ready;
        FsmState::Ready
    }

    /// Initializes the GUI driver for x86_64 architecture.
    /// Bypasses ARM MMIO by directly interacting with the physical VGA text buffer.
    /// Implements a bounded-poll FSM on the VGA Input Status Register (0x3DA) before writing.
    #[cfg(target_arch = "x86_64")]
    pub fn init(&mut self) -> FsmState {
        self.state = FsmState::Polling;
        let mut ready = false;
        unsafe {
            for _ in 0..100 {
                let status: u8;
                core::arch::asm!("in al, dx", out("al") status, in("dx") 0x3DAu16);
                if (status & 0x08) != 0 {
                    ready = true;
                    break;
                }
            }
            if !ready {
                self.state = FsmState::Error;
                return FsmState::Error;
            }
            let vga = 0xB8000 as *mut u16;
            // Write a green 'G' to the first cell of VGA
            core::ptr::write_volatile(vga, 0x0200 | b'G' as u16);
        }
        self.state = FsmState::Ready;
        FsmState::Ready
    }
}
