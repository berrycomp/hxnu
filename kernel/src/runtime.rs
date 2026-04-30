use core::ffi::{c_char, c_int, c_void};

// Provide the C memory/string entry points expected by freestanding builds.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, len: usize) -> *mut c_void {
    let dest = dest.cast::<u8>();
    let src = src.cast::<u8>();

    let mut index = 0usize;
    while index < len {
        unsafe {
            *dest.add(index) = *src.add(index);
        }
        index += 1;
    }

    dest.cast::<c_void>()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, len: usize) -> *mut c_void {
    let dest = dest.cast::<u8>();
    let src = src.cast::<u8>();

    let dest_addr = dest as usize;
    let src_addr = src as usize;
    if dest_addr <= src_addr || dest_addr >= src_addr.saturating_add(len) {
        return unsafe { memcpy(dest.cast::<c_void>(), src.cast::<c_void>(), len) };
    }

    let mut index = len;
    while index != 0 {
        index -= 1;
        unsafe {
            *dest.add(index) = *src.add(index);
        }
    }

    dest.cast::<c_void>()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut c_void, value: c_int, len: usize) -> *mut c_void {
    let dest = dest.cast::<u8>();
    let byte = value as u8;

    let mut index = 0usize;
    while index < len {
        unsafe {
            *dest.add(index) = byte;
        }
        index += 1;
    }

    dest.cast::<c_void>()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(left: *const c_void, right: *const c_void, len: usize) -> c_int {
    let left = left.cast::<u8>();
    let right = right.cast::<u8>();

    let mut index = 0usize;
    while index < len {
        let lhs = unsafe { *left.add(index) };
        let rhs = unsafe { *right.add(index) };
        if lhs != rhs {
            return lhs as c_int - rhs as c_int;
        }
        index += 1;
    }

    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(text: *const c_char) -> usize {
    let text = text.cast::<u8>();
    let mut len = 0usize;
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    len
}
