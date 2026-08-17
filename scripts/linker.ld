/*
 * HXNU Sequential Linker Script
 * G-Boot (GRAND-Bootloader) Kexec Target Map
 * Architecture: x86_64 Bare-Metal
 */

ENTRY(_start)

SECTIONS
{
    . = 0x200000;

    /* Multiboot2 Header MUST be the absolute first section and 8-byte aligned */
    .multiboot_header : ALIGN(8)
    {
        KEEP(*(.multiboot_header))
    }

    /* Xen Notes (If you really need them for bare-metal, keep them here) */
    .note.Xen : ALIGN(4)
    {
        KEEP(*(.note.Xen))
    }

    .text : ALIGN(4K)
    {
        *(.text .text.*)
    }

    .rodata : ALIGN(4K)
    {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K)
    {
        *(.data .data.*)
    }

    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss .bss.*)
    }

    __kernel_end = .;
}
