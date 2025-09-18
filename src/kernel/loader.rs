#[allow(dead_code)]

use uefi::prelude::*;
use uefi::proto::media::file::{Directory, File, FileModule, FileAttribute, FileInfo};
use uefi::table::boot::{AllocateType, MemoryType};

use core::ptr::copy_nonoverlapping;

/// Predefined kernel paths
const KERNEL_PATHS: &[&str] = &["/EFI/BOOT/KERNEL.EFI", "/kernel.elf", "/boot/kernel.elf"];

/// Main entry: find and load kernel
pub fn find_and_load_kernel(st: &SystemTable<Boot>, root: &mut Directory) -> Result<usize, &'static str> {
    for &path in KERNEL_PATHS {
        writeln!(st.stdout(), "Trying: {}", path).ok();
        if let Ok(entry) = load_kernel_from_path(st, root, path) {
            writeln!(st.stdout(), "Loaded kernel at 0x{:X}", entry).ok();
            return Ok(entry);
        }
    }
    Err("No kernel found")
}

/// Load kernel from a given path
fn load_kernel_from_path(st: &SystemTable<Boot>, root: &mut Directory, path: &str) -> Result<usize, &'static str> {
    let kernel_buf = read_file_uefi(root, path)?;
    writeln!(st.stdout(), "Kernel size: {} bytes", kernel_buf.len()).ok();

    // Allocate pages for the kernel
    let kernel_pages = (kernel_buf.len() + 0xFFF) / 0x1000; // round up
    let kernel_addr = st.boot_services().allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        kernel_pages
    ).map_err(|_| "Failed to allocate pages")?;

    // Parse ELF64 and load segments
    parse_and_load_elf64(kernel_buf.as_slice(), kernel_addr)?;

    Ok(kernel_addr)
}

/// Read a file from UEFI SimpleFileSystem
fn read_file_uefi(root: &mut Directory, path: &str) -> Result<Vec<u8>, &'static str> {
    use uefi::CStr16;
    let mut buf16 = [0u16; 260];
    let cpath = CStr16::from_str_with_buf(path, &mut buf16).map_err(|_| "Invalid path")?;
    let file_handle = root.open(cpath, FileMode::Read, FileAttribute::empty()).map_err(|_| "Failed to open file")?;
    
    let mut file = match file_handle.into_type().map_err(|_| "Invalid file type")? {
        File::Regular(f) => f,
        _ => return Err("Not a regular file"),
    };

    let info = file.get_info::<FileInfo>().map_err(|_| "Failed to get file info")?;
    let size = info.file_size() as usize;
    let mut buf = vec![0u8; size];
    file.read(&mut buf).map_err(|_| "Failed to read file")?;
    Ok(buf)
}

/// Parse ELF64 and load PT_LOAD segments
fn parse_and_load_elf64(data: &[u8], load_addr: usize) -> Result<usize, &'static str> {
    if data.len() < 64 { return Err("ELF too small"); }
    if &data[0..4] != b"\x7fELF" { return Err("Not ELF"); }
    if data[4] != 2 { return Err("Not 64-bit ELF"); } // EI_CLASS
    if data[5] != 1 { return Err("Not little-endian"); } // EI_DATA

    // Entry point offset 24
    let entry = u64::from_le_bytes(data[24..32].try_into().unwrap()) as usize;

    // Program header table
    let ph_offset = u64::from_le_bytes(data[32..40].try_into().unwrap()) as usize;
    let ph_entry_size = u16::from_le_bytes(data[54..56].try_into().unwrap()) as usize;
    let ph_count = u16::from_le_bytes(data[56..58].try_into().unwrap()) as usize;

    for i in 0..ph_count {
        let ph_base = ph_offset + i * ph_entry_size;
        if ph_base + ph_entry_size > data.len() { continue; }

        let ph_type = u32::from_le_bytes(data[ph_base..ph_base+4].try_into().unwrap());
        if ph_type != 1 { continue; } // PT_LOAD

        let file_offset = u64::from_le_bytes(data[ph_base+8..ph_base+16].try_into().unwrap()) as usize;
        let virt_addr = u64::from_le_bytes(data[ph_base+16..ph_base+24].try_into().unwrap()) as usize;
        let file_size = u64::from_le_bytes(data[ph_base+32..ph_base+40].try_into().unwrap()) as usize;
        let mem_size = u64::from_le_bytes(data[ph_base+40..ph_base+48].try_into().unwrap()) as usize;

        unsafe {
            // Copy segment
            copy_nonoverlapping(data[file_offset..file_offset+file_size].as_ptr(), virt_addr as *mut u8, file_size);
            // Zero BSS
            if mem_size > file_size {
                core::ptr::write_bytes((virt_addr + file_size) as *mut u8, 0, mem_size - file_size);
            }
        }
    }

    Ok(entry)
}

/// Jump to kernel after exiting boot services
pub fn jump_to_kernel(st: &SystemTable<Boot>, image_handle: Handle, entry_point: usize) -> ! {
    let map_size = 4096 * 4;
    let mut mem_map_buf = [0u8; 4096*4];
    let (_key, _desc_iter) = st.boot_services().memory_map(&mut mem_map_buf)
        .expect("Failed to get memory map");

    st.exit_boot_services(image_handle, _key).expect("ExitBootServices failed");

    let kernel: extern "sysv64" fn() -> ! = unsafe { core::mem::transmute(entry_point) };
    kernel();
}
