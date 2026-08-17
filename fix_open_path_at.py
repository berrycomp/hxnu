with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

old_open = '''fn open_path_at(dirfd: i64, path_ptr: usize, flags: u64) -> SyscallOutcome {
    if !is_read_only_open(flags) {
        return SyscallOutcome::errno(EINVAL);
    }

    let raw_path = match copyin_c_string(path_ptr, MAX_PATH_BYTES) {
        Ok(path) => path,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if raw_path.is_empty() {
        return SyscallOutcome::errno(EINVAL);
    }

    let resolved_path = if raw_path.starts_with('/') {
        raw_path
    } else if dirfd == AT_FDCWD {
        let mut absolute = String::from("/");
        absolute.push_str(&raw_path);
        absolute
    } else {
        return SyscallOutcome::errno(ENOSYS);
    };

    let node = match crate::vfs::lookup(&resolved_path) {'''

new_open = '''fn open_path_at(dirfd: i64, path_ptr: usize, flags: u64) -> SyscallOutcome {
    if !is_read_only_open(flags) {
        return SyscallOutcome::errno(EINVAL);
    }

    let mut path_buf = [0u8; 128];
    let path_len = match copyin_c_string(path_ptr, &mut path_buf) {
        Ok(len) => len,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if path_len == 0 { return SyscallOutcome::errno(EINVAL); }
    let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
        Ok(s) => s, Err(_) => return SyscallOutcome::errno(EINVAL),
    };

    let resolved_path = if raw_path.starts_with('/') {
        raw_path
    } else {
        return SyscallOutcome::errno(ENOSYS);
    };

    let node = match crate::vfs::lookup(resolved_path) {'''
sysc = sysc.replace(old_open, new_open)

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)
