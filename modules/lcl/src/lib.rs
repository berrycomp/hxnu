#![no_std]

pub const ENOSYS: i64 = -38;
pub const EPERM: i64 = -1;

#[derive(Debug)]
#[repr(C)]
pub struct SyscallOutcome {
    pub value: i64,
}

impl SyscallOutcome {
    pub fn errno(code: i64) -> Self {
        Self { value: code }
    }
}

/// Linux Compatibility Layer (LCL) Entrypoint for Syscalls
/// This handles Linux ELF standard POSIX syscalls mapped to HPS zero-latency hardware.
#[allow(improper_ctypes_definitions)]
#[unsafe(no_mangle)]
pub extern "C" fn hxnu_module_dispatch_lcl(number: u64, args: [u64; 6], entropy_id: u64) -> SyscallOutcome {
    if entropy_id == 0 {
        return SyscallOutcome::errno(EPERM);
    }
    
    match number {
        // MMAP (9)
        9 => lcl_mmap(args, entropy_id),
        // FORK (57)
        57 => lcl_fork(args, entropy_id),
        // EXECVE (59)
        59 => lcl_execve(args, entropy_id),
        // Fallback
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

fn lcl_mmap(_args: [u64; 6], _entropy_id: u64) -> SyscallOutcome {
    // TODO: Map to HPS_SYS_MMAP_VMM
    SyscallOutcome::errno(ENOSYS)
}

fn lcl_fork(_args: [u64; 6], _entropy_id: u64) -> SyscallOutcome {
    // TODO: Map to HPS_SYS_SPAWN_WORKER
    SyscallOutcome::errno(ENOSYS)
}

fn lcl_execve(_args: [u64; 6], _entropy_id: u64) -> SyscallOutcome {
    // TODO: Handle ELF loading via HPS
    SyscallOutcome::errno(ENOSYS)
}
