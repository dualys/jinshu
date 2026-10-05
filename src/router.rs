extern crate alloc;
use crate::node::TrieNode;
use crate::storage::{BlockDevice, StorageEngine};
use alloc::collections::BTreeMap;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::default::Default;
use core::option::Option;
use core::result::Result;
use heapless::Vec;
use noun::Noun;

/// The maximum number of entries that the `NounIndex` can hold. This is a fixed-size limit to ensure efficient memory usage and performance.
pub const MAX_ENTRIES: usize = 1024;

/// An index that maps Nouns to their corresponding physical addresses on the `NVMe` disk. This allows for efficient retrieval of data based on its cryptographic identity.
///
/// # Fields
/// - `entries`: A fixed-size vector that holds tuples of `(Noun, u64)`, where `Noun` is the cryptographic identity and `u64` is the physical address (LBA) on the disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NounIndex {
    entries: Vec<(Noun, u64), MAX_ENTRIES>,
}

impl Default for NounIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl NounIndex {
    /// Creates a new [`NounIndex`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Inserts a new entry into the index.
    ///
    /// # Errors
    ///
    /// This function will return an error if the index is full.
    ///
    /// # Parameters
    /// - `noun`: The cryptographic identity of the data.
    /// - `lba`: The physical address of the data on the `NVMe` disk.
    /// # Returns
    /// - `Ok(())` if the entry was successfully inserted.
    /// - `Err(&'static str)` if the index is full.
    ///
    /// # Example
    /// ```no_run
    /// use jinshu::router::NounIndex;
    /// use jinshu::noun::Noun;
    /// let mut index = NounIndex::new();
    /// let noun = Noun::of(&[0u8; 32]);    
    /// let lba = 42;
    /// match index.insert(noun.clone(), lba) {
    ///     Ok(()) => println!("Successfully inserted entry"),
    ///     Err(e) => eprintln!("Error inserting entry: {e}"),
    /// }
    /// ```
    pub fn insert(&mut self, noun: Noun, lba: u64) -> Result<(), &'static str> {
        self.entries
            .push((noun, lba))
            .map_err(|_| "ram full: NounIndex cannot hold more entries")
    }

    /// Returns the number of entries currently in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Returns `true` if the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Retrieves the physical address (LBA) associated with a given Noun.
    ///
    /// # Parameters
    /// - `noun`: The cryptographic identity of the data to look up.
    /// # Returns
    /// - `Some(u64)` containing the physical address if found.
    /// - `None` if the Noun is not present in the index.
    /// # Errors
    /// This function does not return an error, but it will return `None` if the Noun is not found in the index.
    /// # Example
    /// ```no_run
    /// use jinshu::router::NounIndex;
    /// use jinshu::noun::Noun;
    /// let mut index = NounIndex::new();
    /// let noun = Noun::of(&[0u8; 32]);
    /// let lba = 42;
    /// index.insert(noun.clone(), lba).unwrap();
    /// assert_eq!(index.get_lba(&noun), Some(lba));
    /// ```
    #[must_use]
    pub fn get_lba(&self, noun: &Noun) -> Option<u64> {
        for (n, lba) in &self.entries {
            if n == noun {
                return Option::Some(*lba);
            }
        }
        Option::None
    }
}

pub struct SemanticRouter<'a, T: BlockDevice> {
    pub disk_storage: StorageEngine<'a, T>,
    pub disk_index: NounIndex,
    pub core_ocean: &'a BTreeMap<Noun, TrieNode>,
    pub virtual_ocean: BTreeMap<Noun, TrieNode>,
}

impl<'a, T: BlockDevice> SemanticRouter<'a, T> {
    #[must_use]
    pub const fn new(
        disk_storage: StorageEngine<'a, T>,
        core_ocean: &'a BTreeMap<Noun, TrieNode>,
    ) -> Self {
        Self {
            disk_storage,
            disk_index: NounIndex::new(), // À charger depuis le NVMe au boot
            core_ocean,
            virtual_ocean: BTreeMap::new(), // Toujours vide au démarrage du Prisme
        }
    }
    // ... [Mets ça à l'intérieur de ton bloc impl SemanticRouter] ...

    pub fn fetch_node_cascade(
        &self,
        noun: &Noun,
        buffer_phys_addr: u64,
        buffer_virt_addr: u64,
    ) -> Result<(), &'static str> {
        if let Some(node) = self.virtual_ocean.get(noun) {
            // SAFETY: La copie est sécurisée car le pointeur source (node) vient du BTreeMap
            // et la destination est le buffer DMA alloué par le noyau.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    core::ptr::from_ref::<TrieNode>(node).cast::<u8>(),
                    buffer_virt_addr as *mut u8,
                    core::mem::size_of::<TrieNode>(),
                );
            }
            return Ok(());
        }

        if let Some(node) = self.core_ocean.get(noun) {
            // SAFETY: Même logique, copie depuis le CoreOcean vers le buffer DMA.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    core::ptr::from_ref::<TrieNode>(node).cast::<u8>(),
                    buffer_virt_addr as *mut u8,
                    core::mem::size_of::<TrieNode>(),
                );
            }
            return Ok(());
        }

        let lba = self
            .disk_index
            .get_lba(noun)
            .ok_or("Erreur critique : Noun introuvable dans tous les océans")?;
        self.disk_storage.fetch_node(lba, buffer_phys_addr)?;

        Ok(())
    }

    /// La fusion s'opère et s'enregistre EXCLUSIVEMENT dans le `VirtualOcean` (RAM).
    pub fn merge_trees(
        &mut self,
        noun_a: &Noun,
        noun_b: &Noun,
        node_phys_addr: u64,
        node_virt_addr: u64,
    ) -> Result<Noun, &'static str> {
        if noun_a == noun_b {
            return Ok(noun_b.clone());
        }

        self.fetch_node_cascade(noun_a, node_phys_addr, node_virt_addr)?;
        // SAFETY: Le pointeur virtuel est garanti valide car le buffer vient d'être peuplé par la cascade.
        let node_a = unsafe { &*(node_virt_addr as *const TrieNode) }.clone();

        let node_b_virt = node_virt_addr + 4096;
        let node_b_phys = node_phys_addr + 4096;
        self.fetch_node_cascade(noun_b, node_b_phys, node_b_virt)?;
        // SAFETY: Le pointeur virtuel décalé de 4096 octets est valide et peuplé.
        let node_b = unsafe { &*(node_b_virt as *const TrieNode) }.clone();

        let mut merged_node = TrieNode::new();
        merged_node.opcode = node_b.opcode;
        merged_node.flags = node_b.flags;
        merged_node.param = node_b.param;

        if node_b.mask == 0 {
            merged_node.mask = 0;
        } else {
            merged_node.mask = node_a.mask | node_b.mask;

            for i in 0..16 {
                let in_a = node_a.has_branch(i);
                let in_b = node_b.has_branch(i);

                let target_noun = match (in_a, in_b) {
                    (false, true) => node_b.branches[i as usize].clone(),
                    (true, false) => node_a.branches[i as usize].clone(),
                    (true, true) => {
                        // CORRECTION : Plus de unwrap() ! On utilise des if-let pour ignorer silencieusement si la donnée est cassée
                        let Some(sub_a) = node_a.get_branch(i) else {
                            continue;
                        };
                        let Some(sub_b) = node_b.get_branch(i) else {
                            continue;
                        };

                        self.merge_trees(
                            sub_a,
                            sub_b,
                            node_phys_addr + 8192,
                            node_virt_addr + 8192,
                        )?
                    }
                    (false, false) => continue,
                };

                merged_node.set_branch(i, target_noun);
            }
        }

        // CORRECTION : Déplacé hors du if/else (branches_sharing_code)
        merged_node.payload = node_b.payload;

        let new_noun = self.write_virtual_node(merged_node);

        Ok(new_noun)
    }

    pub fn write_virtual_node(&mut self, node: TrieNode) -> Noun {
        // On calcule l'identité du nœud
        let noun_bytes = node.calculate_noun();
        let noun = Noun::of(&noun_bytes);

        // On l'ajoute uniquement en RAM dans le contexte du Prisme
        self.virtual_ocean.insert(noun.clone(), node);

        // On retourne la nouvelle identité cryptographique
        noun
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::TrieNode;
    use crate::storage::{BlockDevice, StorageEngine};
    use alloc::collections::BTreeMap;
    use noun::Noun;

    // 1. On crée un faux Disque NVMe en RAM pour les tests
    struct MockNvme {
        read_count: core::cell::Cell<u32>,
    }
    impl MockNvme {
        fn new() -> Self {
            Self {
                read_count: core::cell::Cell::new(0),
            }
        }
        fn get_reads(&self) -> u32 {
            self.read_count.get()
        }
    }

    impl BlockDevice for MockNvme {
        fn write_node(&self, _lba: u64, _data_phys_addr: u64) {}

        fn read_node(&self, _lba: u64, _dest_phys_addr: u64) {
            self.read_count.set(self.read_count.get() + 1);
        }
    }
    #[test]
    fn test_fetch_node_cascade_priorities() {
        let mock_nvme = MockNvme::new();
        let mut core_ocean = BTreeMap::new();

        // 1. On injecte un Noun dans le CoreOcean AVANT de créer le routeur
        let noun_core = Noun::of(&[0xCC; 32]);
        let mut node_core = TrieNode::new();
        node_core.opcode = 0x99;
        core_ocean.insert(noun_core.clone(), node_core);

        // 2. Création unique du routeur (il prend la référence de core_ocean déjà rempli)
        let storage = StorageEngine::new(1, &mock_nvme);
        let mut router = SemanticRouter::new(storage, &core_ocean);

        // 3. On injecte un index artificiel pour le disque
        let noun_disk = Noun::of(&[0xDD; 32]);
        router.disk_index.insert(noun_disk.clone(), 42).unwrap();

        // 4. On injecte un Noun dans le VirtualOcean (RAM du Plan)
        let noun_virt = Noun::of(&[0xBB; 32]);
        let mut node_virt = TrieNode::new();
        node_virt.opcode = 0x88;
        router.virtual_ocean.insert(noun_virt.clone(), node_virt);

        // Buffer temporaire pour simuler la RAM DMA
        let mut dest_node = TrieNode::new();
        let dest_addr = &mut dest_node as *mut _ as u64;

        // TEST 1 : Lecture du VirtualOcean
        router
            .fetch_node_cascade(&noun_virt, dest_addr, dest_addr)
            .unwrap();
        assert_eq!(mock_nvme.get_reads(), 0, "Le NVMe ne doit pas être lu !");
        assert_eq!(dest_node.opcode, 0x88, "Mauvaise provenance (Virtual) !");

        // TEST 2 : Lecture du CoreOcean (Fallback)
        router
            .fetch_node_cascade(&noun_core, dest_addr, dest_addr)
            .unwrap();
        assert_eq!(
            mock_nvme.get_reads(),
            0,
            "Le NVMe ne doit toujours pas être lu !"
        );
        assert_eq!(dest_node.opcode, 0x99, "Mauvaise provenance (Core) !");

        // TEST 3 : Lecture du DiskOcean (Dernier recours)
        router
            .fetch_node_cascade(&noun_disk, dest_addr, dest_addr)
            .unwrap();
        assert_eq!(
            mock_nvme.get_reads(),
            1,
            "Le NVMe aurait dû être sollicité !"
        );
    }
}
