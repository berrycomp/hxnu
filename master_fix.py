import re

with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

# 1. Fix OpenFile Struct
sysc = re.sub(r'#\[derive\(Clone\)\]\s*pub struct OpenFile \{\s*pub ref_count: usize,\s*pub path: String,\s*pub offset: usize,\s*pub content: Vec<u8>,\s*\}', 
'''#[derive(Copy, Clone)]
pub struct OpenFile {
    pub ref_count: usize,
    pub path: [u8; 128],
    pub path_len: usize,
    pub offset: usize,
    pub content: [u8; 4096],
    pub content_len: usize,
}''', sysc)

# 2. Fix alloc_open_file
sysc = sysc.replace('path: path.clone(),', 'path: { let mut b=[0u8;128]; let l=core::cmp::min(path.len(),128); b[..l].copy_from_slice(path.as_bytes()); b },\n                path_len: core::cmp::min(path.len(), 128),')
sysc = sysc.replace('content: content.clone(),', 'content: { let mut cbuf=[0u8;4096]; let cl=core::cmp::min(content_slice.len(),4096); cbuf[..cl].copy_from_slice(&content_slice[..cl]); cbuf },\n                content_len: core::cmp::min(content_slice.len(), 4096),')
sysc = sysc.replace('fn alloc_open_file(path: String, content: Vec<u8>) -> Result<i64, i64>', 'fn alloc_open_file(path: &str, content_slice: &[u8]) -> Result<i64, i64>')

# 3. Fix sys_pipe
sysc = sysc.replace('path: alloc::string::String::from("pipe:read"), offset: 0, content: alloc::vec::Vec::new()',
                    'path: { let mut b=[0u8;128]; b[..9].copy_from_slice(b"pipe:read"); b }, path_len: 9, offset: 0, content: [0u8; 4096], content_len: 0')
sysc = sysc.replace('path: alloc::string::String::from("pipe:write"), offset: 0, content: alloc::vec::Vec::new()',
                    'path: { let mut b=[0u8;128]; b[..10].copy_from_slice(b"pipe:write"); b }, path_len: 10, offset: 0, content: [0u8; 4096], content_len: 0')

# 4. Fix copyin_bytes
sysc = sysc.replace('''fn copyin_bytes(ptr: usize, len: usize) -> Result<Vec<u8>, i64> {
    let mut bytes = vec![0u8; len];
    uaccess::copyin(ptr, &mut bytes).map_err(map_uaccess_error)?;
    Ok(bytes)
}''', '''fn copyin_bytes(ptr: usize, buf: &mut [u8]) -> Result<(), i64> {
    uaccess::copyin(ptr, buf).map_err(map_uaccess_error)?;
    Ok(())
}''')

# 5. Fix copyin_c_string
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

# 6. Fix sys_openat
old_openat = '''    let raw_path = match copyin_c_string(path_ptr, MAX_PATH_BYTES) {
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
    };'''
new_openat = '''    let mut path_buf = [0u8; 128];
    let path_len = match copyin_c_string(path_ptr, &mut path_buf) {
        Ok(len) => len,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if path_len == 0 { return SyscallOutcome::errno(EINVAL); }
    let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
        Ok(s) => s, Err(_) => return SyscallOutcome::errno(EINVAL)
    };
    let resolved_path = raw_path;'''
sysc = sysc.replace(old_openat, new_openat)
sysc = sysc.replace('match crate::vfs::lookup(&resolved_path)', 'match crate::vfs::lookup(resolved_path)')

# 7. Fix sys_stat
old_stat = '''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let raw_path = match super::copyin_c_string(ptr, super::MAX_PATH_BYTES) {
            Ok(path) => path,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let node = match crate::vfs::lookup(&raw_path) {'''
new_stat = '''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let mut path_buf = [0u8; 128];
        let path_len = match super::copyin_c_string(ptr, &mut path_buf) {
            Ok(len) => len,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s, Err(_) => return SyscallOutcome::errno(super::EINVAL)
        };
        let node = match crate::vfs::lookup(raw_path) {'''
sysc = sysc.replace(old_stat, new_stat)

# 8. Fix sys_execve
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
            Ok(s) => s, Err(_) => return SyscallOutcome::errno(EINVAL)
        };
        let node = match crate::vfs::lookup(raw_path) {'''
sysc = sysc.replace(old_execve, new_execve)

# 9. Fix write_text
old_write = '''    let bytes = match copyin_bytes(ptr, count) {
        Ok(bytes) => bytes,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let text = sanitize_for_console(&bytes);'''
new_write = '''    let mut buf = [0u8; 4096];
    let read_len = core::cmp::min(count, 4096);
    if let Err(error) = copyin_bytes(ptr, &mut buf[..read_len]) {
        return SyscallOutcome::errno(error);
    }
    let text = sanitize_for_console(&buf[..read_len]);'''
sysc = sysc.replace(old_write, new_write)

# 10. Fix sanitize_for_console
old_sanitize = '''fn sanitize_for_console(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'\\n' | b'\\r' | b'\\t' | 0x20..=0x7e => text.push(byte as char),
            _ => text.push('?'),
        }
    }
    text
}'''
new_sanitize = '''fn sanitize_for_console(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("")
}'''
sysc = sysc.replace(old_sanitize, new_sanitize)

# 11. Fix map_uaccess_error recursion
sysc = sysc.replace('''fn map_uaccess_error(error: UserCopyError) -> i64 {
    error.as_errno()
}''', '''fn map_uaccess_error(error: UserCopyError) -> i64 {
    error.as_errno()
}''') # No-op if not messed up

# 12. Fix open.content.len() and open.content
sysc = sysc.replace('open.content.len()', 'open.content_len')
# `let bytes = &open.content[open.offset..open.offset + read_len];` is valid for fixed array too.
sysc = sysc.replace('let content_len = match crate::vfs::read(&node.path) {', 'let content_len = match crate::vfs::read(&node.path) {')

# Wait, `alloc_open_file` is called in `sys_openat` with:
# `match alloc_open_file(node.path.clone(), content_buf[..content_len].to_vec())`
old_alloc_call = 'match alloc_open_file(node.path.clone(), content_buf[..content_len].to_vec()) {'
new_alloc_call = 'match alloc_open_file(&node.path, &content_buf[..content_len]) {'
sysc = sysc.replace(old_alloc_call, new_alloc_call)

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)

# We also need to fix CMakeLists.txt to ensure we use -nostdlib and --emit obj
