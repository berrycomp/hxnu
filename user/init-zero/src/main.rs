#![no_std]
#![no_main]

use core::arch::asm;
use core::ffi::{c_int, c_void};
use core::fmt::{self, Write};
use core::hint::spin_loop;
use core::panic::PanicInfo;

const ABI_HXNU_NATIVE_BOOTSTRAP: u64 = 2;
const HXNU_SYS_LOG_WRITE: u64 = 0x484e_0001;
const HXNU_SYS_THREAD_SELF: u64 = 0x484e_0002;
const HXNU_SYS_PROCESS_SELF: u64 = 0x484e_0003;
const HXNU_SYS_UPTIME_NSEC: u64 = 0x484e_0004;
const HXNU_SYS_SCHED_YIELD: u64 = 0x484e_0005;
const HXNU_SYS_ABI_VERSION: u64 = 0x484e_0006;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    log_static("HXNU-INIT: panic\n");
    loop {
        spin_loop();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log_static("HXNU-INIT: payload online via int 0x80\n");

    let abi = syscall0(HXNU_SYS_ABI_VERSION);
    let process_id = syscall0(HXNU_SYS_PROCESS_SELF);
    let thread_id = syscall0(HXNU_SYS_THREAD_SELF);
    let uptime_ns = syscall0(HXNU_SYS_UPTIME_NSEC);

    let mut line = StackLine::<160>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: abi={:#x} pid={} tid={} uptime-ns={}\n",
        abi,
        process_id,
        thread_id,
        uptime_ns,
    );
    log_bytes(line.as_bytes());

    loop {
        let _ = syscall0(HXNU_SYS_SCHED_YIELD);
        spin_loop();
    }
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
