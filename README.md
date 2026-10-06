# jinshu

Merkle-backed storage engine and Noun graph database on NVMe.

`jinshu` stores a graph of 4096-byte `TrieNode`s. Each node is a content-addressed page: sixteen child [`Noun`](https://docs.rs/noun)s, a presence mask, and a [`kpack`](https://docs.rs/kpack) recipe in the payload. Pages are appended to a block device in 8-LBA extents. Lookup walks RAM first, then the on-disk index.

`no_std`. The router and the ocean loader use `alloc`.

## Install

```toml
[dependencies]
jinshu = "0.1.0"
```

```sh
cargo add jinshu
```

Direct dependencies: `noun`, `kpack`, `blake3` (`default-features = false`), `heapless`.

## Layout

| Module | Role |
| --- | --- |
| `node` | The 4096-byte page and its recipe. |
| `storage` | Append-only LBA allocator and `BlockDevice` trait. |
| `router` | Noun-to-LBA index, cascade fetch, in-RAM merge. |
| `ocean` | Boot-time load of a noun and its subtree into RAM. |

## A page

`TrieNode` is `#[repr(C)]` and, outside tests, aligned to 4096. The header is 8 bytes, the branch table is 16 × 32 bytes, and the payload is 3576 bytes.

| Field | Meaning |
| --- | --- |
| `mask: u16` | Bit `i` set means branch `i` is present. |
| `opcode: u8` | A `kpack::Opcode`. A new node starts as `Lit`. |
| `flags: u8` | Caller-defined. |
| `param: u32` | Passed through to `kpack::execute`. |
| `branches: [Noun; 16]` | Child identities, one per nibble. |
| `payload: [u8; 3576]` | Recipe input. |

```rust
use jinshu::node::TrieNode;
use kpack::Opcode;
use noun::Noun;

let mut node = TrieNode::new();
node.opcode = Opcode::Lit as u8;
node.param = 5;
node.payload[..5].copy_from_slice(b"Hello");

let mut out = [0u8; 8];
node.execute_recipe(&mut out, None);
assert_eq!(&out[..5], b"Hello");

node.set_branch(0xA, Noun::of(b"child"));
assert!(node.has_branch(0xA));
assert!(node.get_branch(0xA).is_some());

// BLAKE3 of the raw page. `write_virtual_node` then wraps this in `Noun::of`.
let id = Noun::of(&node.calculate_noun());
assert!(!id.is_null());
```

`has_branch`, `get_branch`, and `set_branch` take a nibble in `0..16`. An out-of-range nibble hits a `debug_assert`. `execute_recipe` no-ops if `opcode` is not a known `kpack` opcode. `calculate_noun` hashes the struct bytes, including unused branch slots.

## Disk

A node occupies 8 sectors of 512 bytes. `DiskAllocator` starts at LBA 8, so the first eight sectors stay reserved, and hands out the next free extent. `allocate_node` returns `None` when `current + 8` would pass `total_lbas`.

`BlockDevice` is the hardware seam. Both methods take a physical address; the engine does not copy the page itself.

```rust
use jinshu::storage::{BlockDevice, StorageEngine};

struct Nvme;
impl BlockDevice for Nvme {
    fn write_node(&self, lba: u64, data_phys_addr: u64) { /* DMA 4096 bytes */ }
    fn read_node(&self, lba: u64, dest_phys_addr: u64) { /* DMA 4096 bytes */ }
}

let device = Nvme;
let mut engine = StorageEngine::new(1024, &device);
let lba = engine.persist_node(0x1000).expect("disk full");
engine.fetch_node(lba, 0x1000).expect("read failed");
```

`persist_node` allocates, calls `write_node`, and returns the starting LBA. `fetch_node` calls `read_node` and returns `Ok(())`; the trait itself has no error channel.

## Index and cascade

`NounIndex` is a `heapless::Vec` of `(Noun, LBA)`, capped at `MAX_ENTRIES` (1024). `insert` fails with `"ram full: NounIndex cannot hold more entries"`. Lookup is a linear scan.

`SemanticRouter` searches three layers, in order:

1. `virtual_ocean` — private RAM, written by `write_virtual_node` and `merge_trees`.
2. `core_ocean` — shared RAM, borrowed at construction.
3. `disk_index`, then `StorageEngine::fetch_node` into the caller DMA buffer.

`fetch_node_cascade` copies a RAM hit into `buffer_virt_addr`. A disk hit only programs the DMA read to `buffer_phys_addr`. The index starts empty; load it at boot, then `insert` each pair.

`merge_trees` unions two nodes into `virtual_ocean` only. Payload, opcode, flags, and param come from `noun_b`. The mask is the bitwise or. A branch present on one side is kept; a branch present on both sides is merged recursively. Two equal nouns return immediately. The new identity is `Noun::of` of `calculate_noun`.

## Boot load

`CoreOceanBuilder::deep_load` copies a noun and every present child from NVMe into a `BTreeMap<Noun, TrieNode>`. It needs the index, a `StorageEngine`, and one reusable 4096-byte DMA buffer (`phys_addr` for the device, `virt_addr` for the CPU). A missing noun returns `"Init Error: Vital Noun not found on NVMe"`.

## Documentation

API docs are on [docs.rs/jinshu](https://docs.rs/jinshu).

## License

[AGPL-3.0-or-later](LICENSE).
