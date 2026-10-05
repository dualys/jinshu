use core::option::Option;
use core::result::Result;
use core::result::Result::Ok;

/// The type of a branch in the Merkle tree of the filesystem.
///
/// A branch is a node that has children, and it can be either a left or right child of its parent node.
///
pub trait BlockDevice {
    /// ask the hardware to write 4096 bytes from RAM to the LBA.
    /// `data_phys_addr` must be the physical address of the `TrieNode` in RAM.
    /// # Parameters
    /// - `lba`: The Logical Block Address to write to.
    /// - `data_phys_addr`: The physical address of the data in RAM to write.
    ///
    /// ```no_run
    /// let lba = 0;
    /// let data_phys_addr = 0x1000;
    /// device.write_node(lba, data_phys_addr);
    /// ```
    fn write_node(&self, lba: u64, data_phys_addr: u64);

    /// Ask the hardware to read 4096 bytes from the LBA to RAM.
    ///
    /// `dest_phys_addr` must be the physical address of the `TrieNode` in RAM.
    /// # Parameters
    /// - `lba`: The Logical Block Address to read from.
    /// - `dest_phys_addr`: The physical address in RAM where the data should be written.
    ///
    /// ```no_run
    /// let lba = 0;
    /// let dest_phys_addr = 0x1000;
    /// device.read_node(lba, dest_phys_addr);
    /// ```
    fn read_node(&self, lba: u64, dest_phys_addr: u64);
}

/// The `DiskAllocator` is responsible for managing the allocation of Logical Block Addresses (LBAs) on the disk.
///
/// It ensures that each `TrieNode`, which occupies 4096 bytes (or 8 standard `Nvme` sectors of 512 bytes), is allocated in a sequential manner without overlapping or exceeding the disk's capacity.
///
/// # Fields
/// - `current_lba`: The next available Logical Block Address for allocation.
/// - `total_lbas`: The total number of Logical Block Addresses available on the disk.
/// # Example
/// ```no_run
/// let mut allocator = DiskAllocator::new(100); // 100 LBAs available
/// let lba1 = allocator.allocate_node().unwrap(); // Allocates LBAs 8 to 15
/// let lba2 = allocator.allocate_node().unwrap(); // Allocates LBAs 16 to 23
/// ```
pub struct DiskAllocator {
    current_lba: u64,
    total_lbas: u64,
}

impl DiskAllocator {
    /// Initializes the allocator with the raw capacity of the disk.
    /// # Parameters
    /// - `total_lbas`: The total number of LBAs available on the disk.
    /// # Returns
    /// A new instance of `DiskAllocator` ready to manage allocations.
    ///
    /// # Example
    /// ```no_run
    /// use jinshu::storage::DiskAllocator;
    /// let allocator = DiskAllocator::new(100); // 100 LBAs available
    /// ```
    #[must_use]
    pub const fn new(total_lbas: u64) -> Self {
        Self {
            current_lba: 8,
            total_lbas,
        }
    }

    /// Allocates space strictly for a `TrieNode`.
    /// A `TrieNode` is 4096 bytes, which is exactly 8 standard `Nvme` sectors of 512 bytes.
    ///
    /// # Example
    /// ```no_run
    /// use jinshu::storage::DiskAllocator;
    /// let mut allocator = DiskAllocator::new(100); // 100 LBAs available
    /// let lba1 = allocator.allocate_node().unwrap(); // Allocates LBAs 8 to 15
    /// let lba2 = allocator.allocate_node().unwrap(); // Allocates LBAs 16 to 23
    /// ```
    /// # Returns
    /// - `Some(u64)`: The starting Logical Block Address (LBA) allocated for the `TrieNode`.
    /// - `None`: If the disk is full and no more LBAs can be allocated.
    ///
    pub const fn allocate_node(&mut self) -> Option<u64> {
        let start_lba = self.current_lba;
        let next_lba = start_lba + 8;

        // Hardware overflow protection
        if next_lba > self.total_lbas {
            return Option::None; // Physically full disk!
        }
        self.current_lba = next_lba;
        Option::Some(start_lba)
    }
}

/// The `StorageEngine` is the core component of the `Jinshu` database engine responsible for managing storage operations on `Nvme` devices.
///
/// # Fields
/// - `allocator`: An instance of `DiskAllocator` that manages the allocation of Logical Block Addresses (LBAs) on the disk.
/// - `device`: A reference to a type that implements the `BlockDevice` trait, representing the underlying hardware device for storage operations.
pub struct StorageEngine<'a, T: BlockDevice> {
    pub allocator: DiskAllocator,
    pub device: &'a T,
}

impl<'a, T: BlockDevice> StorageEngine<'a, T> {
    /// Creates a new instance of the `StorageEngine`.
    /// It initializes the `DiskAllocator` with the total number of Logical Block Addresses (LBAs) available
    /// on the disk and associates it with the provided `BlockDevice`.
    /// # Parameters
    /// - `total_lbas`: The total number of Logical Block Addresses (LBAs) available on the disk.
    /// - `device`: A reference to the block device that the storage engine will manage.
    /// # Returns
    /// A new instance of `StorageEngine` ready to manage storage operations.
    /// # Example
    /// ```no_run
    /// use jinshu::storage::{StorageEngine, DiskAllocator, BlockDevice};
    /// struct MyBlockDevice;
    /// impl BlockDevice for MyBlockDevice {
    ///     fn write_node(&self, _lba: u64, _data_phys_addr: u64) {
    ///         // Implementation for writing to the device
    ///     }
    ///     fn read_node(&self, _lba: u64, _dest_phys_addr: u64) {
    ///         // Implementation for reading from the device
    ///     }
    /// }
    /// let device = MyBlockDevice;
    /// let storage_engine = StorageEngine::new(100, &device); // 100 LBAs available
    /// ```
    #[must_use]
    pub const fn new(total_lbas: u64, device: &'a T) -> Self {
        Self {
            allocator: DiskAllocator::new(total_lbas),
            device,
        }
    }
    /// Persists a `TrieNode` to the `Nvme` disk.
    /// It calculates the next available Logical Block Address (LBA) using the `DiskAllocator`,
    /// and then instructs the `BlockDevice` to write the `TrieNode` from RAM to that LBA.
    ///
    /// # Example
    /// ```no_run
    /// use jinshu::storage::{StorageEngine, DiskAllocator, BlockDevice};
    /// struct MyBlockDevice;
    /// impl BlockDevice for MyBlockDevice {
    ///     fn write_node(&self, lba: u64, data_phys_addr:  u64) {
    ///         // Implementation for writing to the device
    ///     }
    ///     fn read_node(&self, lba: u64, dest_phys_addr: u64) {
    ///         // Implementation for reading from the device
    ///     }
    /// }
    /// let device = MyBlockDevice;
    /// let mut storage_engine = StorageEngine::new(100, &device); // 100 LBAs available
    /// let data_phys_addr = 0x1000; // Physical address of the TrieNode in RAM
    /// let lba = storage_engine.persist_node(data_phys_addr).unwrap();
    /// ```
    /// # Parameters
    /// - `data_phys_addr`: The physical address of the `TrieNode` in RAM.
    /// # Returns
    /// - `Ok(u64)` containing the LBA where the `TrieNode` was written.
    /// - `Err(&'static str)` if the disk is full or if there was an error during the write operation.
    /// # Errors
    /// This function will return an error if the `DiskAllocator` cannot find a free LBA (i.e., the disk is full) or if the `BlockDevice` fails to write the data.
    ///
    pub fn persist_node(&mut self, data_phys_addr: u64) -> Result<u64, &'static str> {
        // 1. Jinshu  find the next available LBA for the TrieNode using the DiskAllocator
        let target_lba = self.allocator.allocate_node().ok_or("Err: Nvme full")?;
        // jinshu write the node to the disk at the target LBA
        self.device.write_node(target_lba, data_phys_addr);

        // (The `Nvme` will signal its completion later via the OS)
        Ok(target_lba)
    }

    /// Fetches a `TrieNode` from the `Nvme` disk into RAM.
    /// It instructs the `BlockDevice` to read the `TrieNode` from the specified Logical Block Address (LBA)
    /// into the provided physical address in RAM.
    /// # Example
    /// ```no_run
    /// use jinshu::storage::{StorageEngine, DiskAllocator, BlockDevice};
    /// struct MyBlockDevice;
    /// impl BlockDevice for MyBlockDevice {
    ///     fn write_node(&self, lba: u64, data_phys_addr: u64) {
    ///        // Implementation for writing to the device
    ///    }
    ///    fn read_node(&self, lba: u64, dest_phys_addr: u64) {
    ///       // Implementation for reading from the device
    ///   }
    /// }
    /// let device = MyBlockDevice;
    /// let mut storage_engine = StorageEngine::new(100, &device); // 100 LBAs available
    /// let lba = 0; // Logical Block Address of the TrieNode on disk
    /// let dest_phys_addr = 0x1000; // Physical address in RAM where the TrieNode should be written
    /// storage_engine.fetch_node(lba, dest_phys_addr).unwrap();
    /// ```
    /// # Parameters
    /// - `lba`: The Logical Block Address from which to read the `TrieNode`.
    /// - `dest_phys_addr`: The physical address in RAM where the `TrieNode` should be written.
    /// # Returns
    /// - `Ok(())` if the read operation was successful.
    /// - `Err(&'static str)` if there was an error during the read operation.
    /// # Errors
    /// This function will return an error if the `BlockDevice` fails to read the data from the specified LBA into RAM.
    pub fn fetch_node(&self, lba: u64, dest_phys_addr: u64) -> Result<(), &'static str> {
        self.device.read_node(lba, dest_phys_addr);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_disk_allocator_initialization() {
        let allocator = DiskAllocator::new(100);
        assert_eq!(allocator.current_lba, 8);
        assert_eq!(allocator.total_lbas, 100);
    }
    #[test]
    fn test_disk_allocator_append_only() {
        // A tiny disk of 20 sectors (10 KB) for testing
        let mut allocator = DiskAllocator::new(20);

        // Allocation du 1er nœud : prend les secteurs 8 à 15
        assert_eq!(allocator.allocate_node().unwrap(), 8);

        // Disque plein pour le 2ème nœud (il faudrait aller jusqu'à 24, on a que 20)
        assert!(allocator.allocate_node().is_none());
    }

    #[test]
    fn test_storage_engine_persist_and_fetch() {
        struct MockBlockDevice;
        impl BlockDevice for MockBlockDevice {
            fn write_node(&self, lba: u64, data_phys_addr: u64) {
                // Mock implementation: just print the operation
                println!(
                    "Writing to LBA {} from physical address {}",
                    lba, data_phys_addr
                );
            }

            fn read_node(&self, lba: u64, dest_phys_addr: u64) {
                // Mock implementation: just print the operation
                println!(
                    "Reading from LBA {} to physical address {}",
                    lba, dest_phys_addr
                );
            }
        }

        let mut storage_engine = StorageEngine {
            allocator: DiskAllocator::new(20),
            device: &MockBlockDevice,
        };

        let data_phys_addr = 0x1000;
        let lba = storage_engine.persist_node(data_phys_addr).unwrap();
        storage_engine.fetch_node(lba, data_phys_addr).unwrap();
        assert_eq!(lba, 8); // The first allocation should be at LBA 8
        assert_eq!(storage_engine.allocator.current_lba, 16); // After one allocation, the next LBA should be 16
        assert!(storage_engine.allocator.allocate_node().is_none()); // The disk should be full after one allocation
    }
}
