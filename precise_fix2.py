import re

with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

sysc = sysc.replace('match alloc_open_file(node.path, content) {', 'match alloc_open_file(&node.path, &content) {')

# Fix copyin_bytes signature
sysc = re.sub(
    r'fn copyin_bytes\(ptr: usize, len: usize\) -> Result<Vec<u8>, i64> \{.*?Ok\(bytes\)\s*\}',
    r'''fn copyin_bytes(ptr: usize, buf: &mut [u8]) -> Result<(), i64> {
    uaccess::copyin(ptr, buf).map_err(map_uaccess_error)?;
    Ok(())
}''', sysc, flags=re.DOTALL)

# Fix copyin_c_string signature
sysc = re.sub(
    r'fn copyin_c_string\(ptr: usize, max_len: usize\) -> Result<String, i64> \{.*?Err\(ERANGE\)\s*\}',
    r'''fn copyin_c_string(ptr: usize, buf: &mut [u8]) -> Result<usize, i64> {
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
}''', sysc, flags=re.DOTALL)


with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)

