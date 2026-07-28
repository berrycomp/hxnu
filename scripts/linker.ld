/* 
 * HXNU Sequential Linker Script 
 * G-Boot (GRAND-Bootloader) Kexec Target Map
 * Architecture: x86_64 Bare-Metal
 */

ENTRY(_start)

SECTIONS
{
    /* 
     * G-Boot Kexec Sıçrama Noktası: 2 MB Çizgisi (0x200000)
     * KESİNLİKLE DEĞİŞTİRİLEMEZ (Immutable). Çok fazla flag içeren devasa bir 
     * kernel imajının başlangıç güvenliği için 2MB barajı seçildi.
     */
    . = 0x200000;

    /* Multiboot Header (If needed as fallback, MUST be at the very start) */
    .boot : ALIGN(4K)
    {
        KEEP(*(.multiboot_header))
    }

    /* Çekirdek Kod (Executable) */
    .text : ALIGN(4K)
    {
        *(.text .text.*)
    }

    /* Salt-Okunur Veri (Read-Only Data) */
    .rodata : ALIGN(4K)
    {
        *(.rodata .rodata.*)
    }

    /* Başlangıç Değeri Atanmış Veri (Initialized Data) */
    .data : ALIGN(4K)
    {
        *(.data .data.*)
    }

    /* Sıfır Atanmış Veri (Uninitialized Data / Block Starting Symbol) */
    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss .bss.*)
    }

    /* Sembollerin Kernel tarafından okunabilmesi için Limit Belirteçleri */
    __kernel_end = .;
}
