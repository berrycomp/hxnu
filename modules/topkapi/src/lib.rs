#![no_std]

pub const ENOSYS: i64 = -38;
pub const EPERM: i64 = -1;

#[derive(Debug)]
pub struct SyscallOutcome {
    pub value: i64,
}

impl SyscallOutcome {
    pub fn errno(code: i64) -> Self {
        Self { value: code }
    }
}

/// Windows NT UNICODE_STRING Representation (Clean-Room)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UnicodeString {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: *mut u16,
}

/// Windows NT OBJECT_ATTRIBUTES Representation (Clean-Room)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ObjectAttributes {
    pub length: u32,
    pub root_directory: u64, // HANDLE equivalent
    pub object_name: *mut UnicodeString,
    pub attributes: u32,
    pub security_descriptor: *mut u8,
    pub security_quality_of_service: *mut u8,
}

/// TOPKAPI Entrypoint for Windows NT Syscalls
/// This handles PE executables and Windows NT syscall translation to HPS.
#[unsafe(no_mangle)]
pub extern "C" fn hxnu_module_dispatch_topkapi(number: u64, args: [u64; 6], entropy_id: u64) -> SyscallOutcome {
    if entropy_id == 0 {
        return SyscallOutcome::errno(EPERM);
    }
    
    // Windows NT uses completely different syscall numbers.
    match number {
        // NtAllocateVirtualMemory (0x18 usually, depends on NT version)
        0x18 => topkapi_allocate_virtual_memory(args, entropy_id),
        // NtCreateThread (0x4E usually)
        0x4E => topkapi_create_thread(args, entropy_id),
        // Fallback
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

fn topkapi_allocate_virtual_memory(_args: [u64; 6], _entropy_id: u64) -> SyscallOutcome {
    // TODO: Map to HPS_SYS_MMAP_VMM and handle NT semantics
    SyscallOutcome::errno(ENOSYS)
}

fn topkapi_create_thread(_args: [u64; 6], _entropy_id: u64) -> SyscallOutcome {
    // TODO: Map to HPS_SYS_SPAWN_WORKER
    SyscallOutcome::errno(ENOSYS)
}
