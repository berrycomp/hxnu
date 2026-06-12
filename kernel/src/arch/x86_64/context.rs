use core::arch::global_asm;

use crate::kprintln;
use super::gdt;

const STACK_ALIGNMENT: usize = 16;
pub const CONTEXT_KIND_KERNEL: u64 = 0;
pub const CONTEXT_KIND_USER: u64 = 1;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TaskContext {
    pub rsp: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub cr3: u64,
    pub kind: u64,
    pub kernel_rsp0: u64,
}

impl TaskContext {
    pub const fn empty() -> Self {
        Self {
            rsp: 0,
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            cr3: 0,
            kind: CONTEXT_KIND_KERNEL,
            kernel_rsp0: 0,
        }
    }
}

pub fn initialize_kernel_thread(
    context: &mut TaskContext,
    stack: &'static mut [u8],
    entry: extern "C" fn() -> !,
) {
    let kernel_cr3 = context.cr3;
    let stack_base = stack.as_mut_ptr() as usize;
    let stack_end = stack_base + stack.len();
    let aligned_end = stack_end & !(STACK_ALIGNMENT - 1);
    let stack_words = aligned_end as *mut usize;

    unsafe {
        let thread_exit_slot = stack_words.sub(1);
        thread_exit_slot.write(thread_exit_trap as *const () as usize);
        let entry_slot = thread_exit_slot.sub(1);
        entry_slot.write(entry as usize);

        *context = TaskContext {
            rsp: entry_slot as u64,
            cr3: kernel_cr3,
            kind: CONTEXT_KIND_KERNEL,
            ..TaskContext::empty()
        };
    }
}

pub fn initialize_user_thread(
    context: &mut TaskContext,
    stack: &'static mut [u8],
    entry_point: u64,
    user_stack: u64,
    page_table: u64,
) {
    let stack_base = stack.as_mut_ptr() as usize;
    let stack_end = stack_base + stack.len();
    let aligned_end = stack_end & !(STACK_ALIGNMENT - 1);
    let stack_words = aligned_end as *mut u64;

    unsafe {
        let resume_frame = stack_words.sub(20);
        for index in 0..15 {
            resume_frame.add(index).write(0);
        }
        resume_frame.add(15).write(entry_point);
        resume_frame.add(16).write(gdt::USER_CODE_SELECTOR as u64 | 3);
        resume_frame.add(17).write(0x202);
        resume_frame.add(18).write(user_stack);
        resume_frame.add(19).write(gdt::USER_DATA_SELECTOR as u64 | 3);

        let rsp_value = resume_frame as u64;
        kprintln!(
            "HXNU: init_user_thread stack_base={:#018x} len={:#x} aligned_end={:#018x} rsp={:#018x} rsp0={:#018x}",
            stack_base, stack.len(), aligned_end, rsp_value, aligned_end
        );
        *context = TaskContext {
            rsp: rsp_value,
            cr3: page_table,
            kind: CONTEXT_KIND_USER,
            kernel_rsp0: aligned_end as u64,
            ..TaskContext::empty()
        };
    }
}

pub unsafe fn switch_with_cr3(current: &mut TaskContext, next: &TaskContext) {
    unsafe {
        hxnu_switch_with_cr3(current as *mut TaskContext, next as *const TaskContext);
    }
}

pub unsafe fn resume_with_cr3(next: &TaskContext) -> ! {
    unsafe {
        hxnu_resume_with_cr3(next as *const TaskContext);
    }
}

extern "C" fn thread_exit_trap() -> ! {
    loop {
        unsafe {
            core::arch::asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

unsafe extern "C" {
    fn hxnu_switch_with_cr3(current: *mut TaskContext, next: *const TaskContext) -> !;
    fn hxnu_resume_with_cr3(next: *const TaskContext) -> !;
}

global_asm!(
    r#"
    .global hxnu_context_switch
    .type hxnu_context_switch,@function
hxnu_context_switch:
    mov [rdi + 0x00], rsp
    mov [rdi + 0x08], rbx
    mov [rdi + 0x10], rbp
    mov [rdi + 0x18], r12
    mov [rdi + 0x20], r13
    mov [rdi + 0x28], r14
    mov [rdi + 0x30], r15
    mov qword ptr [rdi + 0x40], 0
    jmp hxnu_resume_common

    .global hxnu_switch_with_cr3
    .type hxnu_switch_with_cr3,@function
hxnu_switch_with_cr3:
    mov rax, [rsi + 0x38]
    mov rdx, cr3
    cmp rax, rdx
    je 1f
    mov cr3, rax
1:
    jmp hxnu_context_switch

    .global hxnu_resume_with_cr3
    .type hxnu_resume_with_cr3,@function
hxnu_resume_with_cr3:
    mov rsi, rdi
    mov rax, [rsi + 0x38]
    mov rdx, cr3
    cmp rax, rdx
    je 1f
    mov cr3, rax
1:
    jmp hxnu_resume_common

hxnu_resume_common:
    mov rax, [rsi + 0x40]
    cmp rax, 1
    je hxnu_resume_user_context

    mov rsp, [rsi + 0x00]
    mov rbx, [rsi + 0x08]
    mov rbp, [rsi + 0x10]
    mov r12, [rsi + 0x18]
    mov r13, [rsi + 0x20]
    mov r14, [rsi + 0x28]
    mov r15, [rsi + 0x30]
    ret

hxnu_resume_user_context:
    mov rsp, [rsi + 0x00]
    pop rax
    pop rbp
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15
    iretq
"#
);
