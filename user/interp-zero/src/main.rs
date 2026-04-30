#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::ffi::{c_int, c_void};
use core::fmt::{self, Write};
use core::hint::spin_loop;
use core::panic::PanicInfo;
use core::ptr;
use core::str;

const ABI_HXNU_NATIVE_BOOTSTRAP: u64 = 2;
const HXNU_SYS_LOG_WRITE: u64 = 0x484e_0001;
const HXNU_SYS_THREAD_SELF: u64 = 0x484e_0002;
const HXNU_SYS_PROCESS_SELF: u64 = 0x484e_0003;
const HXNU_SYS_UPTIME_NSEC: u64 = 0x484e_0004;
const HXNU_SYS_SCHED_YIELD: u64 = 0x484e_0005;
const HXNU_SYS_ABI_VERSION: u64 = 0x484e_0006;
const AUX_AT_NULL: u64 = 0;
const AUX_AT_PHDR: u64 = 3;
const AUX_AT_PHENT: u64 = 4;
const AUX_AT_PHNUM: u64 = 5;
const AUX_AT_BASE: u64 = 7;
const AUX_AT_ENTRY: u64 = 9;
const AUX_AT_EXECFN: u64 = 31;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    log_static("HXNU-INTERP: panic\n");
    loop {
        spin_loop();
    }
}

#[derive(Copy, Clone)]
struct InitialStackView {
    argc: usize,
    argv0: *const u8,
    at_entry: u64,
    at_base: u64,
    at_phdr: u64,
    at_phent: u64,
    at_phnum: u64,
    execfn: *const u8,
}

global_asm!(
    r#"
    .global _start
    .type _start,@function
_start:
    mov rdi, rsp
    jmp {entry}
"#,
    entry = sym interp_entry,
);

#[unsafe(no_mangle)]
extern "C" fn interp_entry(initial_stack_pointer: u64) -> ! {
    let abi = syscall0(HXNU_SYS_ABI_VERSION);
    let process_id = syscall0(HXNU_SYS_PROCESS_SELF);
    let thread_id = syscall0(HXNU_SYS_THREAD_SELF);
    let uptime_ns = syscall0(HXNU_SYS_UPTIME_NSEC);
    let stack = unsafe { parse_initial_stack(initial_stack_pointer as *const u64) };
    let argv0 = display_c_string(stack.argv0);
    let execfn = display_c_string(stack.execfn);

    let mut line = StackLine::<320>::new();
    let _ = write!(
        &mut line,
        "HXNU-INTERP: abi={:#x} pid={} tid={} argc={} argv0={} execfn={} at-entry={:#018x} at-base={:#018x} at-phdr={:#018x} at-phent={} at-phnum={} rsp={:#018x} uptime-ns={}\n",
        abi,
        process_id,
        thread_id,
        stack.argc,
        argv0,
        execfn,
        stack.at_entry,
        stack.at_base,
        stack.at_phdr,
        stack.at_phent,
        stack.at_phnum,
        initial_stack_pointer,
        uptime_ns,
    );
    log_bytes(line.as_bytes());

    loop {
        let _ = syscall0(HXNU_SYS_SCHED_YIELD);
        spin_loop();
    }
}

unsafe fn parse_initial_stack(mut cursor: *const u64) -> InitialStackView {
    let argc = unsafe { *cursor as usize };
    cursor = unsafe { cursor.add(1) };

    let argv0 = if argc != 0 {
        unsafe { *cursor as *const u8 }
    } else {
        ptr::null()
    };
    cursor = unsafe { cursor.add(argc) };
    cursor = unsafe { cursor.add(1) };

    while unsafe { *cursor } != 0 {
        cursor = unsafe { cursor.add(1) };
    }
    cursor = unsafe { cursor.add(1) };

    let mut view = InitialStackView {
        argc,
        argv0,
        at_entry: 0,
        at_base: 0,
        at_phdr: 0,
        at_phent: 0,
        at_phnum: 0,
        execfn: ptr::null(),
    };

    loop {
        let key = unsafe { *cursor };
        let value = unsafe { *cursor.add(1) };
        cursor = unsafe { cursor.add(2) };
        if key == AUX_AT_NULL {
            break;
        }

        match key {
            AUX_AT_PHDR => view.at_phdr = value,
            AUX_AT_PHENT => view.at_phent = value,
            AUX_AT_PHNUM => view.at_phnum = value,
            AUX_AT_BASE => view.at_base = value,
            AUX_AT_ENTRY => view.at_entry = value,
            AUX_AT_EXECFN => view.execfn = value as *const u8,
            _ => {}
        }
    }

    view
}

fn display_c_string<'a>(ptr: *const u8) -> &'a str {
    if ptr.is_null() {
        return "<none>";
    }

    let bytes = unsafe { c_string_bytes(ptr, 256) };
    str::from_utf8(bytes).unwrap_or("<invalid>")
}

unsafe fn c_string_bytes<'a>(ptr: *const u8, max_len: usize) -> &'a [u8] {
    let mut len = 0usize;
    while len < max_len {
        if unsafe { *ptr.add(len) } == 0 {
            break;
        }
        len += 1;
    }
    unsafe { core::slice::from_raw_parts(ptr, len) }
}

fn log_static(text: &str) {
    log_bytes(text.as_bytes());
}

fn log_bytes(bytes: &[u8]) {
    let _ = syscall2(HXNU_SYS_LOG_WRITE, bytes.as_ptr() as u64, bytes.len() as u64);
}

fn syscall0(number: u64) -> i64 {
    syscall(number, [0; 6])
}

fn syscall2(number: u64, arg0: u64, arg1: u64) -> i64 {
    syscall(number, [arg0, arg1, 0, 0, 0, 0])
}

fn syscall(number: u64, args: [u64; 6]) -> i64 {
    let mut result = number;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("r12") ABI_HXNU_NATIVE_BOOTSTRAP,
            in("rdi") args[0],
            in("rsi") args[1],
            in("rdx") args[2],
            in("r10") args[3],
            in("r8") args[4],
            in("r9") args[5],
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
    result as i64
}

struct StackLine<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> StackLine<N> {
    const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl<const N: usize> Write for StackLine<N> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let remaining = N.saturating_sub(self.len);
        if text.len() > remaining {
            return Err(fmt::Error);
        }
        let end = self.len + text.len();
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, len: usize) -> *mut c_void {
    let dest = dest.cast::<u8>();
    let src = src.cast::<u8>();

    let mut index = 0usize;
    while index < len {
        unsafe {
            *dest.add(index) = *src.add(index);
        }
        index += 1;
    }

    dest.cast::<c_void>()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut c_void, value: c_int, len: usize) -> *mut c_void {
    let dest = dest.cast::<u8>();
    let byte = value as u8;

    let mut index = 0usize;
    while index < len {
        unsafe {
            *dest.add(index) = byte;
        }
        index += 1;
    }

    dest.cast::<c_void>()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(lhs: *const c_void, rhs: *const c_void, len: usize) -> c_int {
    let lhs = lhs.cast::<u8>();
    let rhs = rhs.cast::<u8>();

    let mut index = 0usize;
    while index < len {
        let left = unsafe { *lhs.add(index) };
        let right = unsafe { *rhs.add(index) };
        if left != right {
            return i32::from(left) - i32::from(right);
        }
        index += 1;
    }

    0
}
