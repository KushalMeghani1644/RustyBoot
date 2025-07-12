#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let msg = b"Hello from bootloader!\n";
    for &b in msg {
        unsafe {
            core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") b);
        }
    }

    loop {}
}
