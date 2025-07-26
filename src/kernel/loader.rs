use crate::{drivers, fs};

const KERNEL_PATHS: &[&str] = &["/boot/vmlinuz", "/boot/kernel", "/kernel", "/boot/bzImage"];

pub fn find_and_load_kernel() -> Result<u32, &'static str> {
    for path in KERNEL_PATHS {
        drivers::vga::print_string("Trying: ");
        drivers::vga::print_string(path);
        drivers::vga::print_string("\n");

        match load_kernel_from_path(path) {
            Ok(entry_point) => return Ok(entry_point),
            Err(_) => continue,
        }
    }
    Err("No kernel found")
}

fn load_kernel_from_path(path: &str) -> Result<u32, &'static str> {
    match fs::ext::read_file(path) {
        Ok(kernel_buffer) => {
            let entry_point = parse_and_load_elf(kernel_buffer.as_slice())?;
            Ok(entry_point)
        }
        Err(_) => Err("Failed to read kernel file"),
    }
}

fn parse_and_load_elf(data: &[u8]) -> Result<u32, &'static str> {
    if data.len() < 52 {
        return Err("Invalid ELF file");
    }

    // Check ELF magic
    if &data[0..4] != b"\x7fELF" {
        return Err("Not an ELF file");
    }

    // Get entry point from ELF header (offset 24 for 32-bit ELF)
    let entry_point = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

    Ok(entry_point)
}

pub fn jump_to_kernel(entry_point: u32) -> ! {
    unsafe {
        core::arch::asm!(
            "cli",
            "mov esp, {stack}",
            "push 0",
            "jmp {entry}",
            stack = const 0x90000,
            entry = in(reg) entry_point,
            options(noreturn)
        );
    }
}
