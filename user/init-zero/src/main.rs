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
const HXNU_SYS_READ: u64 = 0x484e_0008;
const HXNU_SYS_CLOSE: u64 = 0x484e_0009;
const HXNU_SYS_SEEK: u64 = 0x484e_000a;
const HXNU_SYS_ACCESS: u64 = 0x484e_0010;
const HXNU_SYS_DUP: u64 = 0x484e_0012;
const HXNU_SYS_DUP3: u64 = 0x484e_0013;
const HXNU_SYS_FCNTL: u64 = 0x484e_0014;
const HXNU_SYS_PWRITE64: u64 = 0x484e_0029;
const HXNU_SYS_RT_SIGACTION: u64 = 0x484e_0026;
const HXNU_SYS_WAIT4: u64 = 0x484e_002c;
const HXNU_SYS_PRCTL: u64 = 0x484e_0034;
const HXNU_SYS_FORK: u64 = 0x484e_003e;
const HXNU_SYS_EXEC: u64 = 0x484e_0040;
const HXNU_SYS_UNLINK: u64 = 0x484e_0042;
const HXNU_SYS_RENAME: u64 = 0x484e_0043;
const PR_GET_NAME: u64 = 16;
const TASK_COMM_LEN: usize = 16;
const RT_SIGSET_SIZE: u64 = 8;
const SIGCHLD: u64 = 17;
const WNOHANG: u64 = 1;
const F_GETFD: u64 = 1;
const F_SETFD: u64 = 2;
const F_GETFL: u64 = 3;
const F_SETFL: u64 = 4;
const FD_CLOEXEC: u64 = 1;
const F_OK: u64 = 0;
const O_RDONLY: u64 = 0;
const O_WRONLY: u64 = 1;
const O_RDWR: u64 = 2;
const O_CREAT: u64 = 0x40;
const O_APPEND: u64 = 0x400;
const O_CLOEXEC: u64 = 0x80000;
const O_TRUNC: u64 = 0x200;
const SEEK_SET: u64 = 0;
const INIT_PATH: &[u8] = b"/initrd/init\0";
const INIT_ARG0: &[u8] = b"/initrd/init\0";
const INIT_ARG1_REEXEC: &[u8] = b"--reexec\0";
const REEXEC_MARKER_PATH: &[u8] = b"/run/init-zero.reexec\0";
const CLOEXEC_SMOKE_PATH: &[u8] = b"/run/init-zero.cloexec\0";
const TMPFS_SMOKE_SOURCE_PATH: &[u8] = b"/run/init-zero-smoke.a\0";
const TMPFS_SMOKE_DESTINATION_PATH: &[u8] = b"/run/init-zero-smoke.b\0";
const DUP_SMOKE_PATH: &[u8] = b"/run/init-zero-dup\0";
const SIGNALS_PROC_PATH: &[u8] = b"/proc/signals\0";

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    log_static("HXNU-INIT: panic\n");
    loop {
        spin_loop();
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxKernelSigAction {
    handler: u64,
    flags: u64,
    restorer: u64,
    mask: u64,
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
        run_dup_smoke();
        run_signal_smoke();
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

fn syscall4(number: u64, arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> i64 {
    syscall(number, [arg0, arg1, arg2, arg3, 0, 0])
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

    let cloexec_fd = match arm_cloexec_smoke() {
        Ok(fd) => fd,
        Err(errno) => {
            let mut line = StackLine::<160>::new();
            let _ = write!(
                &mut line,
                "HXNU-INIT: cloexec smoke arm failed errno={}\n",
                errno,
            );
            log_bytes(line.as_bytes());
            -1
        }
    };

    let mut line = StackLine::<160>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: self-exec requested path=/initrd/init cloexec-fd={}\n",
        cloexec_fd,
    );
    log_bytes(line.as_bytes());
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
    if cloexec_fd >= 0 {
        let _ = syscall1(HXNU_SYS_CLOSE, cloexec_fd as u64);
    }
    "exec-failed"
}

fn arm_cloexec_smoke() -> Result<i64, i64> {
    let fd = syscall2(
        HXNU_SYS_OPEN,
        CLOEXEC_SMOKE_PATH.as_ptr() as u64,
        O_WRONLY | O_CREAT | O_TRUNC,
    );
    if fd < 0 {
        return Err(fd);
    }

    let setfd_result = syscall3(HXNU_SYS_FCNTL, fd as u64, F_SETFD, FD_CLOEXEC);
    if setfd_result < 0 {
        let _ = syscall1(HXNU_SYS_CLOSE, fd as u64);
        return Err(setfd_result);
    }

    Ok(fd)
}

fn run_tmpfs_smoke() {
    let source_fd = syscall2(
        HXNU_SYS_OPEN,
        TMPFS_SMOKE_SOURCE_PATH.as_ptr() as u64,
        O_RDWR | O_CREAT | O_TRUNC,
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

    let initial_payload = b"tmpfs-initial";
    let initial_write = syscall4(
        HXNU_SYS_PWRITE64,
        source_fd as u64,
        initial_payload.as_ptr() as u64,
        initial_payload.len() as u64,
        0,
    );

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
    let persisted_payload = b"+persist";
    let persisted_write = syscall4(
        HXNU_SYS_PWRITE64,
        source_fd as u64,
        persisted_payload.as_ptr() as u64,
        persisted_payload.len() as u64,
        initial_payload.len() as u64,
    );
    let seek_result = syscall3(HXNU_SYS_SEEK, source_fd as u64, 0, SEEK_SET);
    let mut readback = [0u8; 32];
    let readback_result = syscall3(
        HXNU_SYS_READ,
        source_fd as u64,
        readback.as_mut_ptr() as u64,
        readback.len() as u64,
    );
    let _ = syscall1(HXNU_SYS_CLOSE, source_fd as u64);

    let expected_payload = b"tmpfs-initial+persist";
    let payload_ok = initial_write == initial_payload.len() as i64
        && persisted_write == persisted_payload.len() as i64
        && seek_result == 0
        && readback_result == expected_payload.len() as i64
        && bytes_eq(&readback[..expected_payload.len()], expected_payload);

    let mut line = StackLine::<320>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: tmpfs smoke rename={} source-access={} dest-access={} unlink={} removed-access={} pwrite0={} pwrite-unlinked={} seek={} read={} payload-ok={}\n",
        rename_result,
        source_access,
        destination_access,
        unlink_result,
        removed_access,
        initial_write,
        persisted_write,
        seek_result,
        readback_result,
        yes_no(payload_ok),
    );
    log_bytes(line.as_bytes());
}

fn run_dup_smoke() {
    let source_fd = syscall2(HXNU_SYS_OPEN, DUP_SMOKE_PATH.as_ptr() as u64, O_RDWR | O_CREAT | O_TRUNC);
    if source_fd < 0 {
        let mut line = StackLine::<160>::new();
        let _ = write!(
            &mut line,
            "HXNU-INIT: dup smoke open failed errno={}\n",
            source_fd,
        );
        log_bytes(line.as_bytes());
        return;
    }

    let payload = b"dup-shared";
    let seed_write = syscall4(
        HXNU_SYS_PWRITE64,
        source_fd as u64,
        payload.as_ptr() as u64,
        payload.len() as u64,
        0,
    );
    let rewind_result = syscall3(HXNU_SYS_SEEK, source_fd as u64, 0, SEEK_SET);
    let dup_fd = syscall1(HXNU_SYS_DUP, source_fd as u64);

    let mut head = [0u8; 4];
    let head_read = syscall3(HXNU_SYS_READ, source_fd as u64, head.as_mut_ptr() as u64, head.len() as u64);
    let mut tail = [0u8; 6];
    let tail_read = if dup_fd >= 0 {
        syscall3(HXNU_SYS_READ, dup_fd as u64, tail.as_mut_ptr() as u64, tail.len() as u64)
    } else {
        dup_fd
    };

    let setfl_result = if dup_fd >= 0 {
        syscall3(HXNU_SYS_FCNTL, dup_fd as u64, F_SETFL, O_APPEND)
    } else {
        dup_fd
    };
    let source_getfl = syscall3(HXNU_SYS_FCNTL, source_fd as u64, F_GETFL, 0);
    let source_getfd = syscall3(HXNU_SYS_FCNTL, source_fd as u64, F_GETFD, 0);
    let dup3_fd = syscall3(HXNU_SYS_DUP3, source_fd as u64, 42, O_CLOEXEC);
    let dup3_getfd = if dup3_fd >= 0 {
        syscall3(HXNU_SYS_FCNTL, dup3_fd as u64, F_GETFD, 0)
    } else {
        dup3_fd
    };

    if dup3_fd >= 0 {
        let _ = syscall1(HXNU_SYS_CLOSE, dup3_fd as u64);
    }
    if dup_fd >= 0 {
        let _ = syscall1(HXNU_SYS_CLOSE, dup_fd as u64);
    }
    let _ = syscall1(HXNU_SYS_CLOSE, source_fd as u64);

    let shared_offset = seed_write == payload.len() as i64
        && rewind_result == 0
        && head_read == head.len() as i64
        && tail_read == tail.len() as i64
        && bytes_eq(&head, b"dup-")
        && bytes_eq(&tail, b"shared");
    let shared_status = setfl_result == 0 && source_getfl >= 0 && (source_getfl as u64 & O_APPEND != 0);
    let cloexec_split =
        source_getfd == 0 && dup3_getfd == FD_CLOEXEC as i64;

    let mut line = StackLine::<256>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: dup smoke dup={} dup3={} read1={} read2={} getfl={} setfl={} srcfd={} dup3fd={} shared-offset={} shared-status={} cloexec-split={}\n",
        dup_fd,
        dup3_fd,
        head_read,
        tail_read,
        source_getfl,
        setfl_result,
        source_getfd,
        dup3_getfd,
        yes_no(shared_offset),
        yes_no(shared_status),
        yes_no(cloexec_split),
    );
    log_bytes(line.as_bytes());
}

fn run_signal_smoke() {
    let action = LinuxKernelSigAction {
        handler: 0x51_47_43_48_4c_44,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let sigaction_result = syscall4(
        HXNU_SYS_RT_SIGACTION,
        SIGCHLD,
        (&action as *const LinuxKernelSigAction) as u64,
        0,
        RT_SIGSET_SIZE,
    );
    if sigaction_result < 0 {
        let mut line = StackLine::<160>::new();
        let _ = write!(
            &mut line,
            "HXNU-INIT: signal smoke sigaction failed errno={}\n",
            sigaction_result,
        );
        log_bytes(line.as_bytes());
        return;
    }

    let fork_result = syscall0(HXNU_SYS_FORK);
    if fork_result < 0 {
        let mut line = StackLine::<160>::new();
        let _ = write!(
            &mut line,
            "HXNU-INIT: signal smoke fork failed errno={}\n",
            fork_result,
        );
        log_bytes(line.as_bytes());
        return;
    }

    let mut before_buffer = [0u8; 512];
    let before_len = read_proc_snapshot(SIGNALS_PROC_PATH, &mut before_buffer);
    let before_pending = before_len >= 0
        && snapshot_contains(&before_buffer[..before_len as usize], b"sigchld_pending yes");
    let before_handler = before_len >= 0
        && snapshot_contains(&before_buffer[..before_len as usize], b"sigchld_disposition handler");

    let mut wait_status = 0i32;
    let wait_result = syscall4(
        HXNU_SYS_WAIT4,
        fork_result as u64,
        (&mut wait_status as *mut i32) as u64,
        WNOHANG,
        0,
    );

    let mut after_buffer = [0u8; 512];
    let after_len = read_proc_snapshot(SIGNALS_PROC_PATH, &mut after_buffer);
    let after_pending = after_len >= 0
        && snapshot_contains(&after_buffer[..after_len as usize], b"sigchld_pending yes");

    let mut line = StackLine::<224>::new();
    let _ = write!(
        &mut line,
        "HXNU-INIT: signal smoke sigaction={} fork={} pending-before={} handler={} wait4={} status={} pending-after={}\n",
        sigaction_result,
        fork_result,
        yes_no(before_pending),
        yes_no(before_handler),
        wait_result,
        wait_status,
        yes_no(after_pending),
    );
    log_bytes(line.as_bytes());
}

fn read_proc_snapshot(path: &[u8], buffer: &mut [u8]) -> i64 {
    let fd = syscall2(HXNU_SYS_OPEN, path.as_ptr() as u64, O_RDONLY);
    if fd < 0 {
        return fd;
    }

    let read_result = syscall3(
        HXNU_SYS_READ,
        fd as u64,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    );
    let _ = syscall1(HXNU_SYS_CLOSE, fd as u64);
    read_result
}

fn snapshot_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }

    let mut offset = 0usize;
    while offset + needle.len() <= haystack.len() {
        if &haystack[offset..offset + needle.len()] == needle {
            return true;
        }
        offset += 1;
    }
    false
}

fn bytes_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }

    let mut index = 0usize;
    while index < lhs.len() {
        if lhs[index] != rhs[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
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
