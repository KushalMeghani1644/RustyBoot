use crate::memory::mem;
use core::cell::UnsafeCell;
use core::ptr::null_mut;
use spin::Mutex;

// Memory constants for bootloader environment
const MEMORY_START: usize = 0x100000; // 1MB - above conventional memory
const MEMORY_END: usize = 0x800000; // 8MB - safe upper limit for bootloader
const PAGE_SIZE: usize = 4096;
const MAX_REGIONS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryRegionType {
    Available,
    Reserved,
    AcpiReclaim,
    AcpiNvs,
    BadMemory,
    Bootloader,
    Kernel,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: usize,
    pub size: usize,
    pub region_type: MemoryRegionType,
}

pub struct MemoryManager {
    regions: [Option<MemoryRegion>; MAX_REGIONS],
    region_count: usize,
    heap_start: usize,
    heap_current: usize,
    heap_end: usize,
    allocated_bytes: usize,
}

impl MemoryManager {
    pub fn new() -> Self {
        let mut manager = MemoryManager {
            regions: [None; MAX_REGIONS],
            region_count: 0,
            heap_start: MEMORY_START,
            heap_current: MEMORY_START,
            heap_end: MEMORY_END,
            allocated_bytes: 0,
        };

        // Initialize with basic memory layout
        manager.detect_memory();
        manager
    }

    /// Detect available memory regions (simplified for bootloader)
    fn detect_memory(&mut self) {
        // For a bootloader, we'll use a simple static memory map
        // In a real implementation, you'd use BIOS INT 15h, E820h

        // Add conventional memory region (simplified)
        self.add_region(MemoryRegion {
            start: MEMORY_START,
            size: MEMORY_END - MEMORY_START,
            region_type: MemoryRegionType::Available,
        });

        // Reserve space for bootloader itself (first 1MB + some extra)
        self.add_region(MemoryRegion {
            start: 0x7C00, // MBR location
            size: 0x98400, // Up to ~600KB
            region_type: MemoryRegionType::Bootloader,
        });
    }

    fn add_region(&mut self, region: MemoryRegion) {
        if self.region_count < MAX_REGIONS {
            self.regions[self.region_count] = Some(region);
            self.region_count += 1;
        }
    }

    /// Simple bump allocator for bootloader use
    pub fn allocate(&mut self, size: usize) -> Option<*mut u8> {
        if size == 0 {
            return None;
        }

        // Align to 8-byte boundary
        let aligned_size = (size + 7) & !7;

        // Check if we have enough space
        if self.heap_current + aligned_size > self.heap_end {
            return None;
        }

        let ptr = self.heap_current as *mut u8;
        self.heap_current += aligned_size;
        self.allocated_bytes += aligned_size;

        // Zero the allocated memory
        unsafe {
            mem::memset(ptr, 0, aligned_size);
        }

        Some(ptr)
    }

    /// Allocate aligned memory (useful for page-aligned allocations)
    pub fn allocate_aligned(&mut self, size: usize, alignment: usize) -> Option<*mut u8> {
        if size == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return None;
        }

        // Calculate aligned start address
        let aligned_current = (self.heap_current + alignment - 1) & !(alignment - 1);

        // Check if we have enough space
        if aligned_current + size > self.heap_end {
            return None;
        }

        // Update heap pointer to aligned position
        self.heap_current = aligned_current + size;
        self.allocated_bytes += size;

        let ptr = aligned_current as *mut u8;

        // Zero the allocated memory
        unsafe {
            mem::memset(ptr, 0, size);
        }

        Some(ptr)
    }

    /// Allocate page-aligned memory
    pub fn allocate_pages(&mut self, page_count: usize) -> Option<*mut u8> {
        let size = page_count * PAGE_SIZE;
        self.allocate_aligned(size, PAGE_SIZE)
    }

    /// Get memory statistics
    pub fn get_stats(&self) -> MemoryStats {
        MemoryStats {
            total_memory: self.heap_end - self.heap_start,
            used_memory: self.allocated_bytes,
            free_memory: self.heap_end - self.heap_current,
            heap_start: self.heap_start,
            heap_current: self.heap_current,
            heap_end: self.heap_end,
        }
    }

    /// Reserve memory region (useful for kernel loading)
    pub fn reserve_region(&mut self, start: usize, size: usize) -> Result<(), &'static str> {
        // Check if the region conflicts with our heap
        if start < self.heap_current && start + size > self.heap_start {
            return Err("Cannot reserve region that conflicts with allocated memory");
        }

        // If the region is at the end of our heap, reduce available space
        if start >= self.heap_current && start < self.heap_end {
            self.heap_end = start;
        }

        self.add_region(MemoryRegion {
            start,
            size,
            region_type: MemoryRegionType::Reserved,
        });

        Ok(())
    }

    /// Find a suitable location for kernel loading
    pub fn find_kernel_location(&self, kernel_size: usize) -> Option<usize> {
        // Typical kernel load address
        const KERNEL_LOAD_ADDR: usize = 0x200000; // 2MB

        // Check if we have enough space at the typical location
        if KERNEL_LOAD_ADDR + kernel_size < self.heap_end {
            return Some(KERNEL_LOAD_ADDR);
        }

        // Find alternative location
        let aligned_size = (kernel_size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        for region in &self.regions[..self.region_count] {
            if let Some(region) = region {
                if region.region_type == MemoryRegionType::Available && region.size >= aligned_size
                {
                    return Some(region.start);
                }
            }
        }

        None
    }

    /// Mark kernel region as used
    pub fn mark_kernel_loaded(&mut self, start: usize, size: usize) {
        self.add_region(MemoryRegion {
            start,
            size,
            region_type: MemoryRegionType::Kernel,
        });
    }

    /// Get memory regions (for debugging)
    pub fn get_regions(&self) -> &[Option<MemoryRegion>] {
        &self.regions[..self.region_count]
    }

    /// Simple deallocation (bootloader typically doesn't need this)
    #[allow(unused_variables)]
    pub fn deallocate(&mut self, ptr: *mut u8, size: usize) {
        // In a bootloader, we typically don't deallocate memory
        // This is a placeholder for future implementation
    }

    /// Reset allocator to initial state (useful for cleanup)
    pub fn reset(&mut self) {
        self.heap_current = self.heap_start;
        self.allocated_bytes = 0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
    pub total_memory: usize,
    pub used_memory: usize,
    pub free_memory: usize,
    pub heap_start: usize,
    pub heap_current: usize,
    pub heap_end: usize,
}

// Global memory manager instance (for bootloader use) — changed from static mut
static MEMORY_MANAGER: Mutex<UnsafeCell<Option<MemoryManager>>> = Mutex::new(UnsafeCell::new(None));

pub fn init_global_manager() {
    let mut guard = MEMORY_MANAGER.lock();
    unsafe {
        *guard.get() = Some(MemoryManager::new());
    }
}

pub fn get_global_manager() -> Option<&'static mut MemoryManager> {
    let mut guard = MEMORY_MANAGER.lock();
    unsafe { (*guard.get()).as_mut() }
}

pub fn global_allocate(size: usize) -> Option<*mut u8> {
    let mut guard = MEMORY_MANAGER.lock();
    unsafe { (*guard.get()).as_mut()?.allocate(size) }
}

pub fn global_allocate_pages(count: usize) -> Option<*mut u8> {
    let mut guard = MEMORY_MANAGER.lock();
    unsafe { (*guard.get()).as_mut()?.allocate_pages(count) }
}

// Helper functions for common operations
impl MemoryManager {
    /// Allocate a buffer for file loading
    pub fn allocate_file_buffer(&mut self, size: usize) -> Option<*mut u8> {
        // For file operations, we want 4KB alignment for better performance
        self.allocate_aligned(size, 4096)
    }

    /// Check if an address range is valid
    pub fn is_valid_range(&self, start: usize, size: usize) -> bool {
        start >= self.heap_start && start + size <= self.heap_end
    }

    /// Get available memory in bytes
    pub fn available_memory(&self) -> usize {
        self.heap_end - self.heap_current
    }
}
