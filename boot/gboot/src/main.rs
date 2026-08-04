// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).

// Neonix G-Boot Architecture - Iteration 13
#![no_std]
#![no_main]
use core::panic::PanicInfo;
use core::arch::global_asm;
use core::arch::asm;

macro_rules! panic {
    () => {{
        uart_print("PANIC!\n");
        loop {}
    }};
    ($msg:expr) => {{
        uart_print("PANIC: ");
        uart_print($msg);
        uart_print("\n");
        loop {}
    }};
}

mod drivers;

global_asm!(r#"
.section .note.Xen, "a"
.align 4
.long 4
.long 4
.long 18
1: .asciz "Xen"
.align 4
3: .long _start32
4: .align 4

.section .text
.code32
.global _start32
_start32:
    lgdt gdt64_ptr
    
    mov $pml4, %eax
    mov %eax, %cr3

    mov %cr4, %eax
    or $0x20, %eax
    mov %eax, %cr4

    mov $0xC0000080, %ecx
    rdmsr
    or $0x100, %eax
    wrmsr

    mov %cr0, %eax
    or $0x80000001, %eax
    mov %eax, %cr0

    ljmp $0x08, $_start64

.code64
_start64:
    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    mov $0x200000, %rsp
    
    call _start

    cli
1:  hlt
    jmp 1b

.section .data
.align 4096
pml4:
    .long pdpt + 3
    .long 0
    .fill 4088, 1, 0
pdpt:
    .long pd0 + 3
    .long 0
    .long pd1 + 3
    .long 0
    .long pd2 + 3
    .long 0
    .long pd3 + 3
    .long 0
    .fill 4064, 1, 0
pd0:
    // map first 1GB with 2MB pages
    .set i, 0
    .rept 512
    .long (i << 21) | 0x83
    .long 0
    .set i, i + 1
    .endr
pd1:
    .set i, 512
    .rept 512
    .long (i << 21) | 0x83
    .long 0
    .set i, i + 1
    .endr
pd2:
    .set i, 1024
    .rept 512
    .long (i << 21) | 0x83
    .long 0
    .set i, i + 1
    .endr
pd3:
    .set i, 1536
    .rept 512
    .long (i << 21) | 0x83
    .long 0
    .set i, i + 1
    .endr

.align 8
gdt64:
    .long 0
    .long 0
    .long 0
    .long 0x00209A00
    .long 0
    .long 0x00009200
gdt64_ptr:
    .short 23
    .long gdt64
"#, options(att_syntax));

pub fn uart_print(s: &str) {
    let port: u16 = 0x3f8;
    for b in s.bytes() {
        unsafe {
            asm!("out dx, al", in("dx") port, in("al") b, options(nomem, nostack, preserves_flags));
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    uart_print("G-Boot starting...\n");
    let mut gui = drivers::gui::GuiDriver::new();
    if gui.init() != drivers::gui::FsmState::Ready {
        panic!("GUI");
    }
    
    let mut touch = drivers::touch::TouchDriver::new();
    if touch.init() != drivers::touch::FsmState::Ready {
        panic!("TOUCH");
    }

    let mut alveo = drivers::alveo_sim::AlveoDriver::new();
    if alveo.init() != drivers::alveo_sim::FsmState::Ready {
        panic!("ALVEO");
    }
    uart_print("Driver initialization complete.\n");

    #[cfg(feature = "load_linux")]
    chainload_linux();

    #[cfg(not(feature = "load_linux"))]
    chainload_hxnu();

    loop {}
}

#[cfg(feature = "load_linux")]
fn chainload_linux() {
    uart_print("Chainloading Linux...\n");
    unsafe {
        let linux_entry: extern "C" fn() -> ! = core::mem::transmute(0x1000000_usize);
        linux_entry();
    }
}

#[cfg(not(feature = "load_linux"))]
fn chainload_hxnu() {
    uart_print("Chainloading HXNU...\n");
    unsafe { asm!("cli; hlt"); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart_print("PANIC!\n");
    loop {}
}
