with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

old_stat = '''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let statbuf = args[1] as usize;
        let raw_path = match super::copyin_c_string(ptr, super::MAX_PATH_BYTES) {
            Ok(p) => p,
            Err(e) => return SyscallOutcome::errno(e),
        };
        let node = match crate::vfs::lookup(&raw_path) {'''

new_stat = '''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let statbuf = args[1] as usize;
        let mut path_buf = [0u8; 128];
        let path_len = match super::copyin_c_string(ptr, &mut path_buf) {
            Ok(l) => l,
            Err(e) => return SyscallOutcome::errno(e),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s, Err(_) => return SyscallOutcome::errno(super::EINVAL),
        };
        let node = match crate::vfs::lookup(raw_path) {'''

sysc = sysc.replace(old_stat, new_stat)


old_execve = '''    pub fn sys_execve(args: [u64; 6]) -> SyscallOutcome {
        let path_ptr = args[0] as usize;
        let raw_path = match copyin_c_string(path_ptr, MAX_PATH_BYTES) {
            Ok(path) => path,
            Err(error) => return SyscallOutcome::errno(error),
        };
        
        let node = match crate::vfs::lookup(&raw_path) {'''

new_execve = '''    pub fn sys_execve(args: [u64; 6]) -> SyscallOutcome {
        let path_ptr = args[0] as usize;
        let mut path_buf = [0u8; 128];
        let path_len = match copyin_c_string(path_ptr, &mut path_buf) {
            Ok(len) => len,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s, Err(_) => return SyscallOutcome::errno(EINVAL),
        };
        
        let node = match crate::vfs::lookup(raw_path) {'''

sysc = sysc.replace(old_execve, new_execve)

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)
