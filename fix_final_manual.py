import re

with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

# Fix OpenFile
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

# Fix copyin_bytes
sysc = sysc.replace('''fn copyin_bytes(ptr: usize, len: usize) -> Result<Vec<u8>, i64> {
    let mut bytes = vec![0u8; len];
    uaccess::copyin(ptr, &mut bytes).map_err(map_uaccess_error)?;
    Ok(bytes)
}''', '''fn copyin_bytes(ptr: usize, buf: &mut [u8]) -> Result<(), i64> {
    uaccess::copyin(ptr, buf).map_err(map_uaccess_error)?;
    Ok(())
}''')

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)

