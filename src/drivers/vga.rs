const VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;
static mut CURSOR_POS: usize = 0;

pub fn init() {
    unsafe {
        CURSOR_POS = 0;
        clear_screen();
    }
}

pub fn clear_screen() {
    unsafe {
        for i in 0..80 * 25 * 2 {
            *((VGA_BUFFER as usize + i) as *mut u8) = if i % 2 == 0 { b' ' } else { 0x07 };
        }
        CURSOR_POS = 0;
    }
}

pub fn print_string(s: &str) {
    for byte in s.bytes() {
        print_char(byte);
    }
}

pub fn print_char(c: u8) {
    unsafe {
        if c == b'\n' {
            CURSOR_POS = ((CURSOR_POS / 160) + 1) * 160;
        } else {
            *((VGA_BUFFER as usize + CURSOR_POS) as *mut u8) = c;
            *((VGA_BUFFER as usize + CURSOR_POS + 1) as *mut u8) = 0x07;
            CURSOR_POS += 2;
        }

        if CURSOR_POS >= 80 * 25 * 2 {
            scroll_up();
            CURSOR_POS = 80 * 24 * 2;
        }
    }
}

fn scroll_up() {
    unsafe {
        for i in 0..(80 * 24 * 2) {
            *((VGA_BUFFER as usize + i) as *mut u8) = *((VGA_BUFFER as usize + i + 160) as *mut u8);
        }
        for i in (80 * 24 * 2)..(80 * 25 * 2) {
            *((VGA_BUFFER as usize + i) as *mut u8) = if i % 2 == 0 { b' ' } else { 0x07 };
        }
    }
}
