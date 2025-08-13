#[allow(unused)]
use crate::kernel::loader;
use crate::{drivers, fs};

pub fn start() -> ! {
    drivers::vga::print_string("[stage2] Starting...");

    match drivers::disk::init() {
        Ok(()) => {
            let _ = drivers::vga::print_string("[stage2] Disk init OK\n");
        }
        Err(e) => {
            panic_msg("[stage2] Disk init failed: {}", e);
        }
    }

    if let Err(e) = try_mount_filesystems() {
        drivers::vga::print_string("[stage2] Filesystem init filesystem or skipped: ");
        drivers::vga::print_string(e);
        drivers::vga::print_string("\n");
    }
    let entry = match loader::find_and_load_kernel() {
        Ok(entry) => {
            drivers::vga::print_string("[stage2] kernel loaded, entry @ 0x");
            hex_u32(entry);
            drivers::vga::print_string("\n");
            entry
        }
        Err(e) => panic_msg("[stage2] kernel load FAILED: {}", e),
    };
    unsafe {
        core::arch::asm!("cli");
    }
    unsafe {
        let entry_fn: extern "C" fn() -> ! = core::mem::transmute(entry as usize);
        entry_fn();
    }
}
fn try_mount_filesystems() -> Result<(), &'static str> {
    //Still being worked on.
    Ok(())
}

fn panic_msg(prefix: &str, msg: &str) -> ! {
    drivers::vga::print_string(prefix);
    drivers::vga::print_string(msg);
    drivers::vga::print_string("\nHalted\n");
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

fn hex_u32(mut v: u32) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    drivers::vga::print_string("00000000");
    // Print into a small buffer then write—simple & no alloc
    let mut buf = [b'0'; 10];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..8 {
        let shift = 28 - (i * 4);
        let nibble = ((v >> shift) & 0xF) as usize;
        buf[2 + i] = HEX[nibble];
    }
    // SAFETY: buf is valid UTF-8 ASCI
    drivers::vga::print_string(core::str::from_utf8(&buf).unwrap());
}
