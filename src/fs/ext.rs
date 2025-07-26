use crate::drivers;

#[repr(C, packed)]
pub struct Ext2Superblock {
    inodes_count: u32,
    blocks_count: u32,
    r_blocks_count: u32,
    free_blocks_count: u32,
    free_inodes_count: u32,
    first_data_block: u32,
    log_block_size: u32,
    log_frag_size: u32,
    blocks_per_group: u32,
    frags_per_group: u32,
    inodes_per_group: u32,
    mtime: u32,
    wtime: u32,
    mnt_count: u16,
    max_mnt_count: u16,
    magic: u16,
    state: u16,
    errors: u16,
    minor_rev_level: u16,
    lastcheck: u32,
    checkinterval: u32,
    creator_os: u32,
    rev_level: u32,
    def_resuid: u16,
    def_resgid: u16,
}

// Simple fixed-size buffer instead of Vec
const MAX_FILE_SIZE: usize = 1024 * 1024; // 1MB max file size
#[allow(dead_code)]
pub struct FileBuffer {
    data: [u8; MAX_FILE_SIZE],
    size: usize,
}

#[allow(dead_code)]
impl FileBuffer {
    pub fn new() -> Self {
        Self {
            data: [0; MAX_FILE_SIZE],
            size: 0,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.size]
    }
}

pub fn init() -> Result<(), &'static str> {
    // Read superblock at offset 1024
    let mut buffer = [0u8; 1024];
    drivers::disk::read_sectors(2, 2, &mut buffer)?;

    let superblock = unsafe { &*(buffer.as_ptr() as *const Ext2Superblock) };

    // Check magic number (0xEF53 for ext2/3/4)
    if superblock.magic != 0xEF53 {
        return Err("Not an EXT filesystem");
    }

    drivers::vga::print_string("EXT filesystem initialized\n");
    Ok(())
}

pub fn read_file(_path: &str) -> Result<FileBuffer, &'static str> {
    // Implement file reading logic
    // This would involve:
    // 1. Parse directory structure
    // 2. Find file inode
    // 3. Read file data blocks
    Err("Not implemented")
}
