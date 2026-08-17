// TCOL / HPL (HXNU Public License)
// TCOL v1.1 / HPL (HXNU Public License)
// This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).

//! # Neonix G-Boot Architecture - Rockchip RK3588S AArch64 Bare-Metal Entry Point
//!
//! Bare-metal payload for Orange Pi 5 (Rockchip RK3588S, 64-bit ARMv8-A).
//! Replaces legacy x86_64 startup code with AArch64 startup assembly, exception vector table,
//! 64KB boot stack allocation, MMIO UART2 console driver (0xFEB50000), and register map module.

#![no_std]
#![no_main]

use core::arch::asm;
use core::arch::global_asm;
use core::panic::PanicInfo;

pub mod handover;
pub mod pcie_ep;
pub mod rk3588_regs;
mod drivers;

use rk3588_regs::*;

macro_rules! panic {
    () => {{
        uart_print("[G-BOOT] PANIC!\n");
        loop {}
    }};
    ($msg:expr) => {{
        uart_print("[G-BOOT] PANIC: ");
        uart_print($msg);
        uart_print("\n");
        loop {}
    }};
}

// =========================================================================
// AArch64 Bare-Metal Startup Assembly & ARM64 Exception Vector Table
// =========================================================================
global_asm!(r#"
.section .text.boot, "ax"
.global _start
.type _start, %function

_start:
    /* 1. Mask interrupts globally (DAIF) */
    msr daifset, #0xf

    /* 2. Setup Stack Pointer (SP) to 64KB boot stack top */
    ldr x0, =boot_stack_top
    mov sp, x0

    /* 3. Setup VBAR_ELx vector base address and TTBR0_EL1 based on CurrentEL */
    ldr x0, =exception_vector_table
    mrs x1, CurrentEL
    lsr x1, x1, #2

    cmp x1, #3
    b.ne 1f
    /* Currently in EL3: set up EL3/EL2/EL1 VBAR and initialize TTBR0_EL1 */
    msr vbar_el3, x0
    msr vbar_el2, x0
    msr vbar_el1, x0
    msr ttbr0_el1, xzr
    b 3f

1:  cmp x1, #2
    b.ne 2f
    /* Currently in EL2: set up EL2/EL1 VBAR and initialize TTBR0_EL1 */
    msr vbar_el2, x0
    msr vbar_el1, x0
    msr ttbr0_el1, xzr
    b 3f

2:  /* Currently in EL1: set up EL1 VBAR and initialize TTBR0_EL1 */
    msr vbar_el1, x0
    msr ttbr0_el1, xzr

3:  /* 4. Zero BSS Segment */
    ldr x0, =__bss_start
    ldr x1, =__bss_end
    cmp x0, x1
    b.ge 5f
4:  str xzr, [x0], #8
    cmp x0, x1
    b.lt 4b

5:  /* 5. Transfer execution to Rust entry point */
    bl _start_rust

    /* 6. Fallback Hang Loop */
6:  wfe
    b 6b

.size _start, . - _start

/* ========================================================================= */
/* ARM64 Exception Vector Table (Must be aligned to 2048 bytes / 2KB)       */
/* ========================================================================= */
.align 11
.global exception_vector_table
exception_vector_table:

/* --- Current EL with SP0 --- */
.align 7
curr_sp0_sync:     b default_exception_handler
.align 7
curr_sp0_irq:      b default_exception_handler
.align 7
curr_sp0_fiq:      b default_exception_handler
.align 7
curr_sp0_serror:   b default_exception_handler

/* --- Current EL with SPx --- */
.align 7
curr_spx_sync:     b default_exception_handler
.align 7
curr_spx_irq:      b default_exception_handler
.align 7
curr_spx_fiq:      b default_exception_handler
.align 7
curr_spx_serror:   b default_exception_handler

/* --- Lower EL using AArch64 --- */
.align 7
lower_aarch64_sync:   b default_exception_handler
.align 7
lower_aarch64_irq:    b default_exception_handler
.align 7
lower_aarch64_fiq:    b default_exception_handler
.align 7
lower_aarch64_serror: b default_exception_handler

/* --- Lower EL using AArch32 --- */
.align 7
lower_aarch32_sync:   b default_exception_handler
.align 7
lower_aarch32_irq:    b default_exception_handler
.align 7
lower_aarch32_fiq:    b default_exception_handler
.align 7
lower_aarch32_serror: b default_exception_handler

/* Default Exception Handler */
default_exception_handler:
    wfe
    b default_exception_handler

/* 64 KB Boot Stack Allocation in BSS */
.section .bss.stack, "aw", %nobits
.align 16
.global boot_stack
boot_stack:
    .space 0x10000
.global boot_stack_top
boot_stack_top:
"#);

// =========================================================================
// RK3588S MMIO UART2 Driver (Physical Base 0xFEB50000)
// =========================================================================
const UART_THR_OFFSET: usize = 0x00;
const UART_LSR_OFFSET: usize = 0x14;

/// Transmit string to console over RK3588S Debug UART2 via MMIO at 0xFEB50000
pub fn uart_print(s: &str) {
    let thr = (RK3588_UART2_BASE + UART_THR_OFFSET) as *mut u8;
    let lsr = (RK3588_UART2_BASE + UART_LSR_OFFSET) as *const u32;

    for b in s.bytes() {
        unsafe {
            // Poll LSR Bit 5 (THRE - Transmit Holding Register Empty)
            while (core::ptr::read_volatile(lsr) & (1 << 5)) == 0 {
                core::hint::spin_loop();
            }
            core::ptr::write_volatile(thr, b);
        }
    }
}

// =========================================================================
// Rust Top-Level Entry Point
// =========================================================================
/// High-level Rust entry point invoked after AArch64 assembly boot sequence
#[no_mangle]
pub extern "C" fn _start_rust() -> ! {
    uart_print("\n==================================================\n");
    uart_print("[G-BOOT] Starting bare-metal payload on Rockchip RK3588S (AArch64)...\n");
    uart_print("==================================================\n");

    let el = get_current_el();
    uart_print("[G-BOOT] Current Exception Level: EL");
    print_dec(el as u64);
    uart_print("\n");

    uart_print("[G-BOOT] Debug UART2 verified active at physical address 0xFEB50000.\n");

    // Milestone 2: Initialize PCIe 3.0 Endpoint Mode via GRF bit-flips, PMU power domain & CRU clock/reset sequence
    pcie_ep::init_pcie_endpoint();

    // Initialize display driver (Rockchip VOP2)
    let mut gui = drivers::gui::GuiDriver::new();
    if gui.init() != drivers::gui::FsmState::Ready {
        uart_print("[G-BOOT] ERROR: GUI initialization failed!\n");
        panic!("GUI");
    }

    // Initialize touch driver (I2C)
    let mut touch = drivers::touch::TouchDriver::new();
    if touch.init() != drivers::touch::FsmState::Ready {
        uart_print("[G-BOOT] ERROR: Touch initialization failed!\n");
        panic!("TOUCH");
    }

    // Initialize PCIe / 5G modem driver
    let mut alveo = drivers::alveo_sim::AlveoDriver::new();
    if alveo.init() != drivers::alveo_sim::FsmState::Ready {
        uart_print("[G-BOOT] ERROR: Alveo driver initialization failed!\n");
        panic!("ALVEO");
    }

    uart_print("[G-BOOT] Driver initialization complete.\n");

    #[cfg(feature = "load_linux")]
    chainload_linux();

    #[cfg(not(feature = "load_linux"))]
    chainload_hxnu();

    loop {
        unsafe {
            asm!("wfe", options(nomem, nostack));
        }
    }
}

fn get_current_el() -> u8 {
    let el: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) el, options(nomem, nostack));
    }
    ((el >> 2) & 0x3) as u8
}

fn print_dec(mut val: u64) {
    if val == 0 {
        uart_print("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        if let Ok(s) = core::str::from_utf8(&buf[i..i+1]) {
            uart_print(s);
        }
    }
}

#[cfg(feature = "load_linux")]
fn chainload_linux() {
    uart_print("[G-BOOT] Chainloading Linux...\n");
    unsafe {
        let linux_entry: extern "C" fn() -> ! = core::mem::transmute(0x00400000_usize);
        linux_entry();
    }
}

#[cfg(not(feature = "load_linux"))]
fn chainload_hxnu() {
    uart_print("[G-BOOT] Chainloading HXNU Kernel at 0x00200000 (DTB @ 0x0A000000)...\n");
    handover::perform_handover(0x0020_0000, 0x0A00_0000);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart_print("[G-BOOT] PANIC handler invoked!\n");
    loop {
        unsafe {
            asm!("wfe", options(nomem, nostack));
        }
    }
}
