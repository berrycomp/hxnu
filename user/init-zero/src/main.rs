#![no_std]
#![no_main]

use core::arch::asm;
use core::ffi::{c_int, c_void};
use core::fmt::{self, Write};
use core::hint::spin_loop;
use core::panic::PanicInfo;
use core::str;

const ABI_HXNU_NATIVE_BOOTSTRAP: u64 = 2;
const HXNU_SYS_LOG_WRITE: u64 = 0x484e_0001;
const HXNU_SYS_THREAD_SELF: u64 = 0x484e_0002;
const HXNU_SYS_PROCESS_SELF: u64 = 0x484e_0003;
const HXNU_SYS_UPTIME_NSEC: u64 = 0x484e_0004;
const HXNU_SYS_SCHED_YIELD: u64 = 0x484e_0005;
const HXNU_SYS_ABI_VERSION: u64 = 0x484e_0006;
const HXNU_SYS_OPEN: u64 = 0x484e_0007;
const HXNU_SYS_CLOSE: u64 = 0x484e_0009;
const HXNU_SYS_ACCESS: u64 = 0x484e_0010;
const HXNU_SYS_PRCTL: u64 = 0x484e_0034;
const HXNU_SYS_EXEC: u64 = 0x484e_0040;
const HXNU_SYS_UNLINK: u64 = 0x484e_0042;
const HXNU_SYS_RENAME: u64 = 0x484e_0043;
const PR_GET_NAME: u64 = 16;
const TASK_COMM_LEN: usize = 16;
const F_OK: u64 = 0;
const O_WRONLY: u64 = 1;
const O_CREAT: u64 = 0x40;
const O_TRUNC: u64 = 0x200;
const INIT_PATH: &[u8] = b"/initrd/init\0";
const INIT_ARG0: &[u8] = b"/initrd/init\0";
const INIT_ARG1_REEXEC: &[u8] = b"--reexec\0";
const REEXEC_MARKER_PATH: &[u8] = b"/run/init-zero.reexec\0";
const TMPFS_SMOKE_SOURCE_PATH: &[u8] = b"/run/init-zero-smoke.a\0";
const TMPFS_SMOKE_DESTINATION_PATH: &[u8] = b"/run/init-zero-smoke.b\0";

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

    let reexec_stage = maybe_reexec_once();

    let abi = syscall0(HXNU_SYS_ABI_VERSION);
    let process_id = syscall0(HXNU_SYS_PROCESS_SELF);
    let thread_id = syscall0(HXNU_SYS_THREAD_SELF);
    let uptime_ns = syscall0(HXNU_SYS_UPTIME_NSEC);
    let mut comm_name = [0u8; TASK_COMM_LEN];
    let comm_result = syscall2(HXNU_SYS_PRCTL, PR_GET_NAME, comm_name.as_mut_ptr() as u64);
    let comm_len = c_string_len(&comm_name);
    let comm_text = if comm_result == 0 {
        str::from_utf8(&comm_name[..comm_len]).unwrap_or("<invalid>")
    } else {
        "<unavailable>"
    };

    let mut line = StackLine::<192>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: abi={:#x} pid={} tid={} comm={} stage={} uptime-ns={}\n",
        abi,
        process_id,
        thread_id,
        comm_text,
        reexec_stage,
        uptime_ns,
    );
    log_bytes(line.as_bytes());

    if reexec_stage == "post-exec" {
        run_tmpfs_smoke();
    }

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

fn syscall1(number: u64, arg0: u64) -> i64 {
    syscall(number, [arg0, 0, 0, 0, 0, 0])
}

fn syscall2(number: u64, arg0: u64, arg1: u64) -> i64 {
    syscall(number, [arg0, arg1, 0, 0, 0, 0])
}

fn syscall3(number: u64, arg0: u64, arg1: u64, arg2: u64) -> i64 {
    syscall(number, [arg0, arg1, arg2, 0, 0, 0])
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

fn c_string_len(bytes: &[u8]) -> usize {
    let mut len = 0usize;
    while len < bytes.len() && bytes[len] != 0 {
        len += 1;
    }
    len
}

fn maybe_reexec_once() -> &'static str {
    if syscall2(HXNU_SYS_ACCESS, REEXEC_MARKER_PATH.as_ptr() as u64, F_OK) == 0 {
        return "post-exec";
    }

    let marker_fd = syscall2(
        HXNU_SYS_OPEN,
        REEXEC_MARKER_PATH.as_ptr() as u64,
        O_WRONLY | O_CREAT | O_TRUNC,
    );
    if marker_fd < 0 {
        let mut line = StackLine::<160>::new();
        let _ = write!(
            &mut line,
            "HXNU-INIT: reexec marker create failed errno={}\n",
            marker_fd,
        );
        log_bytes(line.as_bytes());
        return "marker-failed";
    }
    let _ = syscall1(HXNU_SYS_CLOSE, marker_fd as u64);

    log_static("HXNU-INIT: self-exec requested path=/initrd/init\n");
    let argv = [
        INIT_ARG0.as_ptr() as u64,
        INIT_ARG1_REEXEC.as_ptr() as u64,
        0,
    ];
    let exec_result = syscall3(
        HXNU_SYS_EXEC,
        INIT_PATH.as_ptr() as u64,
        argv.as_ptr() as u64,
        0,
    );
    let mut line = StackLine::<160>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: self-exec returned errno={}\n",
        exec_result,
    );
    log_bytes(line.as_bytes());
    "exec-failed"
}

fn run_tmpfs_smoke() {
    let source_fd = syscall2(
        HXNU_SYS_OPEN,
        TMPFS_SMOKE_SOURCE_PATH.as_ptr() as u64,
        O_WRONLY | O_CREAT | O_TRUNC,
    );
    if source_fd < 0 {
        let mut line = StackLine::<160>::new();
        let _ = write!(
            &mut line,
            "HXNU-INIT: tmpfs smoke open failed errno={}\n",
            source_fd,
        );
        log_bytes(line.as_bytes());
        return;
    }
    let _ = syscall1(HXNU_SYS_CLOSE, source_fd as u64);

    let rename_result = syscall2(
        HXNU_SYS_RENAME,
        TMPFS_SMOKE_SOURCE_PATH.as_ptr() as u64,
        TMPFS_SMOKE_DESTINATION_PATH.as_ptr() as u64,
    );
    if rename_result < 0 {
        let mut line = StackLine::<160>::new();
        let _ = write!(
            &mut line,
            "HXNU-INIT: tmpfs smoke rename failed errno={}\n",
            rename_result,
        );
        log_bytes(line.as_bytes());
        return;
    }

    let source_access = syscall2(HXNU_SYS_ACCESS, TMPFS_SMOKE_SOURCE_PATH.as_ptr() as u64, F_OK);
    let destination_access =
        syscall2(HXNU_SYS_ACCESS, TMPFS_SMOKE_DESTINATION_PATH.as_ptr() as u64, F_OK);
    let unlink_result = syscall1(HXNU_SYS_UNLINK, TMPFS_SMOKE_DESTINATION_PATH.as_ptr() as u64);
    let removed_access = syscall2(HXNU_SYS_ACCESS, TMPFS_SMOKE_DESTINATION_PATH.as_ptr() as u64, F_OK);

    let mut line = StackLine::<192>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: tmpfs smoke rename={} source-access={} dest-access={} unlink={} removed-access={}\n",
        rename_result,
        source_access,
        destination_access,
        unlink_result,
        removed_access,
    );
    log_bytes(line.as_bytes());
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
