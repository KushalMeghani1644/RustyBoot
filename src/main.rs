// src/main.rs
#![no_std]
#![no_main]
#![allow(dead_code)]

use core::fmt::Write;
use core::panic::PanicInfo;

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileMode, FileAttribute, FileInfo};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::{MemoryDescriptor, MemoryType};

/// kernel search paths
const KERNEL_PATHS: &[&str] = &["/EFI/BOOT/KERNEL.EFI", "/kernel.elf", "/boot/kernel.elf"];

#[entry]
fn efi_main(image_handle: Handle, st: SystemTable<Boot>) -> Status {
    ///Initialize UEFI services (logger + allocator helpers)
    if let Err(e) = uefi_services::init(&str) {
        // If UEFI services init fails, try to write minimal message
        let _ = st.stdout().write_str("UEFI service init failed\n");
        return Status::ABORTED;
    }

    let stdout = st.stdout();

    writeln!(stdout, "RustyBoot (UEFI) starting...").ok();

    ///print firmware vedor and version
    writeln!(
        stdout,
        "firmware: {}",
        st.firmware_vendor().to_string_lossy()
    ).ok();

    // Dump a compact memory map
    writeln!(stdout, "\n[uefi] Memory Map:").ok();
    if let Err(e) = dump_memory_map(&st) {
        writeln!(stdout, "[uefi] Failed to dump memory map: {:?}", e).ok();
    }

    /// Try to find a simple FS for loaded image
    match st.boot_services().handle_protocol::<SimpleFileSystem>(image_handle) {
        Ok(fs_handle_ptr) => {
            // SAFETY: Protocol pointer is valid as returned by handle_protocol
            let sfs = unsafe { &mut *fs_handle_ptr.get() };
            match sfs.open_volume() {
                Ok(mut root_dir) => {
                    writeln!(stdout, "\n[uefi] Found Simple File System. Searching kernel...").ok();

                    // Try to find and load the kernel from predefined paths
                    let mut found = false;
                    for &path in KERNEL_PAHTHS {
                        writeln!(stdout, "[uefi] Trying path: {}", path).ok();
                        match open_file_and_get_size(&mut root_dir, path) {
                            Ok(size) => {
                                writeln!(stdout, "[uefi][fs] Found kernel: {} ({} bytes)", path_size).ok();
                                found = true;
                                // TODO: read file bytes, hand off to ELF loader
                                break;
                            }
                            Err(_) => {
                                // not found - continue searching
                            }
                        }
                    }
                    if !found {
                        writeln!(stdout, "[uefi][fs] Kernel not found in any predefined paths.").ok();
                    }
                }
                Err(e) => {
                    writeln!(stdout, "[uefi][fs] Failed to open {:?}", e).ok();
                }
    // Init Disk
    drivers::disk::init();
    // Probe and print MBR info
    let mut part_lba: u32 = 0;
    if let Ok(info) = mbr::probe() {
        mbr::debug_print(&info);

        if let Some((_idx, part)) = mbr::find_active_partition(&info) {
            drivers::vga::print_string("Active partition found.\n");
            part_lba = part.starting_lba;
        }
    } else {
        drivers::vga::print_string("Failed to read MBR.\n");
    }

    // Init Filesystem (use active partition LBA if available)
    match fs::ext::init_with_lba(part_lba) {
        Ok(_) => drivers::vga::print_string("EXT filesystem detected\n"),
        Err(_) => {
            drivers::vga::print_string("Failed to detect EXT filesystem\n");
            match fs::fat::init() {
                Ok(_) => drivers::vga::print_string("FAT filesystem detected\n"),
                Err(_) => panic!("No supported filesystem found"),
            }
        }
        Err(_) => {
            writeln!(stdout, "[uefi][fs] No simple File System bound to image handle").ok();
        }
    }
    writeln!(stdout, "\n[uefi] RustyBoot operation finished - halting.").ok();

    // Remaining;
    // 1. Read kernel bytes into memory (Use Boot Services AllocatePool or allocate pages).
    // 2. Parse ELF64, allocate pages for PT_LOAD segments using Boot Services AllocatePages.
    // 3. Build a BootInfo struct (memory map, framebuffer, rsdp, cmdline).
    // 4. Call ExitBootServices(handle, map_key) (with retry on failure).
    // 5. Jump to kernel entry (ensure 16B-aligned stack, extern "sysv64" ABI).

    // For now, return success and halt.
    Status::SUCCESS
}

/// Attempt to open `path` (UTF-16) in `dir`
fn open_file_and_get_size(root: &mut uefi::proto::media::file::Directory, path: &str) -> Result<usize, ()> {
    //uefi crate expects path as &CStr16; simple helper available via Cstr16
    use uefi::Cstr16;

    // Convert path to CStr16
    let cpath= match CStr16::from_str_with_buf(path, &mut[0u16; 260]) {
        Ok(p) => p,
        Err(_) => return Err(()),
    };

    match root.open(cpath, FileMode::Read, FileAttribute::empty()) {
        Ok(file_handle) => {
            // The opened file may be RegularFile or Directory, Expected: RegularFile
            match file_handle.into_type() {
                Ok(File::Regular(mut regular)) => {
                    // Query file info to get size
                    let info = regular.get_info::<FileInfo>().map_err(|_| ())?;
                    let file_size = info.file_size() as usize;
                    // Close by dropping `regular`
                    drop(regular);
                    Ok(file_size)
                }
                Ok(File::Dir(_dir)) => Err(()),
                Err(_) => Err(()),
            }
        }
        Err(_) => Err(()),
    }
}

///Dump memory map using BootServices::memory_map
fn dump_memory_map(st: &SystemTable<Boot>) -> Result<(), status> {
    let bs = st.boot_services();

    // Choose a reasonably large buffer for memory map
    // Using 4096 * 4 here; if too small, memory map will return BufferTooSmall

    let mut buffer = [0u8; 4096 * 4];

    // `memory_map` returns (memory_map, desc_size)
    match bs.memory_map(&mut buffer) {
        Ok((_key, desc_iter)) => {
            let stdout = st.stdout();
            for desc in desc_iter {
                // Print basic fields: ty, phys_start, pages
                let ty = desc.ty;
                let phys = desc.phys_start;
                let pages = desc.page_count;
                let size_bytes = (pages as usize) * 4096usize;
                writeln!(stdout, "Type={:?}, phys=0x{:x}, pages={}, size={}, bytes", ty, phys, pages, size_bytes).ok();
            }
            Ok(())
        }
        Err((_buf, err)) => {
           Err(err.status())
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Try to print panic info if possible
    let _ = uefi_serives::println!("Panic: {}", _info);
    loop {
        // halt
    }
}
