use core::arch::global_asm;

use crate::vector;

const STACK_ALIGNMENT: usize = 16;
const VECTOR_CONTEXT_STORAGE_BYTES: usize = 4096;
const VECTOR_CONTEXT_ALIGNMENT: usize = 64;
const VECTOR_CONTEXT_BUFFER_BYTES: usize = VECTOR_CONTEXT_STORAGE_BYTES + VECTOR_CONTEXT_ALIGNMENT;

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
    vector_area: [u8; VECTOR_CONTEXT_BUFFER_BYTES],
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
            vector_area: [0; VECTOR_CONTEXT_BUFFER_BYTES],
        }
    }

    fn vector_area_ptr(&self) -> *const u8 {
        align_up_ptr(self.vector_area.as_ptr())
    }

    fn vector_area_mut_ptr(&mut self) -> *mut u8 {
        align_up_mut_ptr(self.vector_area.as_mut_ptr())
    }
}

pub fn initialize_kernel_thread(
    context: &mut TaskContext,
    stack: &'static mut [u8],
    entry: extern "C" fn() -> !,
) {
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
            ..TaskContext::empty()
        };
    }
}

pub unsafe fn switch(current: &mut TaskContext, next: &TaskContext) -> ! {
    unsafe {
        save_vector_context(current);
        restore_vector_context(next);
        hxnu_context_switch(current as *mut TaskContext, next as *const TaskContext);
    }
}

unsafe fn save_vector_context(context: &mut TaskContext) {
    match vector::context_mode() {
        vector::VectorContextMode::ScalarOnly
        | vector::VectorContextMode::Aarch64FpSimd
        | vector::VectorContextMode::Ppc64Vsx => {}
        vector::VectorContextMode::Fxsave => unsafe { fxsave(context.vector_area_mut_ptr()) },
        vector::VectorContextMode::Xsave => unsafe {
            xsave(context.vector_area_mut_ptr(), vector::xsave_mask())
        },
    }
}

unsafe fn restore_vector_context(context: &TaskContext) {
    match vector::context_mode() {
        vector::VectorContextMode::ScalarOnly
        | vector::VectorContextMode::Aarch64FpSimd
        | vector::VectorContextMode::Ppc64Vsx => {}
        vector::VectorContextMode::Fxsave => unsafe { fxrstor(context.vector_area_ptr()) },
        vector::VectorContextMode::Xsave => unsafe { xrstor(context.vector_area_ptr(), vector::xsave_mask()) },
    }
}

#[inline(always)]
fn align_up_ptr(ptr: *const u8) -> *const u8 {
    let address = ptr as usize;
    let aligned = (address + (VECTOR_CONTEXT_ALIGNMENT - 1)) & !(VECTOR_CONTEXT_ALIGNMENT - 1);
    aligned as *const u8
}

#[inline(always)]
fn align_up_mut_ptr(ptr: *mut u8) -> *mut u8 {
    let address = ptr as usize;
    let aligned = (address + (VECTOR_CONTEXT_ALIGNMENT - 1)) & !(VECTOR_CONTEXT_ALIGNMENT - 1);
    aligned as *mut u8
}

#[inline(always)]
unsafe fn fxsave(area: *mut u8) {
    unsafe {
        core::arch::asm!(
            "fxsave64 [{}]",
            in(reg) area,
            options(nostack, preserves_flags),
        );
    }
}

#[inline(always)]
unsafe fn fxrstor(area: *const u8) {
    unsafe {
        core::arch::asm!(
            "fxrstor64 [{}]",
            in(reg) area,
            options(nostack, preserves_flags),
        );
    }
}

#[inline(always)]
unsafe fn xsave(area: *mut u8, mask: u64) {
    let eax = mask as u32;
    let edx = (mask >> 32) as u32;
    unsafe {
        core::arch::asm!(
            "xsave [{}]",
            in(reg) area,
            in("eax") eax,
            in("edx") edx,
            options(nostack, preserves_flags),
        );
    }
}

#[inline(always)]
unsafe fn xrstor(area: *const u8, mask: u64) {
    let eax = mask as u32;
    let edx = (mask >> 32) as u32;
    unsafe {
        core::arch::asm!(
            "xrstor [{}]",
            in(reg) area,
            in("eax") eax,
            in("edx") edx,
            options(nostack, preserves_flags),
        );
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
    fn hxnu_context_switch(current: *mut TaskContext, next: *const TaskContext) -> !;
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

    mov rsp, [rsi + 0x00]
    mov rbx, [rsi + 0x08]
    mov rbp, [rsi + 0x10]
    mov r12, [rsi + 0x18]
    mov r13, [rsi + 0x20]
    mov r14, [rsi + 0x28]
    mov r15, [rsi + 0x30]
    ret
"#
);
