import re

with open('kernel/src/syscall.rs', 'r') as f:
    sysc = f.read()

# Fix sys_pipe
sysc = sysc.replace('path: alloc::string::String::from("pipe:read"), offset: 0, content: alloc::vec::Vec::new()',
                    'path: { let mut b=[0u8;128]; b[..9].copy_from_slice(b"pipe:read"); b }, path_len: 9, offset: 0, content: [0u8; 4096], content_len: 0')
sysc = sysc.replace('path: alloc::string::String::from("pipe:write"), offset: 0, content: alloc::vec::Vec::new()',
                    'path: { let mut b=[0u8;128]; b[..10].copy_from_slice(b"pipe:write"); b }, path_len: 10, offset: 0, content: [0u8; 4096], content_len: 0')

# Fix alloc_open_file missing content_len
# It currently has:
#                 content: { let mut cbuf=[0u8;4096]; let cl=core::cmp::min(content.len(),4096); cbuf[..cl].copy_from_slice(&content[..cl]); cbuf },
# We need to add content_len: ...
sysc = sysc.replace('cbuf[..cl].copy_from_slice(&content[..cl]); cbuf },', 'cbuf[..cl].copy_from_slice(&content[..cl]); cbuf },\n                content_len: core::cmp::min(content.len(), 4096),')

with open('kernel/src/syscall.rs', 'w') as f:
    f.write(sysc)
