// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
// HXNU Public License (HPL)
use core::cell::UnsafeCell;

pub struct LclMessage {
    pub pid: u64,
    pub tid: u64,
    pub number: u64,
    pub args: [u64; 6],
    pub abi: u64,
}
pub struct LclQueue {
    pub messages: [Option<LclMessage>; 64],
    pub head: usize,
    pub tail: usize,
}
pub static mut LCL_QUEUE: LclQueue = LclQueue {
    messages: [
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
    ],
    head: 0,
    tail: 0,
};

pub fn enqueue_lcl_message(msg: LclMessage) {
    unsafe {
        LCL_QUEUE.messages[LCL_QUEUE.head] = Some(msg);
        LCL_QUEUE.head = (LCL_QUEUE.head + 1) % 64;
    }
}

pub fn dequeue_lcl_message() -> Option<LclMessage> {
    unsafe {
        if LCL_QUEUE.head == LCL_QUEUE.tail {
            if LCL_QUEUE.messages[LCL_QUEUE.tail].is_none() {
                return None;
            }
        }
        let msg = LCL_QUEUE.messages[LCL_QUEUE.tail].take();
        if msg.is_some() {
            LCL_QUEUE.tail = (LCL_QUEUE.tail + 1) % 64;
        }
        msg
    }
}

use core::cmp::min;
use core::mem::size_of_val;
use core::slice;
use core::str;

use crate::sched;
use crate::time;
use crate::tty;
use crate::uaccess::{self, UserCopyError};
use crate::vfs;
use crate::vfs::VfsNodeKind;

pub const LINUX_ABI_NAME: &str = "linux-x86_64-bootstrap";
pub const GHOST_ABI_NAME: &str = "ghost-bootstrap";
pub const HXNU_ABI_NAME: &str = "hxnu-native-bootstrap";

pub const LINUX_SYS_READ: u64 = 0;
pub const LINUX_SYS_WRITE: u64 = 1;
pub const LINUX_SYS_OPEN: u64 = 2;
pub const LINUX_SYS_CLOSE: u64 = 3;
pub const LINUX_SYS_STAT: u64 = 4;
pub const LINUX_SYS_FSTAT: u64 = 5;
pub const LINUX_SYS_MMAP: u64 = 9;
pub const LINUX_SYS_MUNMAP: u64 = 11;
pub const LINUX_SYS_BRK: u64 = 12;
pub const LINUX_SYS_IOCTL: u64 = 16;
pub const LINUX_SYS_SCHED_YIELD: u64 = 24;
pub const LINUX_SYS_POLL: u64 = 7;
pub const LINUX_SYS_LSEEK: u64 = 8;
pub const LINUX_SYS_MPROTECT: u64 = 10;
pub const LINUX_SYS_RT_SIGACTION: u64 = 13;
pub const LINUX_SYS_RT_SIGPROCMASK: u64 = 14;
pub const LINUX_SYS_MADVISE: u64 = 28;
pub const LINUX_SYS_GETPID: u64 = 39;
pub const LINUX_SYS_EXIT: u64 = 60;
pub const LINUX_SYS_UNAME: u64 = 63;
pub const LINUX_SYS_GETPPID: u64 = 110;
pub const LINUX_SYS_SIGALTSTACK: u64 = 131;
pub const LINUX_SYS_ARCH_PRCTL: u64 = 158;
pub const LINUX_SYS_GETTID: u64 = 186;
pub const LINUX_SYS_TKILL: u64 = 200;
pub const LINUX_SYS_GETDENTS64: u64 = 217;
pub const LINUX_SYS_SET_TID_ADDRESS: u64 = 218;
pub const LINUX_SYS_CLOCK_GETTIME: u64 = 228;
pub const LINUX_SYS_EXIT_GROUP: u64 = 231;
pub const LINUX_SYS_TGKILL: u64 = 234;
pub const LINUX_SYS_OPENAT: u64 = 257;
pub const LINUX_SYS_NEWFSTATAT: u64 = 262;
pub const LINUX_SYS_PRLIMIT64: u64 = 302;
pub const LINUX_SYS_GETRANDOM: u64 = 318;
pub const LINUX_SYS_RSEQ: u64 = 334;

pub const ARCH_SET_FS: u64 = 0x1002;
pub const ARCH_GET_FS: u64 = 0x1003;
const EFAULT: i64 = 14;

pub const GHOST_SYS_WRITE: u64 = 1;
pub const GHOST_SYS_YIELD: u64 = 2;
pub const GHOST_SYS_GETPID: u64 = 3;
pub const GHOST_SYS_GETTID: u64 = 4;
pub const GHOST_SYS_UPTIME_NSEC: u64 = 5;
pub const GHOST_SYS_UNAME: u64 = 6;
pub const GHOST_SYS_EXIT_GROUP: u64 = 7;
pub const GHOST_SYS_OPEN: u64 = 8;
pub const GHOST_SYS_READ: u64 = 9;
pub const GHOST_SYS_CLOSE: u64 = 10;

pub const HXNU_SYS_LOG_WRITE: u64 = 0x484e_0001;
pub const HXNU_SYS_THREAD_SELF: u64 = 0x484e_0002;
pub const HXNU_SYS_PROCESS_SELF: u64 = 0x484e_0003;
pub const HXNU_SYS_UPTIME_NSEC: u64 = 0x484e_0004;
pub const HXNU_SYS_SCHED_YIELD: u64 = 0x484e_0005;
pub const HXNU_SYS_ABI_VERSION: u64 = 0x484e_0006;
pub const HXNU_SYS_OPEN: u64 = 0x484e_0007;
pub const HXNU_SYS_READ: u64 = 0x484e_0008;
pub const HXNU_SYS_CLOSE: u64 = 0x484e_0009;
pub const HXNU_SYS_LCL_RECEIVE: u64 = 0x484e_000a;
pub const HXNU_SYS_LCL_RETURN: u64 = 0x484e_000b;
pub const HXNU_SYS_SPAWN: u64 = 0x484e_000c;
pub const HXNU_SYS_HETEREXEC_SUBMIT: u64 = 0x484e_0010;
pub const HXNU_SYS_HETEREXEC_POLL: u64 = 0x484e_0011;
pub const HXNU_SYS_EXIT_GROUP: u64 = 0x484e_00ff;

const HXNU_NATIVE_ABI_VERSION: i64 = 0x0001_0000;
const LINUX_CLOCK_REALTIME: i32 = 0;
const LINUX_CLOCK_MONOTONIC: i32 = 1;
const AT_FDCWD: i64 = -100;

const O_ACCMODE: u64 = 0x3;
const O_RDONLY: u64 = 0;
const O_CREAT: u64 = 0x40;
const O_TRUNC: u64 = 0x200;
const O_APPEND: u64 = 0x400;

const MAX_WRITE_BYTES: usize = 16 * 1024;
const MAX_READ_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 1024;
const MAX_OPEN_FILES: usize = 64;

const EBADF: i64 = 9;
const EIO: i64 = 5;
const EINVAL: i64 = 22;
const ENOSYS: i64 = 38;
const ERANGE: i64 = 34;
const ENOENT: i64 = 2;
const EISDIR: i64 = 21;
const EMFILE: i64 = 24;

const STDOUT_FD: u64 = 1;
const STDERR_FD: u64 = 2;

#[derive(Copy, Clone)]
pub enum SyscallAbi {
    LinuxBootstrap,
    GhostBootstrap,
    HxnuNativeBootstrap,
    /// Routes the syscall to the Linux Compatibility Layer (LCL) daemon.
    PosixLcl,
}

impl SyscallAbi {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinuxBootstrap => LINUX_ABI_NAME,
            Self::GhostBootstrap => GHOST_ABI_NAME,
            Self::HxnuNativeBootstrap => HXNU_ABI_NAME,
            Self::PosixLcl => "posix-lcl",
        }
    }
}

#[derive(Copy, Clone)]
pub enum SyscallAction {
    Continue,
    YieldThread,
    ExitGroup { status: i32 },
}

#[derive(Copy, Clone)]
pub struct SyscallOutcome {
    pub value: i64,
    pub action: SyscallAction,
}

impl SyscallOutcome {
    const fn success(value: i64) -> Self {
        Self {
            value,
            action: SyscallAction::Continue,
        }
    }

    const fn errno(errno: i64) -> Self {
        Self {
            value: -errno,
            action: SyscallAction::Continue,
        }
    }
}

#[derive(Copy, Clone)]
pub struct LinuxBootstrapProbe {
    pub write_result: i64,
    pub openat_result: i64,
    pub read_result: i64,
    pub close_result: i64,
    pub getpid_result: i64,
    pub getppid_result: i64,
    pub gettid_result: i64,
    pub sched_yield_result: i64,
    pub clock_gettime_result: i64,
    pub clock_seconds: i64,
    pub clock_nanoseconds: i64,
    pub uname_result: i64,
    pub exit_group_captured: bool,
    pub exit_group_status: i32,
    machine_bytes: [u8; 16],
    machine_len: usize,
}

impl LinuxBootstrapProbe {
    pub fn machine_str(&self) -> &str {
        machine_str(&self.machine_bytes, self.machine_len)
    }
}

#[derive(Copy, Clone)]
pub struct GhostBootstrapProbe {
    pub write_result: i64,
    pub open_result: i64,
    pub read_result: i64,
    pub close_result: i64,
    pub getpid_result: i64,
    pub gettid_result: i64,
    pub yield_result: i64,
    pub uptime_result: i64,
    pub uname_result: i64,
    pub exit_group_captured: bool,
    pub exit_group_status: i32,
    machine_bytes: [u8; 16],
    machine_len: usize,
}

impl GhostBootstrapProbe {
    pub fn machine_str(&self) -> &str {
        machine_str(&self.machine_bytes, self.machine_len)
    }
}

#[derive(Copy, Clone)]
pub struct HxnuBootstrapProbe {
    pub write_result: i64,
    pub open_result: i64,
    pub read_result: i64,
    pub close_result: i64,
    pub process_self_result: i64,
    pub thread_self_result: i64,
    pub sched_yield_result: i64,
    pub uptime_result: i64,
    pub abi_version_result: i64,
    pub exit_group_captured: bool,
    pub exit_group_status: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxTimespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxUtsName {
    sysname: [u8; 65],
    nodename: [u8; 65],
    release: [u8; 65],
    version: [u8; 65],
    machine: [u8; 65],
    domainname: [u8; 65],
}

impl LinuxUtsName {
    const fn new() -> Self {
        Self {
            sysname: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
            domainname: [0; 65],
        }
    }
}

const MAX_SYSTEM_FILES: usize = 128;

#[derive(Clone)]
pub struct OpenFile {
    pub ref_count: usize,
    pub path: [u8; 128],
    pub path_len: usize,
    pub offset: usize,
    pub content: [u8; 4096],
    pub content_len: usize,
}

struct GlobalOpenFileTable(UnsafeCell<[Option<OpenFile>; MAX_SYSTEM_FILES]>);

unsafe impl Sync for GlobalOpenFileTable {}

impl GlobalOpenFileTable {
    const fn new() -> Self {
        const INIT_OPT: Option<OpenFile> = None;
        Self(UnsafeCell::new([INIT_OPT; MAX_SYSTEM_FILES]))
    }

    fn get(&self) -> *mut [Option<OpenFile>; MAX_SYSTEM_FILES] {
        self.0.get()
    }
}

static FILE_TABLE: GlobalOpenFileTable = GlobalOpenFileTable::new();


/// Primary syscall entry point for routing `int 0x80` hardware traps.
/// Interprets the `TrapFrame` register states and delegates to the appropriate
/// ABI backend: native bootstrapping or the `PosixLcl` compatible wrapper.
///
/// This routing layer serves as the absolute lowest level in the HXNU architecture
/// before hitting userspace. The Linux Compatibility Layer (LCL) daemon relies on
/// the `PosixLcl` path to intercept `int 0x80` traps and securely translate them
/// into bare-metal native HXNU/heterexec semantics, achieving zero-latency bridging.
pub fn syscall_handler(trap_frame: &mut crate::arch::x86_64::SyscallRegisterFrame) -> SyscallOutcome {
    let abi = match trap_frame.r12 {
        2 => SyscallAbi::HxnuNativeBootstrap,
        _ => SyscallAbi::PosixLcl,
    };
    match abi {
        SyscallAbi::HxnuNativeBootstrap => dispatch_hxnu_bootstrap(
            trap_frame.rax,
            [
                trap_frame.rdi,
                trap_frame.rsi,
                trap_frame.rdx,
                trap_frame.r10,
                trap_frame.r8,
                trap_frame.r9,
            ],
        ),
        SyscallAbi::PosixLcl => {
            crate::tty::write_str("[LCL Handler] Intercepted int 0x80\n");
            let pid = sched::current_process_id();
            let tid = sched::current_thread_id();
            enqueue_lcl_message(LclMessage { pid, tid, number: trap_frame.rax, args: [trap_frame.rdi, trap_frame.rsi, trap_frame.rdx, trap_frame.r10, trap_frame.r8, trap_frame.r9], abi: trap_frame.r12 });
            
            // Suspend thread and wait for LCL return
            sched::block_current_thread();
            SyscallOutcome { value: 0, action: SyscallAction::YieldThread }
        }
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub fn dispatch(abi: SyscallAbi, number: u64, args: [u64; 6]) -> SyscallOutcome {
    match abi {
        SyscallAbi::LinuxBootstrap => posix_compat::dispatch_linux_bootstrap(number, args),
        SyscallAbi::GhostBootstrap => posix_compat::dispatch_ghost_bootstrap(number, args),
        SyscallAbi::HxnuNativeBootstrap => dispatch_hxnu_bootstrap(number, args),
        SyscallAbi::PosixLcl => {
            crate::tty::write_str("[LCL Handler] Intercepted int 0x80\n");
            let pid = sched::current_process_id();
            let tid = sched::current_thread_id();
            enqueue_lcl_message(LclMessage { pid, tid, number, args, abi: 3 });
            sched::block_current_thread();
            SyscallOutcome { value: 0, action: SyscallAction::YieldThread }
        }
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub mod posix_compat {
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct stat {
        pub st_dev: u64,
        pub st_ino: u64,
        pub st_nlink: u64,
        pub st_mode: u32,
        pub st_uid: u32,
        pub st_gid: u32,
        pub __pad0: u32,
        pub st_rdev: u64,
        pub st_size: i64,
        pub st_blksize: i64,
        pub st_blocks: i64,
        pub st_atime: u64,
        pub st_atime_nsec: u64,
        pub st_mtime: u64,
        pub st_mtime_nsec: u64,
        pub st_ctime: u64,
        pub st_ctime_nsec: u64,
        pub __unused: [i64; 3],
    }
    use super::*;

    pub fn dispatch_linux_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
        match number {
            LINUX_SYS_READ => read_from_fd(args),
            LINUX_SYS_WRITE => write_with_fd(args),
            LINUX_SYS_OPEN => open_path_at(AT_FDCWD, args[0] as usize, args[1]),
            LINUX_SYS_CLOSE => close_fd(args),
            LINUX_SYS_STAT | LINUX_SYS_NEWFSTATAT => sys_stat(args),
            LINUX_SYS_FSTAT => sys_fstat(args),
            LINUX_SYS_POLL => SyscallOutcome::success(0),
            LINUX_SYS_LSEEK => SyscallOutcome::success(0),
            LINUX_SYS_MMAP => sys_mmap(args),
            LINUX_SYS_MPROTECT => SyscallOutcome::success(0),
            LINUX_SYS_MUNMAP => SyscallOutcome::success(0),
            LINUX_SYS_BRK => sys_brk(args),
            LINUX_SYS_RT_SIGACTION => SyscallOutcome::success(0),
            LINUX_SYS_RT_SIGPROCMASK => SyscallOutcome::success(0),
            LINUX_SYS_IOCTL => sys_ioctl(args),
            LINUX_SYS_SCHED_YIELD => SyscallOutcome::success(0),
            LINUX_SYS_MADVISE => SyscallOutcome::success(0),
            LINUX_SYS_GETPID => process_id(),
            LINUX_SYS_GETPPID => process_parent_id(),
            LINUX_SYS_SIGALTSTACK => SyscallOutcome::success(0),
            LINUX_SYS_ARCH_PRCTL => sys_arch_prctl(args),
            LINUX_SYS_GETTID => thread_id(),
            LINUX_SYS_TKILL => SyscallOutcome::success(0),
            LINUX_SYS_GETDENTS64 => sys_getdents64(args),
            LINUX_SYS_SET_TID_ADDRESS => SyscallOutcome::success(process_id().value),
            LINUX_SYS_CLOCK_GETTIME => linux_clock_gettime(args),
            LINUX_SYS_UNAME => linux_uname(args),
            LINUX_SYS_EXIT | LINUX_SYS_EXIT_GROUP => exit_group(args),
            LINUX_SYS_TGKILL => SyscallOutcome::success(0),
            LINUX_SYS_OPENAT => linux_openat(args),
            LINUX_SYS_PRLIMIT64 => SyscallOutcome::success(0),
            LINUX_SYS_GETRANDOM => sys_getrandom(args),
            LINUX_SYS_RSEQ => SyscallOutcome::errno(ENOSYS),

            _ => SyscallOutcome::errno(ENOSYS),
        }
    }

    fn sys_getrandom(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let len = args[1] as usize;
        let mut buf = [0u8; 256];
        let copy_len = core::cmp::min(len, buf.len());
        for i in 0..copy_len {
            buf[i] = ((i as u8).wrapping_mul(37)).wrapping_add(13);
        }
        if let Err(e) = copyout_bytes(ptr, &buf[..copy_len]) {
            return SyscallOutcome::errno(e);
        }
        SyscallOutcome::success(copy_len as i64)
    }

    pub fn sys_arch_prctl(args: [u64; 6]) -> SyscallOutcome {
        let code = args[0];
        let addr = args[1];
        match code {
            ARCH_SET_FS => {
                crate::arch::x86_64::cpu::write_msr(0xC000_0100, addr);
                SyscallOutcome::success(0)
            }
            _ => SyscallOutcome::errno(EINVAL),
        }
    }

    pub fn sys_brk(args: [u64; 6]) -> SyscallOutcome {
        static mut CURRENT_BRK: u64 = 0x0000_6000_0000_0000;
        let new_brk = args[0];
        unsafe {
            if new_brk == 0 || new_brk <= CURRENT_BRK {
                SyscallOutcome::success(CURRENT_BRK as i64)
            } else {
                let old_brk = CURRENT_BRK;
                let old_page = (old_brk + 4095) & !4095;
                let new_page = (new_brk + 4095) & !4095;

                if new_page > old_page {
                    let hhdm_offset = crate::limine::hhdm_offset().unwrap();
                    let current_pml4 = crate::arch::x86_64::read_cr3();
                    let mut page = old_page;
                    while page < new_page {
                        if let Some(frame) = crate::mm::frame::allocate_frame() {
                            let phys = frame.start_address();
                            core::ptr::write_bytes((hhdm_offset + phys) as *mut u8, 0, 4096);
                            let _ = crate::arch::x86_64::map_user_region(
                                current_pml4,
                                hhdm_offset,
                                page,
                                phys,
                                4096,
                                crate::arch::x86_64::FLAG_USER_ACCESSIBLE | crate::arch::x86_64::FLAG_WRITABLE,
                            );
                        }
                        page += 4096;
                    }
                }
                CURRENT_BRK = new_brk;
                SyscallOutcome::success(CURRENT_BRK as i64)
            }
        }
    }

    pub fn sys_mmap(args: [u64; 6]) -> SyscallOutcome {
        static mut NEXT_MMAP_VADDR: u64 = 0x0000_7000_0000_0000;
        let hint_addr = args[0];
        let length = args[1] as usize;
        if length == 0 {
            return SyscallOutcome::errno(EINVAL);
        }
        let aligned_len = (length + 4095) & !4095;
        unsafe {
            let virt_addr = if hint_addr >= 0x0000_0001_0000_0000 && (hint_addr & 4095) == 0 {
                hint_addr
            } else {
                let v = NEXT_MMAP_VADDR;
                NEXT_MMAP_VADDR += aligned_len as u64;
                v
            };

            let hhdm_offset = crate::limine::hhdm_offset().unwrap();
            let current_pml4 = crate::arch::x86_64::read_cr3();

            let num_pages = aligned_len / 4096;
            for i in 0..num_pages {
                if let Some(frame) = crate::mm::frame::allocate_frame() {
                    let phys = frame.start_address();
                    core::ptr::write_bytes((hhdm_offset + phys) as *mut u8, 0, 4096);
                    let _ = crate::arch::x86_64::map_user_region(
                        current_pml4,
                        hhdm_offset,
                        virt_addr + (i * 4096) as u64,
                        phys,
                        4096,
                        crate::arch::x86_64::FLAG_USER_ACCESSIBLE | crate::arch::x86_64::FLAG_WRITABLE,
                    );
                }
            }
            SyscallOutcome::success(virt_addr as i64)
        }
    }

    pub fn sys_fstat(args: [u64; 6]) -> SyscallOutcome {
        let fd = args[0] as i32;
        let statbuf = args[1] as usize;
        if fd < 0 || fd as usize >= crate::sched::MAX_PROCESS_FDS {
            return SyscallOutcome::errno(super::EBADF);
        }
        let s = stat {
            st_dev: 0, st_ino: 1, st_nlink: 1,
            st_mode: 0o100644, st_uid: 0, st_gid: 0, __pad0: 0, st_rdev: 0,
            st_size: 4096, st_blksize: 4096, st_blocks: 1,
            st_atime: 0, st_atime_nsec: 0, st_mtime: 0, st_mtime_nsec: 0, st_ctime: 0, st_ctime_nsec: 0,
            __unused: [0; 3],
        };
        match copyout_struct(statbuf, &s) {
            Ok(_) => SyscallOutcome::success(0),
            Err(e) => SyscallOutcome::errno(e),
        }
    }

    #[repr(C, packed)]
    #[derive(Copy, Clone)]
    struct LinuxDirent64Header {
        d_ino: u64,
        d_off: i64,
        d_reclen: u16,
        d_type: u8,
    }

    pub fn sys_getdents64(args: [u64; 6]) -> SyscallOutcome {
        let fd = args[0] as i32;
        let dirp = args[1] as usize;
        let count = args[2] as usize;

        if fd < 0 || fd as usize >= crate::sched::MAX_PROCESS_FDS {
            return SyscallOutcome::errno(EBADF);
        }
        let table = crate::sched::process_fd_table();
        let global_idx = match table[fd as usize] {
            Some(idx) => idx,
            None => return SyscallOutcome::errno(EBADF),
        };

        let global_table = unsafe { &mut *FILE_TABLE.get() };
        let open = match &mut global_table[global_idx] {
            Some(f) => f,
            None => return SyscallOutcome::errno(EBADF),
        };

        if open.offset >= open.content_len {
            return SyscallOutcome::success(0);
        }

        let content_slice = &open.content[open.offset..open.content_len];
        let content_str = match core::str::from_utf8(content_slice) {
            Ok(s) => s,
            Err(_) => return SyscallOutcome::errno(EIO),
        };

        let mut user_written = 0usize;
        let mut current_offset_in_content = open.offset;

        for line in content_str.lines() {
            let name = line.trim();
            if name.is_empty() {
                continue;
            }

            let raw_len = core::mem::size_of::<LinuxDirent64Header>() + name.len() + 1;
            let reclen = (raw_len + 7) & !7;

            if user_written + reclen > count {
                break;
            }

            let next_offset = (current_offset_in_content + line.len() + 1) as i64;
            let header = LinuxDirent64Header {
                d_ino: 1,
                d_off: next_offset,
                d_reclen: reclen as u16,
                d_type: 8,
            };

            let dst_ptr = dirp + user_written;
            if copyout_struct(dst_ptr, &header).is_err() {
                return SyscallOutcome::errno(super::EFAULT);
            }

            let name_dst_ptr = dst_ptr + core::mem::size_of::<LinuxDirent64Header>();
            if crate::uaccess::copyout(name.as_bytes(), name_dst_ptr).is_err() {
                return SyscallOutcome::errno(super::EFAULT);
            }

            let null_byte = [0u8; 1];
            let null_dst_ptr = name_dst_ptr + name.len();
            if crate::uaccess::copyout(&null_byte, null_dst_ptr).is_err() {
                return SyscallOutcome::errno(super::EFAULT);
            }

            user_written += reclen;
            current_offset_in_content += line.len() + 1;
            open.offset = current_offset_in_content;
        }

        SyscallOutcome::success(user_written as i64)
    }

    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let statbuf = args[1] as usize;
        let mut path_buf = [0u8; super::MAX_PATH_BYTES];
        let path_len = match super::copyin_c_string(ptr, &mut path_buf) {
            Ok(len) => len,
            Err(e) => return SyscallOutcome::errno(e),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s,
            Err(_) => return SyscallOutcome::errno(super::EINVAL),
        };
        let node = match crate::vfs::lookup(raw_path) {
            Some(n) => n,
            None => return SyscallOutcome::errno(super::ENOENT),
        };

        let mut st_mode = 0o100644;
        if node.kind == crate::vfs::VfsNodeKind::Directory {
            st_mode = 0o040755;
        }

        let s = stat {
            st_dev: 0, st_ino: 1, st_nlink: 1,
            st_mode, st_uid: 0, st_gid: 0, __pad0: 0, st_rdev: 0,
            st_size: node.size as i64, st_blksize: 4096, st_blocks: ((node.size + 4095) / 4096) as i64,
            st_atime: 0, st_atime_nsec: 0, st_mtime: 0, st_mtime_nsec: 0, st_ctime: 0, st_ctime_nsec: 0,
            __unused: [0; 3],
        };

        match super::copyout_struct(statbuf, &s) {
            Ok(_) => SyscallOutcome::success(0),
            Err(e) => SyscallOutcome::errno(e),
        }
    }

    pub fn sys_lseek(args: [u64; 6]) -> SyscallOutcome {
        let fd = args[0] as i32;
        let offset = args[1] as i64;
        let whence = args[2] as i32;

        if fd < 0 || fd as usize >= crate::sched::MAX_PROCESS_FDS {
            return SyscallOutcome::errno(super::EBADF);
        }
        let table = crate::sched::process_fd_table();
        let global_idx = match table[fd as usize] {
            Some(idx) => idx,
            None => return SyscallOutcome::errno(super::EBADF),
        };
        let global_table = unsafe { &mut *super::FILE_TABLE.get() };
        let open = match &mut global_table[global_idx] {
            Some(f) => f,
            None => return SyscallOutcome::errno(super::EBADF),
        };

        let new_offset = match whence {
            0 => offset, // SEEK_SET
            1 => open.offset as i64 + offset, // SEEK_CUR
            2 => open.content.len() as i64 + offset, // SEEK_END
            _ => return SyscallOutcome::errno(super::EINVAL),
        };

        if new_offset < 0 {
            return SyscallOutcome::errno(super::EINVAL);
        }

        open.offset = new_offset as usize;
        SyscallOutcome::success(new_offset)
    }

    pub fn sys_ioctl(args: [u64; 6]) -> SyscallOutcome {
        let fd = args[0] as i32;
        if fd < 0 || fd as usize >= crate::sched::MAX_PROCESS_FDS {
            return SyscallOutcome::errno(super::EBADF);
        }
        let table = crate::sched::process_fd_table();
        if table[fd as usize].is_none() {
            return SyscallOutcome::errno(super::EBADF);
        }
        SyscallOutcome::success(0)
    }

    pub fn sys_pipe(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let global_table = unsafe { &mut *super::FILE_TABLE.get() };

        let mut r_idx = -1;
        let mut w_idx = -1;
        
        let mut r_fd = -1;
        let mut w_fd = -1;

        for i in 0..super::MAX_SYSTEM_FILES {
            if global_table[i].is_none() {
                if r_idx == -1 {
                    r_idx = i as i32;
                } else if w_idx == -1 {
                    w_idx = i as i32;
                    break;
                }
            }
        }

        if w_idx == -1 {
            return SyscallOutcome::errno(super::EMFILE);
        }

        let mut table = crate::sched::process_fd_table();
        for i in 3..crate::sched::MAX_PROCESS_FDS {
            if table[i].is_none() {
                if r_fd == -1 {
                    r_fd = i as i32;
                } else if w_fd == -1 {
                    w_fd = i as i32;
                    break;
                }
            }
        }

        if w_fd == -1 {
            return SyscallOutcome::errno(super::EMFILE);
        }

        global_table[r_idx as usize] = Some(super::OpenFile {
            ref_count: 1, path: { let mut p = [0u8; 128]; let s = b"pipe:read"; p[..s.len()].copy_from_slice(s); p }, path_len: 9, offset: 0, content: [0u8; 4096], content_len: 0
        });
        global_table[w_idx as usize] = Some(super::OpenFile {
            ref_count: 1, path: { let mut p = [0u8; 128]; let s = b"pipe:write"; p[..s.len()].copy_from_slice(s); p }, path_len: 10, offset: 0, content: [0u8; 4096], content_len: 0
        });

        table[r_fd as usize] = Some(r_idx as usize);
        table[w_fd as usize] = Some(w_idx as usize);
        crate::sched::set_process_fd_table(table);

        let fds = [r_fd, w_fd];
        match super::copyout_struct(ptr, &fds) {
            Ok(_) => SyscallOutcome::success(0),
            Err(e) => SyscallOutcome::errno(e),
        }
    }

    pub fn sys_dup(args: [u64; 6]) -> SyscallOutcome {
        let oldfd = args[0] as i32;
        if oldfd < 0 || oldfd as usize >= crate::sched::MAX_PROCESS_FDS {
            return SyscallOutcome::errno(super::EBADF);
        }
        let mut table = crate::sched::process_fd_table();
        let global_idx = match table[oldfd as usize] {
            Some(idx) => idx,
            None => return SyscallOutcome::errno(super::EBADF),
        };

        let mut newfd = -1;
        for i in 3..crate::sched::MAX_PROCESS_FDS {
            if table[i].is_none() {
                newfd = i as i32;
                break;
            }
        }
        if newfd == -1 {
            return SyscallOutcome::errno(super::EMFILE);
        }

        table[newfd as usize] = Some(global_idx);
        let global_table = unsafe { &mut *super::FILE_TABLE.get() };
        if let Some(open) = &mut global_table[global_idx] {
            open.ref_count += 1;
        }
        crate::sched::set_process_fd_table(table);

        SyscallOutcome::success(newfd as i64)
    }

    pub fn sys_dup2(args: [u64; 6]) -> SyscallOutcome {
        let oldfd = args[0] as i32;
        let newfd = args[1] as i32;
        if oldfd < 0 || oldfd as usize >= crate::sched::MAX_PROCESS_FDS || newfd < 0 || newfd as usize >= crate::sched::MAX_PROCESS_FDS {
            return SyscallOutcome::errno(super::EBADF);
        }
        if oldfd == newfd {
            return SyscallOutcome::success(newfd as i64);
        }

        let mut table = crate::sched::process_fd_table();
        let global_idx = match table[oldfd as usize] {
            Some(idx) => idx,
            None => return SyscallOutcome::errno(super::EBADF),
        };

        if let Some(old_g_idx) = table[newfd as usize] {
            let global_table = unsafe { &mut *super::FILE_TABLE.get() };
            if let Some(open) = &mut global_table[old_g_idx] {
                if open.ref_count > 1 {
                    open.ref_count -= 1;
                } else {
                    global_table[old_g_idx] = None;
                }
            }
        }

        table[newfd as usize] = Some(global_idx);
        let global_table = unsafe { &mut *super::FILE_TABLE.get() };
        if let Some(open) = &mut global_table[global_idx] {
            open.ref_count += 1;
        }
        crate::sched::set_process_fd_table(table);

        SyscallOutcome::success(newfd as i64)
    }

    pub fn sys_fork(_args: [u64; 6]) -> SyscallOutcome {
        match crate::sched::sys_fork_current_thread() {
            Ok(pid) => SyscallOutcome::success(pid as i64),
            Err(_) => SyscallOutcome::errno(super::ENOSYS),
        }
    }

    pub fn sys_wait4(args: [u64; 6]) -> SyscallOutcome {
        let pid = args[0] as i64;
        let status_ptr = args[1] as usize;
        let options = args[2] as u32;

        let parent_pid = crate::sched::current_process_id();
        if pid == -1 {
            loop {
                match crate::sched::sys_reap_child(parent_pid, -1) {
                    Ok((reaped_pid, status)) => {
                        if status_ptr != 0 {
                            let raw_status = status << 8;
                            let _ = super::copyout_struct(status_ptr, &raw_status);
                        }
                        return SyscallOutcome::success(reaped_pid as i64);
                    }
                    Err(has_children) => {
                        if !has_children {
                            return SyscallOutcome::errno(super::ENOENT);
                        }
                        if (options & 1) != 0 {
                            return SyscallOutcome::success(0);
                        }
                        return super::SyscallOutcome { value: 0, action: super::SyscallAction::YieldThread };
                    }
                }
            }
        } else {
            return SyscallOutcome::errno(super::ENOSYS);
        }
    }

    pub fn dispatch_ghost_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
        match number {
            GHOST_SYS_WRITE => write_with_fd(args),
            GHOST_SYS_OPEN => open_path_at(AT_FDCWD, args[0] as usize, args[1]),
            GHOST_SYS_READ => read_from_fd(args),
            GHOST_SYS_CLOSE => close_fd(args),
            GHOST_SYS_YIELD => SyscallOutcome::success(0),
            GHOST_SYS_GETPID => process_id(),
            GHOST_SYS_GETTID => thread_id(),
            GHOST_SYS_UPTIME_NSEC => uptime_ns(),
            GHOST_SYS_UNAME => ghost_uname(args),
            GHOST_SYS_EXIT_GROUP => exit_group(args),
            _ => SyscallOutcome::errno(ENOSYS),
        }
    }

    pub(super) fn linux_openat(args: [u64; 6]) -> SyscallOutcome {
        let dirfd = args[0] as i64;
        super::open_path_at(dirfd, args[1] as usize, args[2])
    }

    pub(super) fn linux_clock_gettime(args: [u64; 6]) -> SyscallOutcome {
        let clock_id = args[0] as i32;
        if clock_id != super::LINUX_CLOCK_REALTIME && clock_id != super::LINUX_CLOCK_MONOTONIC {
            return SyscallOutcome::errno(super::EINVAL);
        }

        let ptr = args[1] as usize;
        let uptime_ns = crate::time::uptime_nanoseconds();
        let timespec = super::LinuxTimespec {
            tv_sec: (uptime_ns / 1_000_000_000) as i64,
            tv_nsec: (uptime_ns % 1_000_000_000) as i64,
        };
        if let Err(error) = super::copyout_struct(ptr, &timespec) {
            return SyscallOutcome::errno(error);
        }

        SyscallOutcome::success(0)
    }

    pub(super) fn linux_uname(args: [u64; 6]) -> SyscallOutcome {
        super::write_uname(
            args[0] as usize,
            "Linux",
            "hxnu",
            "0.1.0-hxnu",
            "HXNU micro-hybrid kernel bootstrap",
            "x86_64",
            "localdomain",
        )
    }

    pub(super) fn ghost_uname(args: [u64; 6]) -> SyscallOutcome {
        super::write_uname(
            args[0] as usize,
            "Ghost",
            "hxnu",
            "0.1.0-ghost",
            "HXNU ghost compatibility bootstrap",
            "x86_64",
            "legacy",
        )
    }
}

pub fn dispatch_hxnu_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
    match number {
        HXNU_SYS_LOG_WRITE => write_without_fd(args),
        HXNU_SYS_OPEN => open_path_at(AT_FDCWD, args[0] as usize, args[1]),
        HXNU_SYS_READ => read_from_fd(args),
        HXNU_SYS_CLOSE => close_fd(args),
        HXNU_SYS_THREAD_SELF => thread_id(),
        HXNU_SYS_PROCESS_SELF => process_id(),
        HXNU_SYS_UPTIME_NSEC => uptime_ns(),
        HXNU_SYS_SCHED_YIELD => SyscallOutcome {
            value: 0,
            action: SyscallAction::YieldThread,
        },
        HXNU_SYS_ABI_VERSION => SyscallOutcome::success(HXNU_NATIVE_ABI_VERSION),
        HXNU_SYS_LCL_RECEIVE => lcl_receive(args),
        HXNU_SYS_LCL_RETURN => lcl_return(args),
        HXNU_SYS_SPAWN => hxnu_spawn(args),
        HXNU_SYS_HETEREXEC_SUBMIT => sys_heterexec_submit(args),
        HXNU_SYS_HETEREXEC_POLL => sys_heterexec_poll(args),
        HXNU_SYS_EXIT_GROUP => exit_group(args),
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

fn sys_heterexec_submit(args: [u64; 6]) -> SyscallOutcome {
    let target = (args[0] & 0xFF) as u8;
    match crate::hsched::HSCHED.submit_workload(crate::hsched::WorkloadType::DirectTarget(target)) {
        Ok(task_id) => SyscallOutcome::success(task_id as i64),
        Err(_) => SyscallOutcome::errno(EIO),
    }
}

fn sys_heterexec_poll(args: [u64; 6]) -> SyscallOutcome {
    let task_id = args[0] as u32;
    if task_id == 0 {
        let head = crate::hsched::HSCHED.shared_buffer.head.load(core::sync::atomic::Ordering::Relaxed);
        let tail = crate::hsched::HSCHED.shared_buffer.tail.load(core::sync::atomic::Ordering::Relaxed);
        SyscallOutcome::success(((head as i64) << 32) | (tail as i64 & 0xFFFF_FFFF))
    } else {
        let head = crate::hsched::HSCHED.shared_buffer.head.load(core::sync::atomic::Ordering::Relaxed);
        let completed = if (task_id as usize) <= head { 1 } else { 0 };
        SyscallOutcome::success(completed)
    }
}

fn hxnu_spawn(args: [u64; 6]) -> SyscallOutcome {
    let path_ptr = args[0] as usize;
    let argv_ptrs_ptr = args[1] as usize;
    let argc = args[2] as usize;

    let mut path_buf = [0u8; MAX_PATH_BYTES];
    let path_len = match copyin_c_string(path_ptr, &mut path_buf) {
        Ok(len) => len,
        Err(e) => return SyscallOutcome::errno(e),
    };
    let path_str = match core::str::from_utf8(&path_buf[..path_len]) {
        Ok(s) => s,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };

    let mut argv_strings: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    if argv_ptrs_ptr != 0 && argc > 0 && argc <= 16 {
        let mut ptr_buf = [0u64; 16];
        let bytes_to_copy = argc * core::mem::size_of::<u64>();
        let ptr_slice = unsafe {
            core::slice::from_raw_parts_mut(ptr_buf.as_mut_ptr() as *mut u8, bytes_to_copy)
        };
        if let Err(e) = copyin_bytes(argv_ptrs_ptr, ptr_slice) {
            return SyscallOutcome::errno(e);
        }
        for i in 0..argc {
            let str_ptr = ptr_buf[i] as usize;
            let mut arg_buf = [0u8; 256];
            let arg_len = match copyin_c_string(str_ptr, &mut arg_buf) {
                Ok(len) => len,
                Err(e) => return SyscallOutcome::errno(e),
            };
            if let Ok(arg_str) = core::str::from_utf8(&arg_buf[..arg_len]) {
                argv_strings.push(alloc::string::String::from(arg_str));
            }
        }
    }

    let argv_refs: alloc::vec::Vec<&str> = argv_strings.iter().map(|s| s.as_str()).collect();

    match crate::init_exec::spawn_executable(path_str, &argv_refs, &[]) {
        Ok(spawned) => SyscallOutcome::success(spawned.process_id as i64),
        Err(_) => SyscallOutcome::errno(ENOENT),
    }
}

pub fn run_linux_bootstrap_probe() -> LinuxBootstrapProbe {
    static WRITE_SMOKE: &[u8] = b"HXNU: linux syscall write() compatibility smoke\n";
    static OPEN_PATH: &[u8] = b"/proc/version\0";
    let abi = SyscallAbi::LinuxBootstrap;

    let write_result = dispatch(
        abi,
        LINUX_SYS_WRITE,
        [
            STDOUT_FD,
            WRITE_SMOKE.as_ptr() as u64,
            WRITE_SMOKE.len() as u64,
            0,
            0,
            0,
        ],
    )
    .value;

    let openat_result = dispatch(
        abi,
        LINUX_SYS_OPENAT,
        [AT_FDCWD as u64, OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let mut read_buffer = [0u8; 64];
    let mut read_result = -EBADF;
    let mut close_result = -EBADF;
    if openat_result >= 0 {
        let fd = openat_result as u64;
        read_result = dispatch(
            abi,
            LINUX_SYS_READ,
            [fd, read_buffer.as_mut_ptr() as u64, read_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        close_result = dispatch(abi, LINUX_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }

    let getpid_result = dispatch(abi, LINUX_SYS_GETPID, [0; 6]).value;
    let getppid_result = dispatch(abi, LINUX_SYS_GETPPID, [0; 6]).value;
    let gettid_result = dispatch(abi, LINUX_SYS_GETTID, [0; 6]).value;
    let sched_yield_result = dispatch(abi, LINUX_SYS_SCHED_YIELD, [0; 6]).value;

    let mut timespec = LinuxTimespec { tv_sec: 0, tv_nsec: 0 };
    let clock_gettime_result = dispatch(
        abi,
        LINUX_SYS_CLOCK_GETTIME,
        [
            LINUX_CLOCK_MONOTONIC as u64,
            (&mut timespec as *mut LinuxTimespec) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;

    let mut utsname = LinuxUtsName::new();
    let uname_result = dispatch(
        abi,
        LINUX_SYS_UNAME,
        [(&mut utsname as *mut LinuxUtsName) as u64, 0, 0, 0, 0, 0],
    )
    .value;

    let exit_group = dispatch(abi, LINUX_SYS_EXIT_GROUP, [17, 0, 0, 0, 0, 0]);
    let (exit_group_captured, exit_group_status) = exit_status(exit_group);

    let mut machine_bytes = [0u8; 16];
    let machine_len = copy_c_field_prefix(&mut machine_bytes, &utsname.machine);

    LinuxBootstrapProbe {
        write_result,
        openat_result,
        read_result,
        close_result,
        getpid_result,
        getppid_result,
        gettid_result,
        sched_yield_result,
        clock_gettime_result,
        clock_seconds: timespec.tv_sec,
        clock_nanoseconds: timespec.tv_nsec,
        uname_result,
        exit_group_captured,
        exit_group_status,
        machine_bytes,
        machine_len,
    }
}

pub fn run_ghost_bootstrap_probe() -> GhostBootstrapProbe {
    static WRITE_SMOKE: &[u8] = b"HXNU: ghost syscall write() compatibility smoke\n";
    static OPEN_PATH: &[u8] = b"/proc/version\0";
    let abi = SyscallAbi::GhostBootstrap;

    let write_result = dispatch(
        abi,
        GHOST_SYS_WRITE,
        [
            STDERR_FD,
            WRITE_SMOKE.as_ptr() as u64,
            WRITE_SMOKE.len() as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let open_result = dispatch(abi, GHOST_SYS_OPEN, [OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;

    let mut read_buffer = [0u8; 64];
    let mut read_result = -EBADF;
    let mut close_result = -EBADF;
    if open_result >= 0 {
        let fd = open_result as u64;
        read_result = dispatch(
            abi,
            GHOST_SYS_READ,
            [fd, read_buffer.as_mut_ptr() as u64, read_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        close_result = dispatch(abi, GHOST_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }

    let getpid_result = dispatch(abi, GHOST_SYS_GETPID, [0; 6]).value;
    let gettid_result = dispatch(abi, GHOST_SYS_GETTID, [0; 6]).value;
    let yield_result = dispatch(abi, GHOST_SYS_YIELD, [0; 6]).value;
    let uptime_result = dispatch(abi, GHOST_SYS_UPTIME_NSEC, [0; 6]).value;

    let mut utsname = LinuxUtsName::new();
    let uname_result = dispatch(
        abi,
        GHOST_SYS_UNAME,
        [(&mut utsname as *mut LinuxUtsName) as u64, 0, 0, 0, 0, 0],
    )
    .value;

    let exit_group = dispatch(abi, GHOST_SYS_EXIT_GROUP, [19, 0, 0, 0, 0, 0]);
    let (exit_group_captured, exit_group_status) = exit_status(exit_group);

    let mut machine_bytes = [0u8; 16];
    let machine_len = copy_c_field_prefix(&mut machine_bytes, &utsname.machine);

    GhostBootstrapProbe {
        write_result,
        open_result,
        read_result,
        close_result,
        getpid_result,
        gettid_result,
        yield_result,
        uptime_result,
        uname_result,
        exit_group_captured,
        exit_group_status,
        machine_bytes,
        machine_len,
    }
}

pub fn run_hxnu_bootstrap_probe() -> HxnuBootstrapProbe {
    static WRITE_SMOKE: &[u8] = b"HXNU: native syscall log_write() bootstrap smoke\n";
    static OPEN_PATH: &[u8] = b"/proc/version\0";
    let abi = SyscallAbi::HxnuNativeBootstrap;

    let write_result = dispatch(
        abi,
        HXNU_SYS_LOG_WRITE,
        [WRITE_SMOKE.as_ptr() as u64, WRITE_SMOKE.len() as u64, 0, 0, 0, 0],
    )
    .value;
    let open_result = dispatch(abi, HXNU_SYS_OPEN, [OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;

    let mut read_buffer = [0u8; 64];
    let mut read_result = -EBADF;
    let mut close_result = -EBADF;
    if open_result >= 0 {
        let fd = open_result as u64;
        read_result = dispatch(
            abi,
            HXNU_SYS_READ,
            [fd, read_buffer.as_mut_ptr() as u64, read_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        close_result = dispatch(abi, HXNU_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }

    let process_self_result = dispatch(abi, HXNU_SYS_PROCESS_SELF, [0; 6]).value;
    let thread_self_result = dispatch(abi, HXNU_SYS_THREAD_SELF, [0; 6]).value;
    let sched_yield_result = dispatch(abi, HXNU_SYS_SCHED_YIELD, [0; 6]).value;
    let uptime_result = dispatch(abi, HXNU_SYS_UPTIME_NSEC, [0; 6]).value;
    let abi_version_result = dispatch(abi, HXNU_SYS_ABI_VERSION, [0; 6]).value;

    let exit_group = dispatch(abi, HXNU_SYS_EXIT_GROUP, [23, 0, 0, 0, 0, 0]);
    let (exit_group_captured, exit_group_status) = exit_status(exit_group);

    HxnuBootstrapProbe {
        write_result,
        open_result,
        read_result,
        close_result,
        process_self_result,
        thread_self_result,
        sched_yield_result,
        uptime_result,
        abi_version_result,
        exit_group_captured,
        exit_group_status,
    }
}



fn open_path_at(dirfd: i64, path_ptr: usize, flags: u64) -> SyscallOutcome {
    if !is_read_only_open(flags) {
        return SyscallOutcome::errno(EINVAL);
    }

    let mut buf = [0u8; MAX_PATH_BYTES];
    let len = match copyin_c_string(path_ptr, &mut buf) {
        Ok(len) => len,
        Err(error) => return SyscallOutcome::errno(error),
    };
    
    let raw_path = match core::str::from_utf8(&buf[..len]) {
        Ok(path) => path,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };

    if raw_path.is_empty() {
        return SyscallOutcome::errno(EINVAL);
    }

    let mut resolved_buf = [0u8; MAX_PATH_BYTES];
    let resolved_len;
    
    if raw_path.starts_with('/') {
        let copy_len = core::cmp::min(raw_path.len(), MAX_PATH_BYTES);
        resolved_buf[..copy_len].copy_from_slice(raw_path.as_bytes());
        resolved_len = copy_len;
    } else if dirfd == AT_FDCWD {
        resolved_buf[0] = b'/';
        let copy_len = core::cmp::min(raw_path.len(), MAX_PATH_BYTES - 1);
        resolved_buf[1..1 + copy_len].copy_from_slice(raw_path.as_bytes());
        resolved_len = 1 + copy_len;
    } else {
        return SyscallOutcome::errno(ENOSYS);
    };

    let resolved_path = match core::str::from_utf8(&resolved_buf[..resolved_len]) {
        Ok(path) => path,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };

    let node = match vfs::lookup(resolved_path) {
        Some(node) => node,
        None => return SyscallOutcome::errno(ENOENT),
    };

    let content = match vfs::read(&node.path) {
        Some(content) => content,
        None => return SyscallOutcome::errno(EIO),
    };
    match alloc_open_file(&node.path, content.as_bytes()) {
        Ok(fd) => SyscallOutcome::success(fd),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn read_from_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let count = match usize::try_from(args[2]) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if count > MAX_READ_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }

    match read_open_file(fd, args[1] as usize, count) {
        Ok(read) => SyscallOutcome::success(read),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn close_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    match close_open_file(fd) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn write_with_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = args[0];
    if fd != STDOUT_FD && fd != STDERR_FD {
        return SyscallOutcome::errno(EBADF);
    }

    write_text(args[1] as usize, args[2])
}

fn write_without_fd(args: [u64; 6]) -> SyscallOutcome {
    write_text(args[0] as usize, args[1])
}

fn write_text(ptr: usize, len: u64) -> SyscallOutcome {
    let count = match usize::try_from(len) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if count > MAX_WRITE_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }
    if count == 0 {
        return SyscallOutcome::success(0);
    }

    let mut buf = [0u8; 1024];
    let copy_len = core::cmp::min(count, buf.len());
    match copyin_bytes(ptr, &mut buf[..copy_len]) {
        Ok(_) => {},
        Err(error) => return SyscallOutcome::errno(error),
    }

    let text = sanitize_for_console(&buf[..copy_len]);
    tty::write_str(text);

    match i64::try_from(copy_len) {
        Ok(written) => SyscallOutcome::success(written),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_id() -> SyscallOutcome {
    match i64::try_from(current_process_id_value()) {
        Ok(process_id) => SyscallOutcome::success(process_id),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_parent_id() -> SyscallOutcome {
    let parent_process_id = sched::stats().current_parent_process_id;
    match i64::try_from(parent_process_id) {
        Ok(parent_process_id) => SyscallOutcome::success(parent_process_id),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn thread_id() -> SyscallOutcome {
    let tid = sched::stats().current_thread_id;
    match i64::try_from(tid) {
        Ok(tid) => SyscallOutcome::success(tid),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn uptime_ns() -> SyscallOutcome {
    let uptime = time::uptime_nanoseconds();
    match i64::try_from(uptime) {
        Ok(uptime) => SyscallOutcome::success(uptime),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}





fn write_uname(
    ptr: usize,
    sysname: &str,
    nodename: &str,
    release: &str,
    version: &str,
    machine: &str,
    domainname: &str,
) -> SyscallOutcome {
    let mut uts = LinuxUtsName::new();
    write_uts_field(&mut uts.sysname, sysname);
    write_uts_field(&mut uts.nodename, nodename);
    write_uts_field(&mut uts.release, release);
    write_uts_field(&mut uts.version, version);
    write_uts_field(&mut uts.machine, machine);
    write_uts_field(&mut uts.domainname, domainname);
    if let Err(error) = copyout_struct(ptr, &uts) {
        return SyscallOutcome::errno(error);
    }

    SyscallOutcome::success(0)
}

fn exit_group(args: [u64; 6]) -> SyscallOutcome {
    let status = args[0] as i32;
    purge_open_files_for_process(current_process_id_value());
    SyscallOutcome {
        value: 0,
        action: SyscallAction::ExitGroup { status },
    }
}

fn exit_status(outcome: SyscallOutcome) -> (bool, i32) {
    match outcome.action {
        SyscallAction::ExitGroup { status } => (true, status),
        SyscallAction::YieldThread => (false, 0),
        SyscallAction::Continue => (false, 0),
    }
}

fn sanitize_for_console(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("")
}

fn is_read_only_open(flags: u64) -> bool {
    (flags & O_ACCMODE) == O_RDONLY && (flags & (O_CREAT | O_TRUNC | O_APPEND)) == 0
}

fn parse_fd(value: u64) -> Result<i32, i64> {
    i32::try_from(value).map_err(|_| EBADF)
}

fn current_process_id_value() -> u64 {
    sched::stats().current_process_id
}

fn alloc_open_file(path: &str, content: &[u8]) -> Result<i64, i64> {
    let mut table = sched::process_fd_table();
    let mut fd = -1;
    for i in 3..sched::MAX_PROCESS_FDS {
        if table[i].is_none() {
            fd = i as i32;
            break;
        }
    }
    if fd == -1 {
        return Err(EMFILE);
    }
    
    let global_table = unsafe { &mut *FILE_TABLE.get() };
    let mut global_idx = -1;
    for i in 0..MAX_SYSTEM_FILES {
        if global_table[i].is_none() {
            global_table[i] = Some(OpenFile {
                ref_count: 1,
                path: { let mut b = [0u8; 128]; let l = core::cmp::min(path.len(), 128); b[..l].copy_from_slice(&path.as_bytes()[..l]); b },
                path_len: core::cmp::min(path.len(), 128),
                offset: 0,
                content: { let mut b = [0u8; 4096]; let l = core::cmp::min(content.len(), 4096); b[..l].copy_from_slice(&content[..l]); b },
                content_len: core::cmp::min(content.len(), 4096),
            });
            global_idx = i as i32;
            break;
        }
    }
    if global_idx == -1 {
        return Err(EMFILE);
    }
    
    table[fd as usize] = Some(global_idx as usize);
    sched::set_process_fd_table(table);
    Ok(fd as i64)
}

fn read_open_file(fd: i32, destination_ptr: usize, count: usize) -> Result<i64, i64> {
    if fd < 0 || fd as usize >= sched::MAX_PROCESS_FDS {
        return Err(EBADF);
    }
    let table = sched::process_fd_table();
    let global_idx = table[fd as usize].ok_or(EBADF)?;
    
    let global_table = unsafe { &mut *FILE_TABLE.get() };
    let open = global_table[global_idx].as_mut().ok_or(EBADF)?;
    
    if count == 0 {
        return Ok(0);
    }

    let available = open.content_len.saturating_sub(open.offset);
    let read_len = min(count, available);
    if read_len > 0 {
        let bytes = &open.content[open.offset..open.offset + read_len];
        uaccess::copyout(bytes, destination_ptr).map_err(map_uaccess_error)?;
        open.offset = open.offset.saturating_add(read_len);
    }
    i64::try_from(read_len).map_err(|_| ERANGE)
}

fn close_open_file(fd: i32) -> Result<i64, i64> {
    if fd < 0 || fd as usize >= sched::MAX_PROCESS_FDS {
        return Err(EBADF);
    }
    let mut table = sched::process_fd_table();
    let global_idx = table[fd as usize].ok_or(EBADF)?;
    
    table[fd as usize] = None;
    sched::set_process_fd_table(table);
    
    let global_table = unsafe { &mut *FILE_TABLE.get() };
    if let Some(open) = &mut global_table[global_idx] {
        if open.ref_count > 1 {
            open.ref_count -= 1;
        } else {
            global_table[global_idx] = None;
        }
    }
    Ok(0)
}

fn purge_open_files_for_process(_process_id: u64) {
    let mut table = sched::process_fd_table();
    for fd in 3..sched::MAX_PROCESS_FDS {
        if let Some(global_idx) = table[fd] {
            let global_table = unsafe { &mut *FILE_TABLE.get() };
            if let Some(open) = &mut global_table[global_idx] {
                if open.ref_count > 1 {
                    open.ref_count -= 1;
                } else {
                    global_table[global_idx] = None;
                }
            }
            table[fd] = None;
        }
    }
    sched::set_process_fd_table(table);
}

fn copyin_c_string(ptr: usize, buf: &mut [u8]) -> Result<usize, i64> {
    for index in 0..buf.len() {
        let address = ptr.checked_add(index).ok_or(ERANGE)?;
        let mut byte = [0u8; 1];
        uaccess::copyin(address, &mut byte).map_err(map_uaccess_error)?;
        if byte[0] == 0 {
            return Ok(index);
        }
        buf[index] = byte[0];
    }
    Err(ERANGE)
}

fn copyin_bytes(ptr: usize, buf: &mut [u8]) -> Result<(), i64> {
    uaccess::copyin(ptr, buf).map_err(map_uaccess_error)?;
    Ok(())
}

fn copyout_bytes(ptr: usize, buf: &[u8]) -> Result<(), i64> {
    uaccess::copyout(buf, ptr).map_err(map_uaccess_error)
}

fn copyout_struct<T: Copy>(ptr: usize, value: &T) -> Result<(), i64> {
    let bytes = unsafe { slice::from_raw_parts((value as *const T).cast::<u8>(), size_of_val(value)) };
    uaccess::copyout(bytes, ptr).map_err(map_uaccess_error)
}

fn map_uaccess_error(error: UserCopyError) -> i64 {
    error.as_errno()
}

fn write_uts_field(field: &mut [u8; 65], value: &str) {
    let bytes = value.as_bytes();
    let copy_len = min(bytes.len(), field.len().saturating_sub(1));
    field[..copy_len].copy_from_slice(&bytes[..copy_len]);
    field[copy_len] = 0;
}

fn copy_c_field_prefix(output: &mut [u8], field: &[u8; 65]) -> usize {
    let mut length = 0usize;
    while length < output.len() && length < field.len() {
        let byte = field[length];
        if byte == 0 {
            break;
        }
        output[length] = byte;
        length += 1;
    }
    length
}

fn machine_str(machine_bytes: &[u8], machine_len: usize) -> &str {
    match str::from_utf8(&machine_bytes[..machine_len]) {
        Ok(machine) => machine,
        Err(_) => "<invalid>",
    }
}

fn lcl_receive(args: [u64; 6]) -> SyscallOutcome {
    let ptr = args[0] as usize;
    if let Some(msg) = dequeue_lcl_message() {
        let mut data = [0u64; 10];
        data[0] = msg.number;
        data[1] = msg.args[0];
        data[2] = msg.args[1];
        data[3] = msg.args[2];
        data[4] = msg.args[3];
        data[5] = msg.args[4];
        data[6] = msg.args[5];
        data[7] = msg.pid;
        data[8] = msg.tid;
        data[9] = msg.abi;
        let data_bytes = unsafe { core::slice::from_raw_parts(&data as *const _ as *const u8, core::mem::size_of_val(&data)) };
        if crate::uaccess::copyout(data_bytes, ptr).is_ok() {
            return SyscallOutcome::success(1);
        }
    }
    SyscallOutcome::success(0)
}

fn lcl_return(args: [u64; 6]) -> SyscallOutcome {
    let result = args[0];
    let target_pid = args[1];
    let target_tid = args[2];

    if crate::sched::unblock_thread_with_result(target_pid, target_tid, result).is_ok() {
        SyscallOutcome::success(0)
    } else {
        SyscallOutcome::errno(EINVAL)
    }
}
