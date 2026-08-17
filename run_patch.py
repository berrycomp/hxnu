import re

with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

# 1. OpenFile
sysc = re.sub(r'pub struct OpenFile \{\s*pub ref_count: usize,\s*pub path: String,\s*pub offset: usize,\s*pub content: Vec<u8>,\s*\}',
r'''pub struct OpenFile {
    pub ref_count: usize,
    pub path: [u8; 128],
    pub path_len: usize,
    pub offset: usize,
    pub content: [u8; 4096],
    pub content_len: usize,
}''', sysc)
sysc = sysc.replace('#[derive(Clone)]\npub struct OpenFile', '#[derive(Copy, Clone)]\npub struct OpenFile')

# 2. sys_pipe
sysc = re.sub(r'path: alloc::string::String::from\("pipe:read"\), offset: 0, content: alloc::vec::Vec::new\(\)',
r'path: { let mut b=[0u8;128]; let s=b"pipe:read"; b[..s.len()].copy_from_slice(s); b }, path_len: 9, offset: 0, content: [0u8; 4096], content_len: 0', sysc)
sysc = re.sub(r'path: alloc::string::String::from\("pipe:write"\), offset: 0, content: alloc::vec::Vec::new\(\)',
r'path: { let mut b=[0u8;128]; let s=b"pipe:write"; b[..s.len()].copy_from_slice(s); b }, path_len: 10, offset: 0, content: [0u8; 4096], content_len: 0', sysc)

# 3. sys_wait4
sysc = sysc.replace('match crate::sched::sys_reap_child(crate::sched::current_process_id()) {', 'match crate::sched::sys_reap_child(crate::sched::current_process_id(), args[0] as i64) {')

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)

