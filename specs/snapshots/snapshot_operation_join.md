# JOIN Operation

[README](README.md) · JOIN Operation

**Location:** `ops/join.rs`

Joins a prefix to all paths in a manifest, producing a new manifest with prefixed paths. The inverse of SUBTREE.

Rust provides separate functions for each type combination:

```rust
pub fn join_snapshot(manifest: &Snapshot, prefix: &str) -> Result<AbsSnapshot>
pub fn join_snapshot_diff(manifest: &SnapshotDiff, prefix: &str) -> Result<AbsSnapshotDiff>
pub fn join_snapshot_rel(manifest: &Snapshot, prefix: &str) -> Result<Snapshot>
pub fn join_snapshot_diff_rel(manifest: &SnapshotDiff, prefix: &str) -> Result<SnapshotDiff>
```

## Parameters

| Parameter | Description |
|-----------|-------------|
| `manifest` | Source manifest with relative paths |
| `prefix` | Path prefix to join. Must not be empty. |

## Path Style Behavior

| Function | Prefix Type | Output |
|----------|-------------|--------|
| `join_snapshot` | Absolute | `AbsSnapshot` |
| `join_snapshot_diff` | Absolute | `AbsSnapshotDiff` |
| `join_snapshot_rel` | Relative | `Snapshot` |
| `join_snapshot_diff_rel` | Relative | `SnapshotDiff` |

## What Gets Prefixed

- File paths (`entry.path`)
- Directory paths (`dir.path`)
- Symlink targets (`entry.symlink_target`)

All via `join_path(prefix, path)` which normalizes the result.

## What Gets Preserved / Dropped

- `file_chunk_size_bytes` — preserved
- `total_size` — preserved
- All file metadata (hash, size, mtime, runnable, chunk_hashes) — preserved
- `parent_manifest_hash` — NOT preserved (set to `None`; root changed)

## Implementation

```rust
fn join_impl<P, K, Q>(manifest: &Manifest<P, K>, prefix: &str) -> Manifest<Q, K> {
    let files = manifest.files.iter().map(|f| join_file(prefix, f)).collect();
    let dirs = manifest.dirs.iter().map(|d| join_dir(prefix, d)).collect();
    let mut result = Manifest::new(manifest.hash_alg, manifest.file_chunk_size_bytes);
    result.files = files;
    result.dirs = dirs;
    result.total_size = manifest.total_size;
    result.parent_manifest_hash = None;
    result
}
```

## Example

```rust
use openjd_snapshots::{join_snapshot, join_snapshot_diff};

// Convert relative to absolute
let abs = join_snapshot(&rel_manifest, "/projects/scene/assets/textures")?;

// Combine multiple manifests for download
let textures_abs = join_snapshot(&textures, "/projects/scene/assets/textures")?;
let models_abs = join_snapshot(&models, "/projects/scene/assets/models")?;
```

## Relationship to SUBTREE

JOIN and SUBTREE are inverse operations:

| Operation | Input | Output | Path Transformation |
|-----------|-------|--------|---------------------|
| SUBTREE | Manifest + subtree path | RelManifest | Removes prefix |
| JOIN | RelManifest + prefix | Manifest | Adds prefix |
