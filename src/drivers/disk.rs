//! ATA PIO disk driver (minimal) for RustyBoot
//!
//! Implements `init()` and `read_sectors()` using 28‑bit LBA on the
//! primary channel, master drive. Sufficient for QEMU/Bochs and many
//! bare‑metal tests.
//!
//! Safety: uses raw port I/O and inline asm; x86 only.

#![allow(dead_code)]

use core::cmp::min;

use crate::drivers::vga;

// ===== ATA I/O port layout (Primary channel) =====
const ATA_PRIMARY_IO: u16 = 0xF10;
const ATA_PRIMARY_CTRL: u16 = 0x3F6; // Device control / alt status

const ATA_REG_DATA: u16 = ATA_PRIMARY_IO + 0; // R/W: data (16‑bit)
const ATA_REG_ERROR: u16 = ATA_PRIMARY_IO + 1; // R: error
const ATA_REG_FEATURES: u16 = ATA_PRIMARY_IO + 1; // W: features
const ATA_REG_SECCOUNT0: u16 = ATA_PRIMARY_IO + 2; // sector count (low)
const ATA_REG_LBA0: u16 = ATA_PRIMARY_IO + 3; // LBA[7:0]
const ATA_REG_LBA1: u16 = ATA_PRIMARY_IO + 4; // LBA[15:8]
const ATA_REG_LBA2: u16 = ATA_PRIMARY_IO + 5; // LBA[23:16]
const ATA_REG_HDDEVSEL: u16 = ATA_PRIMARY_IO + 6; // drive/head + LBA bits
const ATA_REG_COMMAND: u16 = ATA_PRIMARY_IO + 7; // write: command
const ATA_REG_STATUS: u16 = ATA_PRIMARY_IO + 7; // read: status

//control side
const ATA_REG_DEVCTRL: u16 = ATA_PRIMARY_CTRL; // write: nIEN, SRST
const ATA_REG_ALTSTATUS: u16 = ATA_PRIMARY_CTRL; // read: alt status

// ===== Status bits =====
const ATA_SR_ERR: u8 = 0x01; // Error
const ATA_SR_DRQ: u8 = 0x08; // Data Request ready
const ATA_SR_DF: u8 = 0x20; // Device Fault
const ATA_SR_DRDY: u8 = 0x40; // Device Ready
const ATA_SR_BSY: u8 = 0x80; // Busy

// ===== Commands =====
const ATA_CMD_IDENTIFY: u8 = 0xEC;
const ATA_CMD_READ_SECTORS: u8 = 0x20; //  LBA28 PIO

// ===== Low‑level port I/O (x86 only) =====
#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") val, options(nomem, nostack, preserves_flags));
    val
}

#[inline(always)]
unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    core::arch::asm!("in ax, dx", in("dx") port, out("ax") val, options(nomem, nostack, preserves_flags));
    val
}

#[inline(always)]
unsafe fn io_wait() {
    outb(0x80, 0);
}

// ===== Poll helpers =====
unsafe fn poll_status(mask_set: u8, mask_clear: u8) -> Result<u8, &'static str> {
    // Read status until the required bits are set and others cleared, or error.
    loop {
        let s = inb(ATA_REG_STATUS);
        if (s & ATA_SR_ERR) != 0 {
            return Err("ATA: status Err");
        }
        if (s & ATA_SR_DF) != 0 {
            return Err("ATA: device fault");
        }
        if (s & mask_set) == mask_set && (s & mask_clear) == 0 {
            return Ok(s);
        }
    }
}

unsafe fn wait_bsy_clear() -> Result<(), &'static str> {
    // First a few dummy reads per ATA spec
    for _ in 0..4 {
        let _ = inb(ATA_REG_ALTSTATUS);
        io_wait();
    }

    loop {
        let s = inb(ATA_REG_STATUS);
        if (s & ATA_SR_BSY) == 0 {
            return Ok(());
        }
        if (s & ATA_SR_ERR) != 0 {
            return Err("ATA: wait BSY ERR");
        }
        if (s & ATA_SR_DF) != 0 {
            return Err("ATA: wait BSY DF");
        }
    }
}

unsafe fn wait_drq_set() -> Result<(), &'static str> {
    poll_status(ATA_SR_DRQ, ATA_SR_BSY).map(|_| ())
}

// ===== Public API =====

/// Probe primary master with IDENTIFY. Not strictly required for PIO reads,
/// but useful to confirm presence and wake the device up.
pub fn init() -> Result<(), &'static str> {
    unsafe {
        // Disable IRQs from controller (nIEN=1), clear SRST
        outb(ATA_REG_DEVCTRL, 0x02);
        io_wait();

        // Select master, LBA mode upper nibble zero
        outb(ATA_REG_HDDEVSEL, 0xE0);
        io_wait();

        // Zero sector count and LBA regs per IDENTIFY requirements
        outb(ATA_REG_SECCOUNT0, 0);
        outb(ATA_REG_LBA0, 0);
        outb(ATA_REG_LBA1, 0);
        outb(ATA_REG_LBA2, 0);

        // Send IDENTIFY
        outb(ATA_REG_COMMAND, ATA_CMD_IDENTIFY);
        io_wait();

        // If status is 0, no device
        let mut status = inb(ATA_REG_STATUS);
        if status == 0 {
            return Err("ATA: no device on primary master");
        }

        // Busy wait
        wait_bsy_clear()?;

        // Some ATAPI devices set LBA1/LBA2 nonzero; treat as not ATA
        let lba1 = inb(ATA_REG_LBA1);
        let lba2 = inb(ATA_REG_LBA2);
        if lba1 != 0 || lba2 != 0 {
            return Err("ATA: not an ATA disk (ATAPI?)");
        }

        // Wait for DRQ then read 256 words of IDENTIFY data and drop them
        wait_drq_set()?;
        for _ in 0..256 {
            let _ = inw(ATA_REG_DATA);
        }

        vga::print_string("[disk] ATA primary master identified\n");
        Ok(())
    }
}

/// Read `count` sectors (512 bytes each) starting at `lba` into `buffer`.
/// Supports up to 255 sectors per command; larger reads are chunked.
pub fn read_sectors(mut lba: u32, mut count: u16, buffer: &mut [u8]) -> Result<(), &'static str> {
    if count == 0 {
        return Ok(());
    }
    let total = (count as usize) * 512;
    if buffer.len() < total {
        return Err("buffer too small for read_sectors");
    }

    let mut off = 0usize;

    unsafe {
        while count > 0 {
            let chunk: u8 = min(count, 255) as u8; // protocol limit for SECCOUNT0

            // Select drive: master (0xE0) | high 4 bits of LBA
            outb(ATA_REG_HDDEVSEL, 0xE0 | ((lba >> 24) as u8 & 0x0F));
            io_wait();

            // Program sector count and LBA registers
            outb(ATA_REG_SECCOUNT0, chunk);
            outb(ATA_REG_LBA0, (lba & 0xFF) as u8);
            outb(ATA_REG_LBA1, ((lba >> 8) & 0xFF) as u8);
            outb(ATA_REG_LBA2, ((lba >> 16) & 0xFF) as u8);

            // Issue READ SECTORS
            outb(ATA_REG_COMMAND, ATA_CMD_READ_SECTORS);

            // Read `chunk` sectors
            for _ in 0..chunk {
                wait_bsy_clear()?;
                wait_drq_set()?;

                // 256 words per sector
                for _ in 0..256 {
                    let w = inw(ATA_REG_DATA);
                    buffer[off] = (w & 0xFF) as u8;
                    buffer[off + 1] = (w >> 8) as u8;
                    off += 2;
                }

                // optional tiny delay
                io_wait();
            }

            lba = lba.wrapping_add(chunk as u32);
            count -= chunk as u16;
        }
    }

    Ok(())
}
