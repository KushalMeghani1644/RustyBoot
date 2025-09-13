use crate::drivers;

// ===== On-disk structures (ext2-compatible) =====

#[repr(C, packed)]
#[derive(Copy, Clone)]
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
    magic: u16, // 0xEF53
    state: u16,
    errors: u16,
    minor_rev_level: u16,
    lastcheck: u32,
    checkinterval: u32,
    creator_os: u32,
    rev_level: u32,
    def_resuid: u16,
    def_resgid: u16,

    // Extended fields (valid when rev_level >= 1)
    first_ino: u32,         // 0x54
    inode_size: u16,        // 0x58
    block_group_nr: u16,    // 0x5A
    feature_compat: u32,    // 0x5C
    feature_incompat: u32,  // 0x60
    feature_ro_compat: u32, // 0x64
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
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
#[derive(Copy, Clone)]
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
    block: [u32; 15], // 0..11 direct, 12 single-indirect, 13 double, 14 triple
    generation: u32,
    file_acl: u32,
    dir_acl: u32,
    faddr: u32,
    osd2: [u32; 3],
}

#[allow(dead_code)]
#[derive(Default, Copy, Clone)]
struct Ext2DirEntryView {
    inode: u32,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
}

// File types
const _EXT2_FT_UNKNOWN: u8 = 0;
const _EXT2_FT_REG_FILE: u8 = 1;
const _EXT2_FT_DIR: u8 = 2;

// Inode modes
const EXT2_S_IFREG: u16 = 0x8000; // Regular file
const EXT2_S_IFDIR: u16 = 0x4000; // Directory

// ===== Global filesystem state =====
static mut SUPERBLOCK: Option<Ext2Superblock> = None;
static mut BLOCK_SIZE: usize = 0;
static mut SECTORS_PER_BLOCK: usize = 0;
// Base LBA for the partition (added to all on-disk accesses)
static mut PARTITION_LBA_BASE: u32 = 0;

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

// ===== Public init API =====

/// Initialize EXT reader assuming the filesystem starts at absolute LBA 0.
pub fn init() -> Result<(), &'static str> {
    init_with_lba(0)
}

/// Initialize EXT reader using the given partition LBA base (MBR/GPT starting LBA).
pub fn init_with_lba(lba_base: u32) -> Result<(), &'static str> {
    unsafe {
        PARTITION_LBA_BASE = lba_base;
    }

    // Read superblock at byte offset 1024 from the start of the filesystem.
    // 512B sectors => LBA offset +2, read 2 sectors (1024 bytes).
    let mut buffer = [0u8; 1024];
    let lba = unsafe { PARTITION_LBA_BASE }.wrapping_add(2);
    drivers::disk::read_sectors(lba, 2, &mut buffer)?;

    let superblock: Ext2Superblock = unsafe {
        // Use unaligned read; on-disk data is not guaranteed aligned.
        core::ptr::read_unaligned(buffer.as_ptr() as *const Ext2Superblock)
    };

    // Check magic number (0xEF53 for ext2/3/4)
    if superblock.magic != 0xEF53 {
        return Err("Not an EXT filesystem");
    }

    // Basic feature gating: keep early-boot reader simple (no extents/64bit)
    // feature_incompat: 0x40 = EXTENTS, 0x80 = 64BIT
    let extents = (superblock.feature_incompat & 0x40) != 0;
    let has_64bit = (superblock.feature_incompat & 0x80) != 0;
    if extents || has_64bit {
        return Err("EXT filesystem uses unsupported features (extents/64bit)");
    }

    // Calculate block size
    let block_size = 1024usize
        .checked_shl(superblock.log_block_size)
        .ok_or("bad log_block_size")?;
    if block_size == 0 {
        return Err("invalid block size");
    }
    if block_size > 4096 {
        return Err("Unsupported EXT block size (>4096)");
    }
    if (block_size % 512) != 0 {
        return Err("Unsupported EXT block size (not multiple of 512)");
    }
    let sectors_per_block = block_size / 512;

    unsafe {
        SUPERBLOCK = Some(superblock);
        BLOCK_SIZE = block_size;
        SECTORS_PER_BLOCK = sectors_per_block;
    }

    drivers::vga::print_string("EXT filesystem initialized\n");
    Ok(())
}

// ===== Low-level block helpers =====

fn read_block(block_num: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
    let block_size = unsafe { BLOCK_SIZE };
    let sectors_per_block = unsafe { SECTORS_PER_BLOCK };
    let base = unsafe { PARTITION_LBA_BASE };

    if buffer.len() < block_size {
        return Err("Buffer too small for block");
    }
    if sectors_per_block == 0 {
        return Err("Filesystem not initialized (sectors_per_block=0)");
    }

    let start_sector = base.wrapping_add((block_num as usize * sectors_per_block) as u32);
    drivers::disk::read_sectors(
        start_sector,
        sectors_per_block as u16,
        &mut buffer[..block_size],
    )
}

fn descriptors_per_block() -> usize {
    unsafe { BLOCK_SIZE / core::mem::size_of::<Ext2BlockGroupDescriptor>() }
}

// ===== Metadata helpers =====

fn get_inode(inode_num: u32) -> Result<Ext2Inode, &'static str> {
    let superblock = unsafe { SUPERBLOCK.as_ref().ok_or("Filesystem not initialized")? };

    if inode_num == 0 {
        return Err("invalid inode 0");
    }

    // Identify group and local index
    let group = (inode_num - 1) / superblock.inodes_per_group;
    let local_inode = (inode_num - 1) % superblock.inodes_per_group;

    // Group Descriptor Table (GDT) starts at:
    //   gdt_start = first_data_block + 1
    // For 1K blocks: first_data_block==1 -> gdt at block 2
    // For >1K: first_data_block==0 -> gdt at block 1
    let gdt_start = superblock.first_data_block + 1;
    let d_per_blk = descriptors_per_block();
    if d_per_blk == 0 {
        return Err("invalid descriptors_per_block");
    }
    let gdt_block = gdt_start + (group as usize / d_per_blk) as u32;
    let index_in_block =
        (group as usize % d_per_blk) * core::mem::size_of::<Ext2BlockGroupDescriptor>();

    // Read the GDT block and load the descriptor for `group`
    let mut bgd_buffer = [0u8; 4096];
    read_block(gdt_block, &mut bgd_buffer)?;

    if index_in_block + core::mem::size_of::<Ext2BlockGroupDescriptor>() > unsafe { BLOCK_SIZE } {
        return Err("BGD index out of range");
    }

    let bgd: Ext2BlockGroupDescriptor = unsafe {
        core::ptr::read_unaligned(
            bgd_buffer.as_ptr().add(index_in_block) as *const Ext2BlockGroupDescriptor
        )
    };

    // Read inode from inode table
    let mut inode_size = 128usize;
    if superblock.rev_level >= 1 {
        let sz = superblock.inode_size as usize;
        // Accept sane sizes: >=128, <= block size, 4-byte aligned
        if sz >= 128 && sz <= unsafe { BLOCK_SIZE } && (sz & 3) == 0 {
            inode_size = sz;
        }
    }
    let inodes_per_block = unsafe { BLOCK_SIZE } / inode_size;
    if inodes_per_block == 0 {
        return Err("invalid inodes_per_block");
    }

    let inodes_per_block_u32 = inodes_per_block as u32;
    let inode_block = bgd.inode_table + (local_inode / inodes_per_block_u32);
    let inode_offset = ((local_inode % inodes_per_block_u32) as usize) * inode_size;

    let mut inode_buffer = [0u8; 4096];
    read_block(inode_block, &mut inode_buffer)?;

    if inode_offset + inode_size > unsafe { BLOCK_SIZE } {
        return Err("inode offset out of range");
    }

    let inode: Ext2Inode = unsafe {
        core::ptr::read_unaligned(inode_buffer.as_ptr().add(inode_offset) as *const Ext2Inode)
    };

    Ok(inode)
}

fn read_dir_entry(buf: &[u8], offset: usize) -> Result<Ext2DirEntryView, &'static str> {
    if offset + 8 > buf.len() {
        return Err("dir entry short");
    }
    let inode = u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]);
    let rec_len = u16::from_le_bytes([buf[offset + 4], buf[offset + 5]]);
    let name_len = buf[offset + 6];
    let file_type = buf[offset + 7];
    Ok(Ext2DirEntryView {
        inode,
        rec_len,
        name_len,
        file_type,
    })
}

// ===== Directory and file access =====

fn find_file_in_directory(dir_inode: &Ext2Inode, filename: &str) -> Result<u32, &'static str> {
    let block_size = unsafe { BLOCK_SIZE };
    let mut block_buf = [0u8; 4096];

    // Scan direct blocks (0..=11)
    for &block_num in &dir_inode.block[..12] {
        if block_num == 0 {
            continue;
        }

        read_block(block_num, &mut block_buf)?;
        let mut offset = 0usize;

        while offset + 8 <= block_size {
            let entry = read_dir_entry(&block_buf[..block_size], offset)?;

            if entry.inode == 0 || entry.rec_len == 0 {
                break;
            }
            let rec_len = entry.rec_len as usize;
            if rec_len < 8 || offset + rec_len > block_size {
                // Corrupted dir entry; stop scanning this block
                break;
            }

            // Safe bounds for name
            let name_end = 8 + (entry.name_len as usize);
            if name_end <= rec_len && offset + name_end <= block_size {
                let name_slice = &block_buf[offset + 8..offset + name_end];

                // Compare with requested filename
                if let Ok(name_str) = core::str::from_utf8(name_slice) {
                    if name_str == filename {
                        return Ok(entry.inode);
                    }
                }
            }

            offset += rec_len;
        }
    }

    // Scan single-indirect directory data if present
    if dir_inode.block[12] != 0 {
        let mut ind_block = [0u8; 4096];
        read_block(dir_inode.block[12], &mut ind_block)?;

        let ptrs_per_block = block_size / 4;
        let mut pi = 0usize;

        while pi < ptrs_per_block {
            let p = pi * 4;
            let ptr = u32::from_le_bytes([
                ind_block[p],
                ind_block[p + 1],
                ind_block[p + 2],
                ind_block[p + 3],
            ]);
            if ptr == 0 {
                break;
            }

            read_block(ptr, &mut block_buf)?;
            let mut offset = 0usize;

            while offset + 8 <= block_size {
                let entry = read_dir_entry(&block_buf[..block_size], offset)?;

                if entry.inode == 0 || entry.rec_len == 0 {
                    break;
                }
                let rec_len = entry.rec_len as usize;
                if rec_len < 8 || offset + rec_len > block_size {
                    // Corrupted dir entry; stop scanning this block
                    break;
                }

                // Safe bounds for name
                let name_end = 8 + (entry.name_len as usize);
                if name_end <= rec_len && offset + name_end <= block_size {
                    let name_slice = &block_buf[offset + 8..offset + name_end];

                    // Compare with requested filename
                    if let Ok(name_str) = core::str::from_utf8(name_slice) {
                        if name_str == filename {
                            return Ok(entry.inode);
                        }
                    }
                }

                offset += rec_len;
            }

            pi += 1;
        }
    }

    Err("File not found")
}

fn read_inode_data(inode: &Ext2Inode, buffer: &mut FileBuffer) -> Result<(), &'static str> {
    let block_size = unsafe { BLOCK_SIZE };
    let file_size = inode.size as usize;

    if file_size > MAX_FILE_SIZE {
        return Err("File too large");
    }

    let mut bytes_read = 0usize;
    let mut data_block = [0u8; 4096];

    // Read direct blocks (0..=11)
    for &block_num in &inode.block[..12] {
        if block_num == 0 || bytes_read >= file_size {
            break;
        }

        read_block(block_num, &mut data_block)?;

        let to_copy = core::cmp::min(block_size, file_size - bytes_read);
        buffer.data[bytes_read..bytes_read + to_copy].copy_from_slice(&data_block[..to_copy]);
        bytes_read += to_copy;
    }

    if bytes_read >= file_size {
        buffer.size = bytes_read;
        return Ok(());
    }

    // Single-indirect (block[12])
    if bytes_read < file_size && inode.block[12] != 0 {
        let mut ind_block = [0u8; 4096];
        read_block(inode.block[12], &mut ind_block)?;

        let ptrs_per_block = block_size / 4;
        let mut pi = 0usize;

        while pi < ptrs_per_block && bytes_read < file_size {
            let p = pi * 4;
            let ptr = u32::from_le_bytes([
                ind_block[p],
                ind_block[p + 1],
                ind_block[p + 2],
                ind_block[p + 3],
            ]);
            if ptr == 0 {
                break;
            }

            read_block(ptr, &mut data_block)?;

            let to_copy = core::cmp::min(block_size, file_size - bytes_read);
            buffer.data[bytes_read..bytes_read + to_copy].copy_from_slice(&data_block[..to_copy]);
            bytes_read += to_copy;

            pi += 1;
        }
    }

    // Double-indirect (block[13])
    if bytes_read < file_size && inode.block[13] != 0 {
        let mut ind2_block = [0u8; 4096];
        read_block(inode.block[13], &mut ind2_block)?;

        let ptrs_per_block = block_size / 4;
        let mut i = 0usize;

        while i < ptrs_per_block && bytes_read < file_size {
            let p1 = i * 4;
            let ptr1 = u32::from_le_bytes([
                ind2_block[p1],
                ind2_block[p1 + 1],
                ind2_block[p1 + 2],
                ind2_block[p1 + 3],
            ]);
            if ptr1 == 0 {
                break;
            }

            // Read single-indirect block pointed by ptr1
            let mut ind_block = [0u8; 4096];
            read_block(ptr1, &mut ind_block)?;

            let mut j = 0usize;
            while j < ptrs_per_block && bytes_read < file_size {
                let p2 = j * 4;
                let ptr2 = u32::from_le_bytes([
                    ind_block[p2],
                    ind_block[p2 + 1],
                    ind_block[p2 + 2],
                    ind_block[p2 + 3],
                ]);
                if ptr2 == 0 {
                    break;
                }

                read_block(ptr2, &mut data_block)?;

                let to_copy = core::cmp::min(block_size, file_size - bytes_read);
                buffer.data[bytes_read..bytes_read + to_copy]
                    .copy_from_slice(&data_block[..to_copy]);
                bytes_read += to_copy;

                j += 1;
            }

            i += 1;
        }
    }

    if bytes_read < file_size {
        // Triple-indirect not implemented
        return Err("Large files not fully supported (needs triple indirect)");
    }

    buffer.size = bytes_read;
    Ok(())
}

/// Read a file by absolute POSIX-like path (e.g., "/boot/vmlinuz") from the EXT filesystem.
pub fn read_file(path: &str) -> Result<FileBuffer, &'static str> {
    if !path.starts_with('/') {
        return Err("Path must be absolute");
    }

    let mut current_inode_num = 2; // Root directory is always inode 2

    // Split path and traverse directories
    let mut start = 1; // skip leading '/'
    let bytes = path.as_bytes();
    let mut i = 1;
    let len = bytes.len();

    while i <= len {
        if i == len || bytes[i] == b'/' {
            if i > start {
                // component = &path[start..i]
                let component = match core::str::from_utf8(&bytes[start..i]) {
                    Ok(s) => s,
                    Err(_) => return Err("Invalid path encoding"),
                };

                // Get current inode and ensure directory except for last check is skipped;
                let inode = get_inode(current_inode_num)?;

                // If there are more components after this one, require directory
                if i < len && (inode.mode & EXT2_S_IFDIR) == 0 {
                    return Err("Not a directory");
                }

                current_inode_num = find_file_in_directory(&inode, component)?;
            }
            start = i + 1;
        }
        i += 1;
    }

    // Read final inode
    let file_inode = get_inode(current_inode_num)?;

    // Ensure it's a regular file
    if (file_inode.mode & EXT2_S_IFREG) == 0 {
        return Err("Not a regular file");
    }

    let mut file_buffer = FileBuffer::new();
    read_inode_data(&file_inode, &mut file_buffer)?;

    Ok(file_buffer)
}
