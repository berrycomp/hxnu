/*
 * HXNU Sequential Linker Script
 * G-Boot (GRAND-Bootloader) Kexec Target Map
 * Architecture: x86_64 Bare-Metal
 */

ENTRY(_start)

SECTIONS
{
    . = 0x200000;

    .note.Xen : ALIGN(4)
    {
        KEEP(*(.note.Xen))
    }

    .multiboot_header : ALIGN(4K)
    {
        KEEP(*(.multiboot_header))
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
