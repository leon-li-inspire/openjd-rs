# COLLECT Operation: `collect_abs_snapshot()`

[README](README.md) · COLLECT Operation

**Location:** `ops/collect.rs`

Collects provided lists of paths into an absolute-path snapshot manifest WITHOUT computing hashes.

```rust
pub fn collect_abs_snapshot(
    directories: &[impl AsRef<Path>],
    filenames: &[impl AsRef<Path>],
    options: CollectOptions,
) -> Result<AbsSnapshot>
```

## Parameters

```rust
pub struct CollectOptions {
    pub optional_filenames: Vec<PathBuf>,
    pub symlink_policy: SymlinkPolicy,
    pub file_chunk_size_bytes: Option<i64>,
}
```

| Parameter | Description |
|-----------|-------------|
| `directories` | Directory paths whose full contents are collected recursively. Must exist and be directories. Empty directories are included. |
| `filenames` | File/symlink paths that must exist. Raises `FileNotFound` if missing. |
| `optional_filenames` | File/symlink paths to include if they exist. Missing files silently ignored. |
| `symlink_policy` | How to handle symlinks. Default `CollapseEscaping`. |
| `file_chunk_size_bytes` | `None` = use `DEFAULT_FILE_CHUNK_SIZE` (256MB). `WHOLE_FILE_CHUNK_SIZE` (-1) = no chunking. |

## Returns

`AbsSnapshot` with:
- File entries with `hash=None` (unhashed)
- Symlink entries with `symlink_target` set to absolute paths
- Directory entries for empty directories

## Validation Rules

| Condition | Behavior |
|-----------|----------|
| File in `filenames` does not exist | `SnapshotError::FileNotFound` |
| File in `optional_filenames` does not exist | Silently ignored |
| Directory in `directories` does not exist | `SnapshotError::Io` |
| Path in `directories` is not a directory | `SnapshotError::Validation` |

## Implementation Details

- Uses `walkdir::WalkDir` for directory traversal
- `file_meta()` helper extracts `(size, mtime_micros, runnable)` from `std::fs::Metadata`
  - `mtime` is microseconds since epoch via `modified().duration_since(UNIX_EPOCH).as_micros()`
  - `runnable` checks POSIX execute bit (`mode & 0o111 != 0`); always `false` on non-Unix
- `abs_normalized()` converts paths to absolute normalized form via `std::path::absolute()` + `normalize_path()`
- Two-pass symlink handling for `CollapseEscaping`/`ExcludeEscaping`:
  1. Walk directories collecting non-symlink entries into a `HashSet<String>` (collected set)
  2. Process `DeferredSymlink` entries against the collected set

### DeferredSymlink

```rust
struct DeferredSymlink {
    path: String,       // Absolute normalized path of the symlink
    fs_path: PathBuf,   // Filesystem path for read_link/metadata calls
}
```

## Symlink Policy Options

| Policy | Description |
|--------|-------------|
| `CollapseEscaping` | Preserve symlinks whose targets are included; collapse escaping symlinks |
| `CollapseAll` | Collapse all symlinks to files/directories |
| `Preserve` | Keep all symlinks as-is with absolute targets |
| `TransitiveIncludeTargets` | Keep symlinks and add their targets to the manifest |
| `ExcludeAll` | Skip all symlinks |
| `ExcludeEscaping` | Preserve symlinks whose targets are included; exclude escaping symlinks |

See [snapshot_symlink_handling.md](snapshot_symlink_handling.md) for detailed documentation.

## Example

```rust
use openjd_snapshots::{collect_abs_snapshot, CollectOptions, SymlinkPolicy};

let manifest = collect_abs_snapshot(
    &["/data/shared/models", "/data/shared/textures"],
    &["/home/user/project/scene.blend"],
    CollectOptions {
        optional_filenames: vec!["/home/user/project/cache.bin".into()],
        symlink_policy: SymlinkPolicy::CollapseEscaping,
        ..Default::default()
    },
)?;

for entry in &manifest.files {
    if let Some(ref target) = entry.symlink_target {
        println!("symlink: {} -> {}", entry.path, target);
    } else {
        println!("file: {} ({}B)", entry.path, entry.size.unwrap_or(0));
    }
}
```
