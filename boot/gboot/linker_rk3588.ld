/*
 * TCOL v1.1 / HPL (HXNU Public License)
 * This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).
 *
 * Rockchip RK3588S G-Boot Bare-Metal Payload Linker Script
 * Target: RK3588S AArch64 (ARMv8-A 64-bit)
 * Target Triple: aarch64-hxnu-none
 * DRAM Base Origin: 0x00200000 (2MB mark in DRAM)
 */

ENTRY(_start)

SECTIONS
{
    /* 1. Memory Origin / DRAM Base Address */
    . = 0x00200000;

    /* 2. Text Section (Executable Code & Exception Vector Table) */
    .text : ALIGN(2048) {
        __text_start = .;
        /* Place entry symbol _start at absolute base address 0x00200000 */
        KEEP(*(.text.boot))

        /* Ensure ARM64 exception vector table is 2KB (2048-byte) aligned */
        . = ALIGN(2048);
        KEEP(*(.text.vector))
        KEEP(*(.text.exceptions))

        /* All other code sections */
        *(.text .text.*)
        . = ALIGN(8);
        __text_end = .;
    }

    /* 3. Read-Only Data Section */
    .rodata : ALIGN(8) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        . = ALIGN(8);
        __rodata_end = .;
    }

    /* 4. Read-Write Data Section */
    .data : ALIGN(8) {
        __data_start = .;
        *(.data .data.*)
        *(.got .got.*)
        . = ALIGN(8);
        __data_end = .;
    }

    /* 5. BSS & COMMON Section (Uninitialized Data & Boot Stack) */
    .bss : ALIGN(16) {
        . = ALIGN(8);
        __bss_start = .;
        *(.bss .bss.*)
        *(.bss.stack)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* 6. Discarded Metadata Sections */
    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
    }
}
