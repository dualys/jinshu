extern crate alloc;
use crate::node::TrieNode;
use crate::router::NounIndex;
use crate::storage::{BlockDevice, StorageEngine};
use alloc::collections::BTreeMap;
use noun::Noun;

/// Forge tool for the `CoreOcean` at system startup.
pub struct CoreOceanBuilder;

impl CoreOceanBuilder {
    /// Load recursively a Noun and its entire subtree of child nodes (multiple Nouns)
    /// from the `DiskOcean` (`NVMe`) to the `CoreOcean` (RAM).
    ///
    /// # Parameters
    /// - `root_noun`: The cryptographic identity of the root of the tree to load.
    /// - `storage` & `index`: Access to the `NVMe` hardware.
    /// - `ocean`: The OS shared RAM (`BTreeMap`) to populate.
    /// - `phys_addr` / `virt_addr`: A unique 4096-byte DMA buffer reused for extraction.
    ///
    /// # Returns
    /// - `Ok(())` if the entire subtree was successfully loaded into RAM.
    /// - `Err(&'static str)` if any error occurred during the loading process.
    ///
    /// # Errors
    /// - If the `root_noun` is not found in the `NounIndex`, an error is returned.
    /// - If the `StorageEngine` fails to fetch a node from the `NVMe`, an error is returned.
    ///
    /// # Safety
    /// - The function uses unsafe code to dereference a pointer to the DMA buffer.
    ///
    /// It is assumed that the buffer is valid and contains a properly formatted `TrieNode` after fetching from storage.
    ///
    /// # Example
    /// ```no_run
    /// use jinshu::ocean::CoreOceanBuilder;
    /// use jinshu::storage::StorageEngine;
    /// use noun::Noun;
    /// use jinshu::router::NounIndex;
    /// use alloc::collections::BTreeMap;
    /// let mut ocean = BTreeMap::new();
    /// let root_noun = Noun::from_bytes(&[0u8; 32]);
    /// let mut storage = StorageEngine::new(...);
    /// let index = NounIndex::new(...);
    /// let phys_addr = 0x1000_0000; // Example physical address
    /// let virt_addr = 0x2000_0000; // Example virtual address
    /// CoreOceanBuilder::deep_load(&root_noun, &mut storage, &index, &mut ocean, phys_addr, virt_addr).expect("Failed to load CoreOcean");    
    /// ```
    pub fn deep_load<T: BlockDevice>(
        root_noun: &Noun,
        storage: &mut StorageEngine<'_, T>,
        index: &NounIndex,
        ocean: &mut BTreeMap<Noun, TrieNode>,
        phys_addr: u64,
        virt_addr: u64,
    ) -> Result<(), &'static str> {
        if ocean.contains_key(root_noun) {
            return Ok(());
        }

        let lba = index
            .get_lba(root_noun)
            .ok_or("Init Error: Vital Noun not found on NVMe")?;
        storage.fetch_node(lba, phys_addr)?;

        // SAFETY: The pointer is valid because the node has just been loaded into physical memory via DMA.
        let node = unsafe { &*(virt_addr as *const TrieNode) }.clone();

        let lba = index
            .get_lba(root_noun)
            .ok_or("Init Error: Vital Noun not found on NVMe")?;
        storage.fetch_node(lba, phys_addr)?;

        // 3. Geometric Navigation: If it's a folder (mask > 0), it has child Nouns
        if node.mask > 0 {
            for i in 0..16 {
                if node.has_branch(i)
                    && let Some(child_noun) = node.get_branch(i)
                {
                    Self::deep_load(child_noun, storage, index, ocean, phys_addr, virt_addr)?;
                }
            }
        }

        // 4. Insert the loaded node into the in-memory ocean
        ocean.insert(root_noun.clone(), node);

        Ok(())
    }
}
