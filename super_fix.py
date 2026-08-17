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

# 2. sys_stat
sysc = sysc.replace('''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let statbuf = args[1] as usize;
        let raw_path = match super::copyin_c_string(ptr, super::MAX_PATH_BYTES) {
            Ok(p) => p,
            Err(e) => return SyscallOutcome::errno(e),
        };
        let node = match crate::vfs::lookup(&raw_path) {''', '''    pub fn sys_stat(args: [u64; 6]) -> SyscallOutcome {
        let ptr = args[0] as usize;
        let statbuf = args[1] as usize;
        let mut path_buf = [0u8; 128];
        let path_len = match super::copyin_c_string(ptr, &mut path_buf) {
            Ok(l) => l,
            Err(e) => return SyscallOutcome::errno(e),
        };
        let raw_path = match core::str::from_utf8(&path_buf[..path_len]) {
            Ok(s) => s,
            Err(_) => return SyscallOutcome::errno(super::EINVAL),
        };
        let node = match crate::vfs::lookup(raw_path) {''')

# 3. sys_pipe
sysc = sysc.replace('''        global_table[r_idx as usize] = Some(super::OpenFile {
            ref_count: 1, path: alloc::string::String::from("pipe:read"), offset: 0, content: alloc::vec::Vec::new()
        });
        global_table[w_idx as usize] = Some(super::OpenFile {
            ref_count: 1, path: alloc::string::String::from("pipe:write"), offset: 0, content: alloc::vec::Vec::new()
        });''', '''        global_table[r_idx as usize] = Some(super::OpenFile {
            ref_count: 1, path: { let mut b=[0u8;128]; let s=b"pipe:read"; b[..s.len()].copy_from_slice(s); b }, path_len: 9, offset: 0, content: [0u8; 4096], content_len: 0
        });
        global_table[w_idx as usize] = Some(super::OpenFile {
            ref_count: 1, path: { let mut b=[0u8;128]; let s=b"pipe:write"; b[..s.len()].copy_from_slice(s); b }, path_len: 10, offset: 0, content: [0u8; 4096], content_len: 0
        });''')

# 4. sys_wait4
sysc = sysc.replace('''    pub fn sys_wait4(args: [u64; 6]) -> SyscallOutcome {
        let pid = args[0] as i64;
        let status_ptr = args[1] as usize;
        let options = args[2] as u32;

        let WNOHANG = 1;
        
        loop {
            crate::sched::with_scheduler(|_s| {
                match crate::sched::sys_reap_child(crate::sched::current_process_id()) {''', '''    pub fn sys_wait4(args: [u64; 6]) -> SyscallOutcome {
        let pid = args[0] as i64;
        let status_ptr = args[1] as usize;
        let options = args[2] as u32;

        let WNOHANG = 1;
        
        loop {
            crate::sched::with_scheduler(|_s| {
                match crate::sched::sys_reap_child(crate::sched::current_process_id(), pid) {''')

# Wait, let me check the EXACT text of sys_wait4 from syscall.rs because earlier it was "match crate::sched::sys_reap_child(parent_pid)".
