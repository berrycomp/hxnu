; AP bootstrap trampoline
; Assembled with: yasm -f bin -o ap_trampoline.bin ap_trampoline.asm

BITS 16
ORG 0x0000

; POST code helper for QEMU debug (out 0x80, al)
%macro POST 1
    mov al, %1
    out 0x80, al
%endmacro

ap_trampoline_start:
    POST 0x01       ; entered trampoline
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax

    ; DS = CS so data references are page-relative
    mov ax, cs
    mov ds, ax
    mov es, ax

    POST 0x02       ; about to enable A20
    ; Fast A20 enable
    in al, 0x92
    or al, 2
    out 0x92, al

    POST 0x03       ; about to lgdt
    ; Load GDT. Descriptor is at offset 0x02F0.
    lgdt [gdtr]

    POST 0x04       ; about to enable protected mode
    ; Enable protected mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    POST 0x05       ; about to far-jump to 32-bit
    ; Far jump to 32-bit protected mode
    jmp 0x08:protected_mode

    times (0x0100 - ($ - ap_trampoline_start)) db 0

BITS 32
protected_mode:
    POST 0x10       ; reached 32-bit protected mode
    mov eax, 0x10
    mov ds, eax
    mov es, eax
    mov ss, eax
    mov fs, eax
    mov gs, eax

    POST 0x11       ; about to enable PAE
    ; Enable PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    POST 0x12       ; about to load CR3
    ; Load CR3 from mailbox
    mov eax, [page_table_phys]
    mov cr3, eax

    POST 0x13       ; about to enable long mode
    ; Enable long mode
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8) | (1 << 11)
    wrmsr

    POST 0x14       ; about to enable paging
    ; Enable paging
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    POST 0x15       ; about to far-jump to 64-bit
    ; Enter long mode with an absolute offset inside the identity-mapped
    ; trampoline page. In 64-bit mode CS base is ignored, so using the
    ; raw label offset here would jump to linear 0x00000200 instead of
    ; trampoline_base + 0x0200.
    jmp 0x18:(TRAMPOLINE_PHYSICAL_BASE + long_mode)

    times (0x0200 - ($ - ap_trampoline_start)) db 0

BITS 64
long_mode:
    POST 0x20       ; reached 64-bit long mode
    mov ax, 0x20
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    POST 0x21       ; about to discover trampoline base
    ; Discover trampoline base without touching the stack. The stack
    ; pointer is loaded from the mailbox a few instructions later.
    lea rbx, [rel ap_trampoline_start]

    POST 0x22       ; about to load stack
    ; Load stack and arguments from mailbox (rbx-relative)
    mov rsp, qword [rbx + stack_ptr]

    POST 0x23       ; about to set done_flag
    mov dword [rbx + done_flag], 1

    POST 0x24       ; about to load arguments
    mov rdi, qword [rbx + cpu_index]
    mov rsi, qword [rbx + percpu_base]

    POST 0x25       ; about to call rust_entry
    mov rax, qword [rbx + rust_entry]
    call rax

    ; Should never return
    POST 0x2F
    cli
.hlt_loop:
    hlt
    jmp .hlt_loop

    times (0x02F0 - ($ - ap_trampoline_start)) db 0

gdtr:
    dw gdt_end - gdt - 1    ; limit
    dd gdt                  ; base (patched by BSP to trampoline_base + gdt)

    times (0x0300 - ($ - ap_trampoline_start)) db 0

gdt:
    dq 0x0000000000000000          ; 0x00 : null
    dq 0x00cf9b000000ffff          ; 0x08 : 32-bit code, base patched
    dq 0x00cf93000000ffff          ; 0x10 : 32-bit data, base patched
    dq 0x00af9b000000ffff          ; 0x18 : 64-bit code
    dq 0x00cf93000000ffff          ; 0x20 : 64-bit data
gdt_end:

    times (0x0380 - ($ - ap_trampoline_start)) db 0

done_flag:      dd 0
cpu_index:      dd 0
stack_ptr:      dq 0
page_table_phys: dq 0
rust_entry:     dq 0
percpu_base:    dq 0

    times (0x0400 - ($ - ap_trampoline_start)) db 0

ap_trampoline_end:

; Assemble-time constant for trampoline physical base.
TRAMPOLINE_PHYSICAL_BASE equ 0x8000
