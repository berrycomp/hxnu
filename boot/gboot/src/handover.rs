// THOL (Turkish Hybrid Open License)
// TCOL v1.1 / HPL (Turkish Hybrid Open License)
// This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).

//! # G-Boot Handover Routine - GICv3 Shutdown, Cache/MMU Flush & HXNU Kernel Entry
//!
//! Handles low-level AArch64 hardware teardown:
//! - Data Cache range flush (`dc civac`) and full Set/Way clean & invalidate (`dc cisw`)
//! - MMU and D/I Cache disabling in `SCTLR_EL1` (bits M=0, C=0, I=0)
//! - Translation Lookaside Buffer invalidation (`tlbi vmalle1is`)
//! - Instruction Cache invalidation (`ic iallu`)
//! - GICv3 CPU interface disable & DAIF interrupt masking
//! - Floating-Point / SIMD enable at EL1 (`CPACR_EL1`)
//! - Exception Level (EL3/EL2 -> EL1h) transition and register contract setup:
//!   - x0 = dtb_ptr (0x0A000000)
//!   - x1 = 0
//!   - x2 = 0
//!   - x3 = 0
//!   - pc/ELR = kernel_entry (0x00200000)

#![allow(dead_code)]

use core::arch::asm;
use crate::rk3588_regs::*;

/// Physical memory address default for HXNU Kernel entry point
pub const HXNU_ENTRY_POINT: usize = 0x0020_0000;

/// Physical memory address default for Device Tree Blob (DTB) / Boot Header
pub const DTB_HEADER_PTR: usize = 0x0A00_0000;

/// Clean and Invalidate Data Cache by Virtual Address range (`dc civac`, `dsb sy`)
pub fn flush_dcache_range(start: usize, end: usize) {
    let ctr: u64;
    unsafe {
        asm!("mrs {}, CTR_EL0", out(reg) ctr, options(nomem, nostack));
    }
    let dmin_line = ((ctr >> 16) & 0xF) as u32;
    let line_size = 1usize << (dmin_line + 2);

    let mut addr = start & !(line_size - 1);
    while addr < end {
        unsafe {
            asm!("dc civac, {}", in(reg) addr, options(nomem, nostack));
        }
        addr += line_size;
    }
    unsafe {
        asm!("dsb sy", options(nomem, nostack));
    }
}

/// Clean and Invalidate all Data Cache levels by Set/Way (`CLIDR_EL1`, `CCSIDR_EL1`, `dc cisw`, `dsb sy`)
pub fn flush_dcache_all_setway() {
    let clidr: u64;
    unsafe {
        asm!("mrs {}, CLIDR_EL1", out(reg) clidr, options(nomem, nostack));
    }
    let loc = ((clidr >> 24) & 0x7) as u32;

    for level in 0..loc {
        let csselr = (level << 1) as u64;
        unsafe {
            asm!("msr CSSELR_EL1, {}", in(reg) csselr, options(nomem, nostack));
            asm!("isb", options(nomem, nostack));
        }

        let ccsidr: u64;
        unsafe {
            asm!("mrs {}, CCSIDR_EL1", out(reg) ccsidr, options(nomem, nostack));
        }

        let line_size = (ccsidr & 0x7) as u32;
        let line_shift = line_size + 4;
        let num_ways = (((ccsidr >> 3) & 0x3FF) + 1) as u32;
        let num_sets = (((ccsidr >> 13) & 0x7FFF) + 1) as u32;

        let way_shift = (num_ways - 1).leading_zeros();

        for way in 0..num_ways {
            for set in 0..num_sets {
                let setway_val = ((way << way_shift) | (set << line_shift) | (level << 1)) as u64;
                unsafe {
                    asm!("dc cisw, {}", in(reg) setway_val, options(nomem, nostack));
                }
            }
        }
    }
    unsafe {
        asm!("dsb sy", options(nomem, nostack));
    }
}

/// Disable MMU, Data Cache, and Instruction Cache in `SCTLR_EL1` (M=bit 0, C=bit 2, I=bit 12, `dsb sy`, `isb`)
pub fn disable_mmu_and_caches() {
    let mut sctlr: u64;
    unsafe {
        asm!("mrs {}, SCTLR_EL1", out(reg) sctlr, options(nomem, nostack));
        sctlr &= !((1 << 0) | (1 << 2) | (1 << 12));
        asm!("msr SCTLR_EL1, {}", in(reg) sctlr, options(nomem, nostack));
        asm!("dsb sy", options(nomem, nostack));
        asm!("isb", options(nomem, nostack));
    }
}

/// Invalidate Translation Lookaside Buffers (`tlbi vmalle1is`, `dsb sy`, `isb`)
pub fn invalidate_tlb() {
    unsafe {
        asm!("tlbi vmalle1is", options(nomem, nostack));
        asm!("dsb sy", options(nomem, nostack));
        asm!("isb", options(nomem, nostack));
    }
}

/// Invalidate Instruction Cache (`ic iallu`, `dsb sy`, `isb`)
pub fn invalidate_icache() {
    unsafe {
        asm!("ic iallu", options(nomem, nostack));
        asm!("dsb sy", options(nomem, nostack));
        asm!("isb", options(nomem, nostack));
    }
}

/// Mask DAIF interrupts, disable GICv3 CPU interface & distributor (`msr daifset, #0xf`, `ICC_IGRPEN1_EL1 = 0`, `ICC_PMR_EL1 = 0x00`, `GICD_CTLR = 0`, `dsb sy`, `isb`)
pub fn disable_gicv3() {
    unsafe {
        asm!("msr daifset, #0xf", options(nomem, nostack));

        let mut sre: u64;
        asm!("mrs {}, ICC_SRE_EL1", out(reg) sre, options(nomem, nostack));
        if (sre & 0x1) == 0 {
            sre |= 0x1;
            asm!("msr ICC_SRE_EL1, {}", in(reg) sre, options(nomem, nostack));
            asm!("isb", options(nomem, nostack));
        }

        let zero: u64 = 0;
        asm!("msr ICC_IGRPEN1_EL1, {}", in(reg) zero, options(nomem, nostack));
        asm!("msr ICC_PMR_EL1, {}", in(reg) zero, options(nomem, nostack));

        write_volatile(GICD_CTLR, 0x0000_0000);

        asm!("dsb sy", options(nomem, nostack));
        asm!("isb", options(nomem, nostack));
    }
}

/// Enable Floating-Point and SIMD (NEON) instructions at EL1 (`CPACR_EL1` bits [21:20] = 0b11, `isb`)
pub fn enable_fp_simd_el1() {
    let mut cpacr: u64;
    unsafe {
        asm!("mrs {}, CPACR_EL1", out(reg) cpacr, options(nomem, nostack));
        cpacr |= 3 << 20;
        asm!("msr CPACR_EL1, {}", in(reg) cpacr, options(nomem, nostack));
        asm!("isb", options(nomem, nostack));
    }
}

/// Get current processor Exception Level from `CurrentEL` system register
pub fn get_current_el() -> u8 {
    let el: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) el, options(nomem, nostack));
    }
    ((el >> 2) & 0x3) as u8
}

/// Perform complete maintenance sequence and transfer execution to HXNU Kernel
///
/// Hardware Handover Contract:
/// - x0: Physical DTB pointer (default 0x0A000000)
/// - x1..x3: 0
/// - SPSR_ELx: 0x3C5 (EL1h mode, DAIF masked)
/// - ELR_ELx: kernel_entry (default 0x00200000)
pub fn perform_handover(kernel_entry: usize, dtb_ptr: usize) -> ! {
    crate::uart_print("\n[G-BOOT] Executing hardware teardown & HXNU handover sequence...\n");

    // 1. Disable GICv3 CPU interface & distributor, mask interrupts
    disable_gicv3();

    // 2. Enable FP/SIMD access at EL1
    enable_fp_simd_el1();

    // 3. Clean & invalidate D-Cache range for DTB (64KB) and Kernel text (2MB)
    flush_dcache_range(dtb_ptr, dtb_ptr + 0x10000);
    flush_dcache_range(kernel_entry, kernel_entry + 0x200000);

    // 4. Clean & invalidate all D-Cache levels by Set/Way
    flush_dcache_all_setway();

    // 5. Disable MMU, D-Cache, and I-Cache in SCTLR_EL1
    disable_mmu_and_caches();

    // 6. Invalidate TLB
    invalidate_tlb();

    // 7. Invalidate I-Cache
    invalidate_icache();

    crate::uart_print("[G-BOOT] Maintenance complete. Transferring control to HXNU Kernel at ");
    print_hex_handover(kernel_entry as u64);
    crate::uart_print(" (DTB @ ");
    print_hex_handover(dtb_ptr as u64);
    crate::uart_print(")...\n\n");

    let el = get_current_el();

    unsafe {
        match el {
            3 => {
                asm!(
                    // SCR_EL3: NS=1 (bit 0), HCE=1 (bit 8), RW=1 (bit 10), ST=1 (bit 11)
                    "mrs x4, scr_el3",
                    "orr x4, x4, #(1 << 0)",
                    "orr x4, x4, #(1 << 8)",
                    "orr x4, x4, #(1 << 10)",
                    "orr x4, x4, #(1 << 11)",
                    "msr scr_el3, x4",

                    // HCR_EL2: RW=1 (bit 31)
                    "mov x4, #(1 << 31)",
                    "msr hcr_el2, x4",

                    // SPSR_EL3: EL1h mode (0x5), DAIF masked (0x3C0) => 0x3C5
                    "mov x4, #0x3C5",
                    "msr spsr_el3, x4",

                    // Set ELR_EL3 to kernel entry
                    "msr elr_el3, {entry}",

                    // Set register contract: x0=dtb_ptr, x1=0, x2=0, x3=0
                    "mov x0, {dtb}",
                    "mov x1, #0",
                    "mov x2, #0",
                    "mov x3, #0",

                    "isb",
                    "eret",
                    entry = in(reg) kernel_entry,
                    dtb = in(reg) dtb_ptr,
                    options(noreturn)
                );
            }
            2 => {
                asm!(
                    // HCR_EL2: RW=1 (bit 31)
                    "mov x4, #(1 << 31)",
                    "msr hcr_el2, x4",

                    // SPSR_EL2: EL1h mode (0x5), DAIF masked (0x3C0) => 0x3C5
                    "mov x4, #0x3C5",
                    "msr spsr_el2, x4",

                    // Set ELR_EL2 to kernel entry
                    "msr elr_el2, {entry}",

                    // Set register contract: x0=dtb_ptr, x1=0, x2=0, x3=0
                    "mov x0, {dtb}",
                    "mov x1, #0",
                    "mov x2, #0",
                    "mov x3, #0",

                    "isb",
                    "eret",
                    entry = in(reg) kernel_entry,
                    dtb = in(reg) dtb_ptr,
                    options(noreturn)
                );
            }
            _ => {
                asm!(
                    // Already at EL1: direct branch with register contract x0=dtb_ptr, x1..x3=0
                    "mov x0, {dtb}",
                    "mov x1, #0",
                    "mov x2, #0",
                    "mov x3, #0",

                    "isb",
                    "br {entry}",
                    entry = in(reg) kernel_entry,
                    dtb = in(reg) dtb_ptr,
                    options(noreturn)
                );
            }
        }
    }
}

/// Alias for `perform_handover`
pub fn jump_to_hxnu(kernel_entry: usize, dtb_ptr: usize) -> ! {
    perform_handover(kernel_entry, dtb_ptr);
}

fn print_hex_handover(val: u64) {
    crate::uart_print("0x");
    let hex_chars = b"0123456789ABCDEF";
    let mut started = false;
    for i in (0..16).rev() {
        let digit = ((val >> (i * 4)) & 0xF) as usize;
        if digit != 0 || started || i == 0 {
            started = true;
            let buf = [hex_chars[digit]];
            if let Ok(s) = core::str::from_utf8(&buf) {
                crate::uart_print(s);
            }
        }
    }
}
