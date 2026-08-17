with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

# 1. OpenFile
sysc = sysc.replace('''#[derive(Clone)]
pub struct OpenFile {
    pub ref_count: usize,
    pub path: String,
    pub offset: usize,
    pub content: Vec<u8>,
}''', '''#[derive(Copy, Clone)]
pub struct OpenFile {
    pub ref_count: usize,
    pub path: [u8; 128],
    pub path_len: usize,
    pub offset: usize,
    pub content: [u8; 4096],
    pub content_len: usize,
}''')

# 2. alloc_open_file usages
sysc = sysc.replace('fn alloc_open_file(path: String, content: Vec<u8>) -> Result<i64, i64> {', 'fn alloc_open_file(path: &str, content: &[u8]) -> Result<i64, i64> {')
sysc = sysc.replace('path: path.clone(),', 'path: { let mut b=[0u8;128]; let l=core::cmp::min(path.len(),128); b[..l].copy_from_slice(path.as_bytes()); b }, path_len: core::cmp::min(path.len(), 128),')
sysc = sysc.replace('content: content.clone(),', 'content: { let mut cbuf=[0u8;4096]; let cl=core::cmp::min(content.len(),4096); cbuf[..cl].copy_from_slice(&content[..cl]); cbuf }, content_len: core::cmp::min(content.len(), 4096),')

# 3. usages of alloc_open_file
sysc = sysc.replace('match alloc_open_file(node.path.clone(), content_buf[..content_len].to_vec()) {', 'match alloc_open_file(&node.path, &content_buf[..content_len]) {')
sysc = sysc.replace('match alloc_open_file(node.path, content) {', 'match alloc_open_file(&node.path, &content) {')
sysc = sysc.replace('match alloc_open_file(node.path, Vec::new()) {', 'match alloc_open_file(&node.path, &[]) {')

# 4. sys_pipe
sysc = sysc.replace('path: alloc::string::String::from("pipe:read"), offset: 0, content: alloc::vec::Vec::new()', 'path: { let mut b=[0u8;128]; let s=b"pipe:read"; b[..s.len()].copy_from_slice(s); b }, path_len: 9, offset: 0, content: [0u8; 4096], content_len: 0')
sysc = sysc.replace('path: alloc::string::String::from("pipe:write"), offset: 0, content: alloc::vec::Vec::new()', 'path: { let mut b=[0u8;128]; let s=b"pipe:write"; b[..s.len()].copy_from_slice(s); b }, path_len: 10, offset: 0, content: [0u8; 4096], content_len: 0')

# 5. sys_wait4
sysc = sysc.replace('''        loop {
            let status_ptr = args[1] as usize;
            
            crate::sched::with_scheduler(|_s| {
                match crate::sched::sys_reap_child(parent_pid) {''', '''        let target_pid = args[0] as i64;
        loop {
            let status_ptr = args[1] as usize;
            
            crate::sched::with_scheduler(|_s| {
                match crate::sched::sys_reap_child(parent_pid, target_pid) {''')

# 6. copyin_bytes signature and implementation
sysc = sysc.replace('''fn copyin_bytes(ptr: usize, len: usize) -> Result<Vec<u8>, i64> {
    let mut bytes = vec![0u8; len];
    uaccess::copyin(ptr, &mut bytes).map_err(map_uaccess_error)?;
    Ok(bytes)
}''', '''fn copyin_bytes(ptr: usize, buf: &mut [u8]) -> Result<(), i64> {
    uaccess::copyin(ptr, buf).map_err(map_uaccess_error)?;
    Ok(())
}''')

# 7. copyin_c_string signature and implementation
sysc = sysc.replace('''fn copyin_c_string(ptr: usize, max_len: usize) -> Result<String, i64> {
    let mut bytes = Vec::new();
    for index in 0..max_len {
        let address = ptr.checked_add(index).ok_or(ERANGE)?;
        let mut byte = [0u8; 1];
        uaccess::copyin(address, &mut byte).map_err(map_uaccess_error)?;
        if byte[0] == 0 {
            let text = str::from_utf8(&bytes).map_err(|_| EINVAL)?;
            return Ok(String::from(text));
        }
        bytes.push(byte[0]);
    }

    Err(ERANGE)
}''', '''fn copyin_c_string(ptr: usize, buf: &mut [u8]) -> Result<usize, i64> {
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
}''')

# 8. write_text
sysc = sysc.replace('''    let bytes = match copyin_bytes(ptr, count) {
        Ok(bytes) => bytes,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let text = sanitize_for_console(&bytes);''', '''    let mut buf = [0u8; 4096];
    let read_len = core::cmp::min(count as usize, 4096);
    if let Err(error) = copyin_bytes(ptr, &mut buf[..read_len]) {
        return SyscallOutcome::errno(error);
    }
    let text = sanitize_for_console(&buf[..read_len]);''')

# 9. sanitize_for_console
sysc = sysc.replace('''fn sanitize_for_console(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'\\n' | b'\\r' | b'\\t' | 0x20..=0x7e => text.push(byte as char),
            _ => text.push('?'),
        }
    }
    text
}''', '''fn sanitize_for_console(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("")
}''')

# 10. open_path_at (sys_openat)
sysc = sysc.replace('''    let raw_path = match copyin_c_string(path_ptr, MAX_PATH_BYTES) {
        Ok(path) => path,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if raw_path.is_empty() {
        return SyscallOutcome::errno(EINVAL);
    }

    let resolved_path = if raw_path.starts_with('/') {
        String::from(raw_path.as_str())
    } else {
        String::from(raw_path.as_str())
    };''', '''    let mut path_buf = [0u8; 128];
    let path_len = match copyin_c_string(path_ptr, &mut path_buf) {
        Ok(len) => len,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if path_len == 0 { return SyscallOutcome::errno(EINVAL); }
    let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
        Ok(s) => s,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let resolved_path = raw_path;''')

# 11. sys_execve
sysc = sysc.replace('''    pub fn sys_execve(args: [u64; 6]) -> SyscallOutcome {
        let path_ptr = args[0] as usize;
        let raw_path = match copyin_c_string(path_ptr, MAX_PATH_BYTES) {
            Ok(path) => path,
            Err(error) => return SyscallOutcome::errno(error),
        };
        
        let node = match crate::vfs::lookup(&raw_path) {''', '''    pub fn sys_execve(args: [u64; 6]) -> SyscallOutcome {
        let path_ptr = args[0] as usize;
        let mut path_buf = [0u8; 128];
        let path_len = match copyin_c_string(path_ptr, &mut path_buf) {
            Ok(len) => len,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s,
            Err(_) => return SyscallOutcome::errno(EINVAL),
        };
        let node = match crate::vfs::lookup(raw_path) {''')

# 12. sys_stat
sysc = sysc.replace('''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let raw_path = match super::copyin_c_string(ptr, super::MAX_PATH_BYTES) {
            Ok(path) => path,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let node = match crate::vfs::lookup(&raw_path) {''', '''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let mut path_buf = [0u8; 128];
        let path_len = match super::copyin_c_string(ptr, &mut path_buf) {
            Ok(len) => len,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s,
            Err(_) => return SyscallOutcome::errno(super::EINVAL),
        };
        let node = match crate::vfs::lookup(raw_path) {''')

# Fix starts_with correctly
sysc = sysc.replace("raw_path.starts_with('/')", 'raw_path.starts_with("/")')

# Fix lookup
sysc = sysc.replace('crate::vfs::lookup(&raw_path)', 'crate::vfs::lookup(raw_path)')

# Fix open.content.len()
sysc = sysc.replace('open.content.len()', 'open.content_len')
# Except we need to restore `match crate::vfs::read(&node.path) {` because it uses `.len()` on something else? No, `read` returns `Vec` but wait, `open.content.len()` is just the struct field.
# Oh, wait! `content.len()` inside `alloc_open_file` is `content.len()` on `&[u8]`, which is CORRECT!
sysc = sysc.replace('open.content_len', 'open.content_len') # no-op

# Oh wait! In `alloc_open_file`, it's `open.content.len()`. I've already replaced it earlier.
# In `sys_read`:
sysc = sysc.replace('open.content.len()', 'open.content_len')

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)

