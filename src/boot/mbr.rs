//! Master Boot Record (MBR) helper for RustyBoot
//!
//! Responsibilities:
//! - Read LBA 0 (sector 0) from the boot disk
//! - Verify 0x55AA signature
//! - Parse four partition entries
//! - Provide helpers to query the active partition and print info for debugging
//!
//! This module is *not* the 512‑byte real-mode MBR. It runs as part of the
//! Rust bootloader after basic drivers are up.

#![allow(dead_code)]

use core::mem::size_of;

use crate::drivers::{disk, vga};

pub const MBR_BYTES: usize = 512;
pub const MBR_SIGNATURE: u16 = 0xAA55; // note: little-endian on disk is 55 AA
pub const PARTITION_TABLE_OFFSET: usize = 446; // 0x1BE
pub const PARTITION_ENTRY_COUNT: usize = 4;

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Default)]
pub struct RawPartitionEntry {
    pub boot_indicator: u8,    // 0x80 = bootable
    pub starting_chs: [u8; 3], // CHS (legacy; usually ignored)
    pub partition_type: u8,    // type code (0x83 ext4, 0x0B/0x0C FAT32, etc.)
    pub ending_chs: [u8; 3],   // CHS (legacy)
    pub starting_lba: u32,     // first LBA
    pub sectors: u32,          // number of sectors
}

#[derive(Copy, Clone, Debug)]
pub struct PartitionEntry {
    pub bootable: bool,
    pub partition_type: u8,
    pub starting_lba: u32,
    pub sectors: u32,
}

#[derive(Clone, Debug)]
pub struct MbrInfo {
    pub signature_valid: bool,
    pub partitions: [Option<PartitionEntry>; PARTITION_ENTRY_COUNT],
}

/// Read LBA0 into a fixed 512‑byte buffer.
pub fn read_mbr_sector(buf: &mut [u8; MBR_BYTES]) -> Result<(), &'static str> {
    disk::read_sectors(0, 1, buf).map_err(|_| "disk read LBA0 failed")
}

/// Validate the 0x55AA signature at the end of the MBR.
fn has_valid_signature(mbr: &[u8]) -> bool {
    if mbr.len() < MBR_BYTES {
        return false;
    }
    let lo = mbr[MBR_BYTES - 2] as u16;
    let hi = mbr[MBR_BYTES - 1] as u16;
    (hi << 8) | lo == MBR_SIGNATURE
}

/// Parse the four partition entries from the 512‑byte buffer.
pub fn parse_partitions(mbr: &[u8]) -> [Option<PartitionEntry>; PARTITION_ENTRY_COUNT] {
    let mut out: [Option<PartitionEntry>; PARTITION_ENTRY_COUNT] = [None, None, None, None];

    let base = PARTITION_TABLE_OFFSET;
    let step = size_of::<RawPartitionEntry>();

    let to_u32 = |b: &[u8]| -> u32 { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) };

    for i in 0..PARTITION_ENTRY_COUNT {
        let o = base + i * step;
        if o + step > mbr.len() {
            break;
        }
        let entry = &mbr[o..o + step];

        let boot_indicator = entry[0];
        let partition_type = entry[4];
        let starting_lba = to_u32(&entry[8..12]);
        let sectors = to_u32(&entry[12..16]);

        if partition_type != 0 && sectors != 0 {
            out[i] = Some(PartitionEntry {
                bootable: boot_indicator == 0x80,
                partition_type,
                starting_lba,
                sectors,
            });
        }
    }

    out
}

/// Read, verify, and parse the MBR into a high‑level `MbrInfo`.
pub fn probe() -> Result<MbrInfo, &'static str> {
    let mut buf = [0u8; MBR_BYTES];
    read_mbr_sector(&mut buf)?;

    let signature_valid = has_valid_signature(&buf);
    if !signature_valid {
        vga::print_string("MBR signature invalid (expected 0x55AA)\n");
    }

    let partitions = parse_partitions(&buf);
    Ok(MbrInfo {
        signature_valid,
        partitions,
    })
}

/// Return the first `bootable` (active) partition, if any, with its index.
pub fn find_active_partition(info: &MbrInfo) -> Option<(usize, PartitionEntry)> {
    for (idx, p) in info.partitions.iter().enumerate() {
        if let Some(pe) = p {
            if pe.bootable {
                return Some((idx, *pe));
            }
        }
    }
    None
}

/// Convenience: return the first non‑empty partition if no active flag is set.
pub fn first_present_partition(info: &MbrInfo) -> Option<(usize, PartitionEntry)> {
    for (idx, p) in info.partitions.iter().enumerate() {
        if let Some(pe) = p {
            return Some((idx, *pe));
        }
    }
    None
}

/// Pretty‑print parsed MBR information to VGA for debugging during bring‑up.
pub fn debug_print(info: &MbrInfo) {
    vga::print_string("— MBR —\n");
    vga::print_string("signature: ");
    if info.signature_valid {
        vga::print_string("OK\n");
    } else {
        vga::print_string("BAD\n");
    }

    for i in 0..PARTITION_ENTRY_COUNT {
        match info.partitions[i] {
            None => {
                vga::print_string("[ ");
                print_dec(i as u32);
                vga::print_string(" ] <empty>\n");
            }
            Some(p) => {
                vga::print_string("[ ");
                print_dec(i as u32);
                vga::print_string("] boot=");
                vga::print_string(if p.bootable { "Y" } else { "N" });
                vga::print_string(" type=0x");
                print_hex8(p.partition_type);
                vga::print_string(" start=");
                print_dec(p.starting_lba);
                vga::print_string(" sectors=");
                print_dec(p.sectors);
                vga::print_string("\n");
            }
        }
    }
}

// ===== Small local print helpers (avoid depending on other private modules) =====
fn print_hex8(mut v: u8) {
    for shift in [4u8, 0u8] {
        let nibble = ((v >> shift) & 0xF) as u8;
        let ch = if nibble < 10 {
            b'0' + nibble
        } else {
            b'A' + (nibble - 10)
        };
        vga::print_char(ch);
    }
}

fn print_dec(mut n: u32) {
    if n == 0 {
        vga::print_char(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while n > 0 && i < buf.len() {
        buf[i] = (n % 10) as u8 + b'0';
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        vga::print_char(buf[i]);
    }
}
