pub mod manager;
pub mod mem;

use manager::{get_global_manager, global_allocate_pages, init_global_manager};

pub fn init() {
    init_global_manager();

    if let Some(manager) = get_global_manager() {
        let stats = manager.get_stats();
        crate::drivers::vga::print_string("Memory initialized: ");
        print_size(stats.total_memory);
        crate::drivers::vga::print_string(" total, ");
        print_size(stats.free_memory);
        crate::drivers::vga::print_string(" available\n");
    }
}

/// Simple page allocator implementation
pub fn allocate_pages(count: usize) -> Result<*mut u8, &'static str> {
    match global_allocate_pages(count) {
        Some(ptr) => Ok(ptr),
        None => Err("Out of memory"),
    }
}

/// Get memory manager statistics
pub fn get_memory_stats() -> Option<manager::MemoryStats> {
    get_global_manager().map(|m| m.get_stats())
}

/// Print memory statistics (useful for debugging)
pub fn print_memory_stats() {
    if let Some(stats) = get_memory_stats() {
        crate::drivers::vga::print_string("Memory stats:\n");
        crate::drivers::vga::print_string(" total: ");
        print_size(stats.total_memory);
        crate::drivers::vga::print_string("\n used: ");
        print_size(stats.used_memory);
        crate::drivers::vga::print_string("\n Free: ");
        print_size(stats.free_memory);
        crate::drivers::vga::print_string("\n");
    }
}

pub fn reserve_for_kernel(start: usize, size: usize) -> Result<(), &'static str> {
    if let Some(manager) = get_global_manager() {
        manager.reserve_region(start, size)?;
        manager.mark_kernel_loaded(start, size);
        Ok(())
    } else {
        Err("Memory manager not initialized")
    }
}

/// Find suitable kernel loading address
pub fn find_kernel_address(kernel_size: usize) -> Option<usize> {
    get_global_manager()?.find_kernel_location(kernel_size)
}

fn print_size(bytes: usize) {
    if bytes >= 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        print_decimal(mb);
        crate::drivers::vga::print_string("MB");
    } else if bytes >= 1024 {
        let kb = bytes / 1024;
        print_decimal(kb);
        crate::drivers::vga::print_string("KB");
    } else {
        print_decimal(bytes);
        crate::drivers::vga::print_string("B");
    }
}

fn print_decimal(mut num: usize) {
    if num == 0 {
        crate::drivers::vga::print_char(b'0');
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
        crate::drivers::vga::print_char(digits[j]);
    }
}
