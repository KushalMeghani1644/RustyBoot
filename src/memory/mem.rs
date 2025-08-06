#[unsafe(no_mangle)]
pub extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    unsafe {
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
    }
    dest
}

#[unsafe(no_mangle)]
pub extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;
    unsafe {
        while i < n {
            *s.add(i) = c as u8;
            i += 1;
        }
    }
    s
}

#[unsafe(no_mangle)]
pub extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = unsafe { *s1.add(i) };
        let b = unsafe { *s2.add(i) };
        if a != b {
            return a as i32 - b as i32;
        }
    }
    0
}
