# PARTITION Operation: `partition_manifest()`

[README](README.md) · PARTITION Operation

**Location:** `ops/partition.rs`

Partitions a manifest into multiple `(root, Snapshot)` pairs, dividing entries by their root paths. Each `Snapshot` is an extracted subtree with paths relative to its root.

```rust
pub fn partition_manifest(
    manifest: &AbsSnapshot,
    options: PartitionOptions,
) -> Result<Vec<(String, Snapshot)>>
```

## Parameters

```rust
pub struct PartitionOptions {
    pub roots: Option<Vec<String>>,
    pub referenced_paths: Option<Vec<String>>,
    pub symlink_policy: SymlinkPolicy,
}
```

| Parameter | Description |
|-----------|-------------|
| `roots` | Optional list of root paths. No root may be a subpath of another. |
| `referenced_paths` | Paths referenced by the workload, influencing auto-root determination even if no files exist under them. |
| `symlink_policy` | How to handle escaping symlinks. Only `CollapseAll`, `CollapseEscaping`, `ExcludeAll` supported. |

## Returns

`Vec<(String, Snapshot)>` where each tuple is `(root_path, relative_manifest)`.

## Validation Rules

| Condition | Behavior |
|-----------|----------|
| A root is a subpath of another root | Error |
| `Preserve` or `TransitiveIncludeTargets` policy | Error |

## Output Ordering

1. First: entries for each explicitly provided root (in `roots` order)
2. Then: auto-determined roots for remaining entries (sorted alphabetically)

If no entries exist under an explicitly provided root, its `Snapshot` is empty but still included.

## Auto-Root Determination

| Scenario | Platform | Behavior |
|----------|----------|----------|
| `roots` is `None` | POSIX | Single root: longest common directory prefix of all entries and referenced_paths |
| `roots` is `None` | Windows | One root per drive letter or UNC root |
| `roots` provided | Any | Provided roots first, then smallest set of additional roots for remaining entries |

### Longest Common Prefix

Implemented via `longest_common_prefix()`:
- Splits paths by `/`, finds common prefix components
- For absolute paths, splitting `/a/b` gives `["", "a", "b"]`; if only empty-string prefix matches, root is `"/"`

### Empty Directory Handling

Empty directories (in `manifest.dirs`) are included in root determination alongside file parent directories via `all_dir_paths()`.

## Implementation Details

- `parent_dir()` extracts the parent directory of a path
- `all_dir_paths()` collects all relevant directory paths for root determination
- `find_root_for_path()` matches a path to its containing root
- Each partition is produced by calling `subtree_snapshot()` with the root path

## Example

```rust
use openjd_snapshots::{partition_manifest, PartitionOptions};

// Auto-partition (no roots provided)
let partitions = partition_manifest(&manifest, PartitionOptions::default())?;
// Result: [("/projects/scene", Snapshot with relative paths)]

// Explicit roots with remainder
let partitions = partition_manifest(&manifest, PartitionOptions {
    roots: Some(vec!["/projects/scene".into()]),
    ..Default::default()
})?;
// Result: [
//   ("/projects/scene", Snapshot),      // explicit root
//   ("/data/shared", Snapshot),          // auto-determined remainder
// ]
```

## Relationship to SUBTREE and JOIN

PARTITION is conceptually the inverse of multiple JOIN + COMPOSE:

```rust
let [(root1, rel1), (root2, rel2)] = partition_manifest(&abs_manifest, opts)?;

// Inverse:
let abs1 = join_snapshot(&rel1, &root1)?;
let abs2 = join_snapshot(&rel2, &root2)?;
// compose abs1 and abs2 ≈ original abs_manifest
```
