use blake3::Hasher;
use core::clone::Clone;
use core::debug_assert;
use core::default::Default;
use core::derive;
use core::fmt::Debug;
use core::option::Option;
use core::option::Option::{None, Some};
use kpack::{Opcode, execute};
use noun::Noun;

/// The `TrieNode` struct represents a node in the Merkle tree of the filesystem. Each `TrieNode` occupies 4096 bytes, which is exactly 8 standard `Nvme` sectors of 512 bytes.
///
/// The structure is designed to be aligned to 4096 bytes to ensure efficient access and manipulation on `Nvme` devices.
///
/// # Fields
/// - `mask`: A 16-bit mask indicating which branches are present in the node.
/// - `opcode`: An 8-bit field representing the operation code for the node's recipe.
/// - `flags`: An 8-bit field for additional flags related to the node's operation.
/// - `param`: A 32-bit parameter that can be used in conjunction with the opcode for various operations.
/// - `branches`: An array of 16 Nouns, each representing a branch in the Merkle tree. Each Noun is a 32-byte cryptographic identity.
/// - `payload`: A 3576-byte array that holds the data or recipe associated with the node. The size is calculated to ensure that the total size of the `TrieNode` is 4096 bytes, accounting for the other fields.
/// # Alignment
/// The `TrieNode` is aligned to 4096 bytes to match the size of a standard `Nvme` sector, ensuring that each node can be read or written in a single operation without crossing sector boundaries. This alignment is crucial for performance and data integrity when interacting with `Nvme` devices.
#[cfg_attr(not(test), repr(C, align(4096)))]
#[cfg_attr(test, repr(C))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrieNode {
    pub mask: u16,
    pub opcode: u8,
    pub flags: u8,
    pub param: u32,
    pub branches: [Noun; 16],
    pub payload: [u8; 3576],
}

impl Default for TrieNode {
    fn default() -> Self {
        Self::new()
    }
}

impl TrieNode {
    /// Creates a new `TrieNode` with default values. The mask is set to 0, indicating no branches are present. The opcode is set to `Lit`, which is the default operation for a new node. The flags and param fields are initialized to 0. All branches are initialized to a default Noun with a zeroed 32-byte array, and the payload is filled with zeros.
    /// # Returns
    /// A new instance of `TrieNode` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mask: 0,
            opcode: Opcode::Lit as u8,
            flags: 0,
            param: 0,
            branches: core::array::from_fn(|_| Noun::of(&[0u8; 32])),
            payload: [0; 3576],
        }
    }
    /// Executes the recipe associated with the `TrieNode`.
    ///
    /// The recipe is determined by the opcode and may involve operations such as literal data output,
    /// run-length encoding, seeding, patching, or delta application. The output of the recipe is written to the provided `output_buffer`.
    ///
    /// If the recipe requires a reference buffer (e.g., for delta operations), it can be provided as an optional parameter.
    ///
    /// # Parameters
    /// - `output_buffer`: A mutable slice where the output of the recipe will be written.
    /// - `reference_buffer`: An optional slice that may be used as a reference for certain operations, such as delta application.
    /// # Panics
    /// This function will panic if the opcode is unknown or corrupted, as it relies on a valid opcode to determine the operation to perform.
    ///
    /// # Example
    /// ```no_run
    /// use jinshu::node::TrieNode;
    /// use jinshu::node::Noun;
    /// let node = TrieNode::new();
    /// let mut output = [0u8; 1024];
    /// node.execute_recipe(&mut output, None);
    /// ```
    /// # Returns
    /// This function does not return a value. The result of the recipe execution is written directly to the `output_buffer`.
    pub fn execute_recipe(&self, output_buffer: &mut [u8], reference_buffer: Option<&[u8]>) {
        if let Some(op) = Opcode::from_u8(self.opcode) {
            execute(
                &op,
                self.param,
                &self.payload,
                output_buffer,
                reference_buffer,
            );
        }
    }

    /// Checks if a branch exists for the given nibble (0-15). The mask field of the `TrieNode` is used to determine the presence of branches. Each bit in the mask corresponds to a branch, where a set bit indicates that the branch is present.
    /// # Parameters
    /// - `nibble`: A value between 0 and 15 representing the branch to check.
    /// # Returns
    /// - `true` if the branch exists (the corresponding bit in the mask is set).
    /// - `false` if the branch does not exist (the corresponding bit in the mask is not set).
    /// # Panics
    /// This function will panic if the provided nibble is not in the range of 0 to 15, as it is expected to represent a valid branch index.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use jinshu::node::TrieNode;
    /// let node = TrieNode::new();
    /// assert!(node.has_branch(0) == false);
    /// ```
    ///
    #[must_use]
    pub fn has_branch(&self, nibble: u8) -> bool {
        debug_assert!(nibble < 16);
        (self.mask & (1 << nibble)) != 0
    }
    /// Retrieves the Noun associated with a specific branch, if it exists. The function first checks if the branch is present using the `has_branch` method. If the branch exists, it returns a reference to the corresponding Noun in the branches array. If the branch does not exist, it returns `None`.
    /// # Parameters
    /// - `nibble`: A value between 0 and 15 representing the branch to retrieve.
    /// # Returns
    /// - `Some(&Noun)` if the branch exists.
    /// - `None` if the branch does not exist.
    /// # Panics
    /// This function will panic if the provided nibble is not in the range of 0 to 15, as it is expected to represent a valid branch index.
    /// # Example
    /// ```no_run
    /// use jinshu::node::TrieNode;
    /// use jinshu::node::Noun;
    /// let node = TrieNode::new();
    /// if let Some(noun) = node.get_branch(0) {
    ///     // Use the noun associated with branch 0
    /// } else {
    ///     // Branch 0 does not exist
    /// }
    /// ```
    #[must_use]
    pub fn get_branch(&self, nibble: u8) -> Option<&Noun> {
        if self.has_branch(nibble) {
            Some(&self.branches[nibble as usize])
        } else {
            None
        }
    }
    /// Sets the Noun for a specific branch and updates the mask to indicate that the branch is present. The function takes a nibble (0-15) representing the branch index and a Noun to associate with that branch. It sets the corresponding bit in the mask to indicate that the branch is now present.
    /// # Parameters
    /// - `nibble`: A value between 0 and 15 representing the branch to set.
    /// - `noun`: The Noun to associate with the specified branch.
    /// # Panics
    /// This function will panic if the provided nibble is not in the range of 0 to 15, as it is expected to represent a valid branch index.
    pub fn set_branch(&mut self, nibble: u8, noun: Noun) {
        debug_assert!(nibble < 16);
        self.branches[nibble as usize] = noun;
        self.mask |= 1 << nibble;
    }
    /// Calculates the cryptographic identity (Noun) of the `TrieNode` by hashing its entire content using the BLAKE3 hashing algorithm.
    ///
    /// The resulting hash is a 32-byte array that uniquely represents the state of the `TrieNode`.
    ///
    /// This identity can be used for integrity checks, deduplication, and as a reference in the Merkle tree structure.
    ///
    /// # Returns
    /// A 32-byte array representing the cryptographic identity of the `TrieNode`.
    /// # Panics
    /// This function will panic if the hashing operation fails, which is unlikely under normal circumstances.
    ///
    /// # Example
    /// ```no_run
    /// use jinshu::node::TrieNode;
    /// let node = TrieNode::new();
    /// let noun_identity = node.calculate_noun();
    /// ```
    #[must_use]
    pub fn calculate_noun(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        let node_bytes = unsafe {
            // SAFETY: This is safe because we are creating a byte slice from a reference to self,
            // which is guaranteed to be valid for the lifetime of the function.
            // The size of the slice is determined by the size of the `TrieNode` struct, which is known at compile time.
            core::slice::from_raw_parts(
                core::ptr::from_ref::<Self>(self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        };
        hasher.update(node_bytes);
        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::assert;
    use core::assert_eq;

    #[test]
    fn test_trie_node_creation() {
        let node = TrieNode::new();
        assert_eq!(node.mask, 0);
        assert_eq!(node.opcode, Opcode::Lit as u8);
        assert_eq!(node.flags, 0);
        assert_eq!(node.param, 0);
        for branch in &node.branches {
            assert_eq!(branch, &Noun::of(&[0; 32]));
        }
        assert_eq!(node.payload, [0; 3576]);
    }

    #[test]
    fn test_recipe_lit() {
        let mut node = TrieNode::new();
        node.opcode = Opcode::Lit as u8;
        node.param = 5;
        node.payload[..5].copy_from_slice(b"Hello");
        let mut buffer = [0u8; 10];
        node.execute_recipe(&mut buffer, None);
        assert_eq!(&buffer[..5], b"Hello");
    }

    #[test]
    fn test_recipe_rle() {
        let mut node = TrieNode::new();
        node.opcode = Opcode::Rle as u8;
        node.param = 10;
        node.payload[0] = b'X';
        let mut buffer = [0u8; 10];
        node.execute_recipe(&mut buffer, None);
        assert_eq!(&buffer, b"XXXXXXXXXX");
    }

    #[test]
    fn test_recipe_seed_and_patch() {
        let mut node = TrieNode::new();
        node.opcode = Opcode::Seed as u8;
        node.param = 42;
        node.payload[0] = 0x01;
        node.payload[1] = 0x00;
        node.payload[2] = 0x00;
        node.payload[3] = 0x00;
        node.payload[4] = 0x4B;
        let mut buffer = [0u8; 16];
        node.execute_recipe(&mut buffer, None);
        assert_eq!(buffer[0], b'K');
    }

    #[test]
    fn test_recipe_delta() {
        let mut node = TrieNode::new();
        node.opcode = Opcode::Delta as u8;
        node.param = 0;
        node.payload[0] = 0x01;
        node.payload[1] = 0x00;
        node.payload[2] = 0x00;
        node.payload[3] = 0x04;
        node.payload[4] = 0x00;
        node.payload[5] = 0x00;
        let parent = b"AmentysOSBest";
        let mut buffer = [0u8; 4];
        node.execute_recipe(&mut buffer, Some(parent));
        assert_eq!(&buffer, b"Amen");
    }

    #[test]
    fn test_trie_node_has_branch() {
        let mut node = TrieNode::new();
        assert!(!node.has_branch(0));
        node.mask = 1;
        assert!(node.has_branch(0));
    }

    #[test]
    fn test_trie_node_get_branch() {
        let mut node = TrieNode::new();
        assert!(node.get_branch(0).is_none());
        node.mask = 1;
        assert!(node.get_branch(0).is_some());
    }

    #[test]
    fn test_trie_node_set_branch() {
        let mut node = TrieNode::new();
        assert!(!node.has_branch(0));
        node.mask |= 1 << 0;
        assert!(node.has_branch(0));
    }
}
