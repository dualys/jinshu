#![cfg_attr(not(test), no_std)]

#[doc = "The jinshu crate is the core of the Jinshu database engine. It provides the storage engine, disk allocator, and data structures for managing `TrieNodes` on `Nvme` devices."]
pub mod storage;

#[doc = "The node module provides the data structures and algorithms for managing `TrieNodes` within the Jinshu database engine."]
pub mod node;

#[doc = "The router module provides the routing logic for the Jinshu database engine, allowing for efficient data retrieval and storage across multiple nodes."]
pub mod router;

#[doc = "The ocean module provides the core data structures and algorithms for managing the ocean within the Jinshu database engine."]
pub mod ocean;
