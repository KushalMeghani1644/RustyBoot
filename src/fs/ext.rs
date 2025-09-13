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

#[repr(C, packed)]
pub struct Ext2BlockGroupDescriptor {
    block_bitmap: u32,
    inode_bitmap: u32,
    inode_table: u32,
    free_blocks_count: u16,
    free_inodes_count: u16,
    used_dirs_count: u16,
    pad: u16,
    reserved: [u32; 3],
}

#[repr(C, packed)]
pub struct Ext2Inode {
    mode: u16,
    uid: u16,
    size: u32,
    atime: u32,
    ctime: u32,
    mtime: u32,
    dtime: u32,
    gid: u16,
    links_count: u16,
    blocks: u32,
    flags: u32,
    osd1: u32,
    block: [u32; 15], // Direct blocks (12) + indirect blocks (3)
    generation: u32,
    file_acl: u32,
    dir_acl: u32,
    faddr: u32,
    osd2: [u32; 3],
}

#[repr(C, packed)]
pub struct Ext2DirEntry {
    inode: u32,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
    // name follows (variable length)
}

// File types
const EXT2_FT_UNKNOWN: u8 = 0;
const EXT2_FT_REG_FILE: u8 = 1;
const EXT2_FT_DIR: u8 = 2;
const EXT2_FT_CHRDEV: u8 = 3;
const EXT2_FT_BLKDEV: u8 = 4;
const EXT2_FT_FIFO: u8 = 5;
const EXT2_FT_SOCK: u8 = 6;
const EXT2_FT_SYMLINK: u8 = 7;

// Inode modes
const EXT2_S_IFREG: u16 = 0x8000; // Regular file
const EXT2_S_IFDIR: u16 = 0x4000; // Directory

// Global filesystem state
static mut SUPERBLOCK: Option<Ext2Superblock> = None;
static mut BLOCK_SIZE: usize = 0;
static mut SECTORS_PER_BLOCK: usize = 0;

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

    // Calculate block size
    let block_size = 1024 << superblock.log_block_size;
    let sectors_per_block = block_size / 512;

    unsafe {
        SUPERBLOCK = Some(*superblock);
        BLOCK_SIZE = block_size;
        SECTORS_PER_BLOCK = sectors_per_block;
    }

    drivers::vga::print_string("EXT filesystem initialized\n");
    Ok(())
}

fn read_block(block_num: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
    let block_size = unsafe { BLOCK_SIZE };
    let sectors_per_block = unsafe { SECTORS_PER_BLOCK };

    if buffer.len() < block_size {
        return Err("Buffer too small for block");
    }

    let start_sector = (block_num as usize) * sectors_per_block;
    drivers::disk::read_sectors(start_sector, sectors_per_block, &mut buffer[..block_size])
}

fn get_inode(inode_num: u32) -> Result<Ext2Inode, &'static str> {
    let superblock = unsafe { SUPERBLOCK.as_ref().ok_or("Filesystem not initialized")? };

    // Calculate which block group contains this inode
    let group = (inode_num - 1) / superblock.inodes_per_group;
    let local_inode = (inode_num - 1) % superblock.inodes_per_group;

    // Read block group descriptor
    let bgd_block = if superblock.first_data_block == 0 {
        2
    } else {
        1
    };
    let mut bgd_buffer = [0u8; 4096];
    read_block(bgd_block, &mut bgd_buffer)?;

    let bgd_offset = (group as usize) * core::mem::size_of::<Ext2BlockGroupDescriptor>();
    let bgd =
        unsafe { &*((bgd_buffer.as_ptr().add(bgd_offset)) as *const Ext2BlockGroupDescriptor) };

    // Read inode from inode table
    let inode_size = 128; // Standard inode size
    let inodes_per_block = unsafe { BLOCK_SIZE } / inode_size;
    let inode_block = bgd.inode_table + (local_inode / inodes_per_block as u32);
    let inode_offset = ((local_inode % inodes_per_block as u32) as usize) * inode_size;

    let mut inode_buffer = [0u8; 4096];
    read_block(inode_block, &mut inode_buffer)?;

    let inode = unsafe { &*((inode_buffer.as_ptr().add(inode_offset)) as *const Ext2Inode) };

    Ok(*inode)
}

fn find_file_in_directory(dir_inode: &Ext2Inode, filename: &str) -> Result<u32, &'static str> {
    let mut buffer = [0u8; 4096];

    // Only handle direct blocks for simplicity
    for &block_num in &dir_inode.block[..12] {
        if block_num == 0 {
            break;
        }

        read_block(block_num, &mut buffer)?;

        let mut offset = 0;
        while offset < unsafe { BLOCK_SIZE } {
            let entry = unsafe { &*((buffer.as_ptr().add(offset)) as *const Ext2DirEntry) };

            if entry.inode == 0 || entry.rec_len == 0 {
                break;
            }

            // Extract filename
            let name_ptr = unsafe { buffer.as_ptr().add(offset + 8) };
            let name_slice =
                unsafe { core::slice::from_raw_parts(name_ptr, entry.name_len as usize) };

            // Convert to string and compare
            let mut name_buf = [0u8; 256];
            if entry.name_len as usize <= name_buf.len() {
                name_buf[..entry.name_len as usize].copy_from_slice(name_slice);
                let name_str = core::str::from_utf8(&name_buf[..entry.name_len as usize])
                    .map_err(|_| "Invalid filename encoding")?;

                if name_str == filename {
                    return Ok(entry.inode);
                }
            }

            offset += entry.rec_len as usize;
        }
    }

    Err("File not found")
}

fn read_inode_data(inode: &Ext2Inode, buffer: &mut FileBuffer) -> Result<(), &'static str> {
    let file_size = inode.size as usize;

    if file_size > MAX_FILE_SIZE {
        return Err("File too large");
    }

    let mut bytes_read = 0;
    let mut temp_buffer = [0u8; 4096];

    // Read direct blocks
    for &block_num in &inode.block[..12] {
        if block_num == 0 || bytes_read >= file_size {
            break;
        }

        read_block(block_num, &mut temp_buffer)?;

        let bytes_to_copy = core::cmp::min(unsafe { BLOCK_SIZE }, file_size - bytes_read);

        buffer.data[bytes_read..bytes_read + bytes_to_copy]
            .copy_from_slice(&temp_buffer[..bytes_to_copy]);

        bytes_read += bytes_to_copy;
    }

    // TODO: Handle indirect blocks for larger files
    if bytes_read < file_size && inode.block[12] != 0 {
        return Err("Large files not supported yet");
    }

    buffer.size = bytes_read;
    Ok(())
}

pub fn read_file(path: &str) -> Result<FileBuffer, &'static str> {
    if !path.starts_with('/') {
        return Err("Path must be absolute");
    }

    let mut current_inode_num = 2; // Root directory is always inode 2

    // Split path and traverse directories
    let parts: Result<Vec<&str>, _> = path[1..] // Skip leading '/'
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|s| Ok(s))
        .collect();

    let parts = parts?;

    for (i, part) in parts.iter().enumerate() {
        let inode = get_inode(current_inode_num)?;

        // Check if it's a directory (except for the last component)
        if i < parts.len() - 1 && (inode.mode & EXT2_S_IFDIR) == 0 {
            return Err("Not a directory");
        }

        // Find the file/directory in current directory
        current_inode_num = find_file_in_directory(&inode, part)?;
    }

    // Read the final file
    let file_inode = get_inode(current_inode_num)?;

    // Ensure it's a regular file
    if (file_inode.mode & EXT2_S_IFREG) == 0 {
        return Err("Not a regular file");
    }

    let mut file_buffer = FileBuffer::new();
    read_inode_data(&file_inode, &mut file_buffer)?;

    Ok(file_buffer)
}
