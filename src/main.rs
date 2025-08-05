#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod boot;
mod drivers;
mod fs;
mod kernel;
mod memory;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Basic Serial Output (optional early output)
    let msg = b"Hello from bootloader!\n";
    for &b in msg {
        unsafe {
            core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") b);
        }
    }

    // VGA Output
    drivers::vga::init();
    drivers::vga::print_string("RustyBoot v0.1.0 - Initializing...\n");

    // Init Memory
    memory::init();

    // Init Disk
    drivers::disk::init();

    // Init Filesystem
    match fs::ext::init() {
        Ok(_) => drivers::vga::print_string("EXT filesystem detected\n"),
        Err(_) => {
            drivers::vga::print_string("Failed to detect EXT filesystem\n");
            match fs::fat::init() {
                Ok(_) => drivers::vga::print_string("FAT filesystem detected\n"),
                Err(_) => panic!("No supported filesystem found"),
            }
        }
    }

    // Kernel Loading
    drivers::vga::print_string("Searching for kernel...\n");
    match kernel::loader::find_and_load_kernel() {
        Ok(kernel_entry) => {
            drivers::vga::print_string("Kernel loaded successfully\n");
            drivers::vga::print_string("Jumping to kernel...\n");
            kernel::loader::jump_to_kernel(kernel_entry);
        }
        Err(_) => panic!("Failed to load kernel"),
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    drivers::vga::print_string("PANIC: Bootloader panic occurred\n");
    loop {}
}
