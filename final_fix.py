import re

with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

# sys_wait4
sysc = sysc.replace('match crate::sched::sys_reap_child(parent_pid) {', 'match crate::sched::sys_reap_child(parent_pid, target_pid) {')
sysc = sysc.replace('let parent_pid = crate::sched::current_process_id();', 'let parent_pid = crate::sched::current_process_id();\n        let target_pid = args[0] as i64;')

# alloc_open_file usages
sysc = sysc.replace('match alloc_open_file(node.path.clone(), content_buf[..content_len].to_vec()) {', 'match alloc_open_file(&node.path, &content_buf[..content_len]) {')
sysc = sysc.replace('match alloc_open_file(node.path, content) {', 'match alloc_open_file(&node.path, &content) {')

# copyin_bytes calls
sysc = sysc.replace('match copyin_bytes(ptr, &mut buf[..read_len]) {', 'match copyin_bytes(ptr, &mut buf[..core::cmp::min(read_len, 4096)]) {')

sysc = re.sub(
    r'let bytes = match copyin_bytes\(ptr, count\) \{.*?Err\(error\) => return SyscallOutcome::errno\(error\),\s*\};\s*let text = sanitize_for_console\(&bytes\);',
    r'''let mut buf = [0u8; 4096];
    let read_len = core::cmp::min(count as usize, 4096);
    if let Err(error) = copyin_bytes(ptr, &mut buf[..read_len]) {
        return SyscallOutcome::errno(error);
    }
    let text = sanitize_for_console(&buf[..read_len]);''', sysc, flags=re.DOTALL)

# raw_path in sys_openat, sys_execve
sysc = re.sub(
    r'let raw_path = match copyin_c_string\(path_ptr, MAX_PATH_BYTES\) \{.*?Err\(error\) => return SyscallOutcome::errno\(error\),\s*\};',
    r'''let mut path_buf = [0u8; 128];
    let path_len = match copyin_c_string(path_ptr, &mut path_buf) {
        Ok(len) => len,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
        Ok(s) => s,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };''', sysc, flags=re.DOTALL)

# raw_path in sys_stat
sysc = re.sub(
    r'let raw_path = match super::copyin_c_string\(ptr, super::MAX_PATH_BYTES\) \{.*?Err\(error\) => return SyscallOutcome::errno\(error\),\s*\};',
    r'''let mut path_buf = [0u8; 128];
        let path_len = match super::copyin_c_string(ptr, &mut path_buf) {
            Ok(len) => len,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s,
            Err(_) => return SyscallOutcome::errno(super::EINVAL),
        };''', sysc, flags=re.DOTALL)

sysc = sysc.replace('if raw_path.is_empty() {', 'if path_len == 0 {')
sysc = sysc.replace("if raw_path.starts_with('/') {", 'if raw_path.starts_with("/") {')
sysc = sysc.replace('match crate::vfs::lookup(&raw_path) {', 'match crate::vfs::lookup(raw_path) {')

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)
