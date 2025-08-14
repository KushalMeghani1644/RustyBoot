use crate::{drivers, fs, memory};

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
    // Read kernel file
    let kernel_buffer = fs::ext::read_file(path).map_err(|_| "Failed to read kernel file")?;

    drivers::vga::print_string("Kernel file size: ");
    print_file_size(kernel_buffer.as_slice().len());
    drivers::vga::print_string("\n");

    // Find suitable memory location for kernel
    let kernel_load_addr = memory::find_kernel_address(kernel_buffer.as_slice().len())
        .ok_or("Cannot find suitable kernel load address")?;

    drivers::vga::print_string("Loading kernel at: 0x");
    print_hex_usize(kernel_load_addr);
    drivers::vga::print_string("\n");

    // Parse ELF and get entry point
    let entry_point = parse_and_load_elf(kernel_buffer.as_slice(), kernel_load_addr)?;

    // Reserve the memory region for the kernel
    memory::reserve_for_kernel(kernel_load_addr, kernel_buffer.as_slice().len())?;

    Ok(entry_point)
}

fn parse_and_load_elf(data: &[u8], load_addr: usize) -> Result<u32, &'static str> {
    if data.len() < 52 {
        return Err("Invalid ELF file");
    }

    // Check ELF magic
    if &data[0..4] != b"\x7fELF" {
        return Err("Not an ELF file");
    }

    // Verify it's 32-bit ELF
    if data[4] != 1 {
        return Err("Only 32-bit ELF supported");
    }

    // Verify it's little-endian
    if data[5] != 1 {
        return Err("Only little-endian ELF supported");
    }

    // Get entry point from ELF header (offset 24 for 32-bit ELF)
    let entry_point = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

    drivers::vga::print_string("ELF entry point: 0x");
    print_hex_u32(entry_point);
    drivers::vga::print_string("\n");

    // Get program header table information
    let ph_offset = u32::from_le_bytes([data[28], data[29], data[30], data[31]]) as usize;
    let ph_size = u16::from_le_bytes([data[42], data[43]]) as usize;
    let ph_count = u16::from_le_bytes([data[44], data[45]]) as usize;

    drivers::vga::print_string("Loading ELF segments...\n");

    // Process program headers (load segments)
    for i in 0..ph_count {
        let ph_base = ph_offset + (i * ph_size);
        if ph_base + 32 > data.len() {
            continue;
        }

        let ph_type = u32::from_le_bytes([
            data[ph_base],
            data[ph_base + 1],
            data[ph_base + 2],
            data[ph_base + 3],
        ]);

        // PT_LOAD = 1
        if ph_type == 1 {
            let file_offset = u32::from_le_bytes([
                data[ph_base + 4],
                data[ph_base + 5],
                data[ph_base + 6],
                data[ph_base + 7],
            ]) as usize;

            let virt_addr = u32::from_le_bytes([
                data[ph_base + 8],
                data[ph_base + 9],
                data[ph_base + 10],
                data[ph_base + 11],
            ]) as usize;

            let file_size = u32::from_le_bytes([
                data[ph_base + 16],
                data[ph_base + 17],
                data[ph_base + 18],
                data[ph_base + 19],
            ]) as usize;

            let mem_size = u32::from_le_bytes([
                data[ph_base + 20],
                data[ph_base + 21],
                data[ph_base + 22],
                data[ph_base + 23],
            ]) as usize;

            // Copy segment data to memory
            if file_offset + file_size <= data.len() {
                unsafe {
                    crate::memory::mem::memcpy(
                        virt_addr as *mut u8,
                        data.as_ptr().add(file_offset),
                        file_size,
                    );

                    // Zero out the rest if mem_size > file_size (BSS section)
                    if mem_size > file_size {
                        crate::memory::mem::memset(
                            (virt_addr + file_size) as *mut u8,
                            0,
                            mem_size - file_size,
                        );
                    }
                }

                drivers::vga::print_string("Loaded segment: 0x");
                print_hex_usize(virt_addr);
                drivers::vga::print_string(" (");
                print_file_size(mem_size);
                drivers::vga::print_string(")\n");
            }
        }
    }

    Ok(entry_point)
}

pub fn jump_to_kernel(entry_point: u32) -> ! {
    drivers::vga::print_string("Jumping to kernel at 0x");
    print_hex_u32(entry_point);
    drivers::vga::print_string("\n");

    // Print final memory statistics
    memory::print_memory_stats();

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

// Helper functions for printing
fn print_hex_u32(mut v: u32) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    for i in 0..8 {
        let shift = 28 - (i * 4);
        let nibble = ((v >> shift) & 0xF) as usize;
        drivers::vga::print_char(HEX[nibble]);
    }
}

fn print_hex_usize(v: usize) {
    print_hex_u32(v as u32);
}

fn print_file_size(bytes: usize) {
    if bytes >= 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        print_decimal(mb);
        drivers::vga::print_string("MB");
    } else if bytes >= 1024 {
        let kb = bytes / 1024;
        print_decimal(kb);
        drivers::vga::print_string("KB");
    } else {
        print_decimal(bytes);
        drivers::vga::print_string("B");
    }
}

fn print_decimal(mut num: usize) {
    if num == 0 {
        drivers::vga::print_char(b'0');
        return;
    }

    let mut digits = [0u8; 20];
    let mut i = 0;

    while num > 0 && i < digits.len() {
        digits[i] = (num % 10) as u8 + b'0';
        num /= 10;
        i += 1;
    }

    for j in (0..i).rev() {
        drivers::vga::print_char(digits[j]);
    }
}
