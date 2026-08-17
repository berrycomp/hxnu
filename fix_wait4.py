with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

old_wait4 = '''    pub fn sys_wait4(args: [u64; 6]) -> SyscallOutcome {
        let parent_pid = crate::sched::current_process_id();
        let options = args[2] as u32;

        let WNOHANG = 1;
        
        loop {
            let status_ptr = args[1] as usize;
            
            crate::sched::with_scheduler(|_s| {
                match crate::sched::sys_reap_child(parent_pid) {'''

new_wait4 = '''    pub fn sys_wait4(args: [u64; 6]) -> SyscallOutcome {
        let parent_pid = crate::sched::current_process_id();
        let target_pid = args[0] as i64;
        let options = args[2] as u32;

        let WNOHANG = 1;
        
        loop {
            let status_ptr = args[1] as usize;
            
            crate::sched::with_scheduler(|_s| {
                match crate::sched::sys_reap_child(parent_pid, target_pid) {'''

sysc = sysc.replace(old_wait4, new_wait4)

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)
