# COMPOSE Operation

[README](README.md) · COMPOSE Operation

**Location:** `ops/compose.rs`

Layers multiple manifests together into a single manifest, as if applying each manifest as a set of changes in order. Rust provides two separate functions making the type relationships explicit at compile time:

```rust
pub fn compose_snapshot_with_diffs<P: Clone>(
    base: &Manifest<P, Full>,
    diffs: &[&Manifest<P, Diff>],
) -> Manifest<P, Full>

pub fn compose_diffs<P: Clone>(
    diffs: &[&Manifest<P, Diff>],
) -> Result<Manifest<P, Diff>>
```

## Composition Semantics

The result represents the directory tree you would get by:
1. Starting with the first manifest's directory tree
2. Applying each subsequent manifest as a "patch" — adding new entries, updating modified entries, removing deleted entries

### Snapshot + Diffs → Snapshot

`compose_snapshot_with_diffs(base, &[diff1, diff2])`:
- `deleted=true` markers remove entries from the result
- Output is a `Manifest<P, Full>` (no deletion markers)

### Diffs → Diff

`compose_diffs(&[diff1, diff2, diff3])`:
- Tracks cumulative changes including deletion markers
- `parent_manifest_hash` comes from the first diff
- Output includes both current entries AND deletion markers

## Trie-Based Implementation

Both functions use a trie (prefix tree) where each node represents a path component:

```rust
struct TrieNode {
    children: HashMap<String, TrieNode>,
    file_entry: Option<FileEntry>,
    dir_deleted: bool,
}
```

### Why a Trie?

1. **Efficient path operations:** Insert, lookup, delete are O(path depth)
2. **Natural directory structure:** Mirrors the filesystem hierarchy
3. **Cascading deletions:** Directory subtrees can be efficiently removed or marked

### Snapshot + Diffs

1. Insert base snapshot entries into trie
2. For each diff: deletions remove nodes (`delete_file`), additions/modifications insert or update nodes
3. Final trie contains only entries that exist after all diffs applied
4. Deletion markers NOT preserved in output

### Diff + Diffs

1. Each node has a `dir_deleted` flag to track deletion markers
2. Deletions set `dir_deleted=true` via `mark_deleted`; additions clear it and set `file_entry`
3. After all diffs applied, `reconcile_deleted_flags()` handles the case where a deleted directory has non-deleted children

### The Reconciliation Step

`reconcile_deleted_flags()` handles this scenario:
1. diff1 deletes `/dir/` and all its contents
2. diff2 adds `/dir/newfile.txt`

After diff2, `/dir/` must NOT be marked as deleted because it has a non-deleted child. The reconciliation traverses the trie depth-first and clears `dir_deleted` on any node with non-deleted descendants.

## Validation Rules

| Condition | Behavior |
|-----------|----------|
| Empty diffs list | Returns base unchanged (snapshot+diffs) or error (diffs-only) |
| Manifests have different `file_chunk_size_bytes` | Error |

## Example

```rust
use openjd_snapshots::{compose_snapshot_with_diffs, compose_diffs};

// Apply diffs to a base snapshot
let final_state = compose_snapshot_with_diffs(&base, &[&diff1, &diff2]);
// final_state is a Snapshot with deletions applied

// Combine multiple diffs
let combined = compose_diffs(&[&diff1, &diff2, &diff3])?;
// combined is a SnapshotDiff with deletion markers preserved
```

## Relationship to PARTITION and JOIN

PARTITION is conceptually the inverse of multiple JOIN operations followed by COMPOSE:

```rust
let [(root1, rel1), (root2, rel2)] = partition_manifest(&abs_manifest, opts)?;

// Inverse:
let abs1 = join_snapshot(&rel1, &root1)?;
let abs2 = join_snapshot(&rel2, &root2)?;
let composed = compose_snapshot_with_diffs(&abs1, &[]);
// Then merge abs2 entries...
```
