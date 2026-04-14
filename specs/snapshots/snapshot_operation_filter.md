# FILTER Operation: `filter_manifest()`

[README](README.md) · FILTER Operation

**Location:** `ops/filter.rs`

Applies a filter predicate to manifest entries, returning a new manifest with only matching entries.

```rust
pub fn filter_manifest<P: Clone, K: Clone>(
    manifest: &Manifest<P, K>,
    filter: &dyn Fn(&ManifestEntry) -> bool,
) -> Manifest<P, K>
```

## Parameters

| Parameter | Description |
|-----------|-------------|
| `manifest` | Any manifest type (`AbsSnapshot`, `Snapshot`, `AbsSnapshotDiff`, `SnapshotDiff`) |
| `filter` | Closure taking `&ManifestEntry` (enum of `&FileEntry` or `&DirEntry`), returns `true` to keep |

## Returns

A new manifest of the same type containing only entries that pass the filter. The returned manifest has:
- `total_size` recomputed from filtered entries
- `parent_manifest_hash` preserved
- `file_chunk_size_bytes` preserved

## Implementation

```rust
pub fn filter_manifest<P: Clone, K: Clone>(
    manifest: &Manifest<P, K>,
    filter: &dyn Fn(&ManifestEntry) -> bool,
) -> Manifest<P, K> {
    let files = manifest.files.iter()
        .filter(|f| filter(&ManifestEntry::File(f)))
        .cloned().collect();
    let dirs = manifest.dirs.iter()
        .filter(|d| filter(&ManifestEntry::Dir(d)))
        .cloned().collect();
    let mut result = Manifest::new(manifest.hash_alg, manifest.file_chunk_size_bytes);
    result.files = files;
    result.dirs = dirs;
    result.parent_manifest_hash = manifest.parent_manifest_hash.clone();
    result.recompute_total_size();
    result
}
```

## Built-in Filter: `IncludeExcludePathsFilter`

```rust
pub struct IncludeExcludePathsFilter {
    include: Vec<glob::Pattern>,
    exclude: Vec<glob::Pattern>,
}

impl IncludeExcludePathsFilter {
    pub fn new(include: &[&str], exclude: &[&str]) -> Result<Self, glob::PatternError>;
    pub fn matches(&self, entry: &ManifestEntry) -> bool;
}
```

**Pattern Matching Rules:**

| Rule | Description |
|------|-------------|
| Include empty | All paths are candidates |
| Include specified | Path must match at least one include pattern |
| Exclude | Path must not match any exclude pattern |
| Evaluation order | Include checked first, then exclude |

Uses `glob::Pattern` for matching (`*`, `?`, `[seq]`, `**`).

## Using FILTER with DIFF

Apply filters consistently to both manifests:

| Scenario | Result |
|----------|--------|
| Same filter on both | ✓ Correct diff |
| No filters on either | ✓ Correct diff |
| Filter on only one | ✗ Incorrect diff |
| Different filters | ✗ Incorrect diff |

```rust
let filter = IncludeExcludePathsFilter::new(&["*.blend"], &["backup/*"])?;
let filtered_parent = filter_manifest(&parent, &|e| filter.matches(e));
let filtered_current = filter_manifest(&current, &|e| filter.matches(e));
let diff = diff_snapshots(&filtered_parent, &filtered_current, &DiffOptions::default())?;
```

## Relationship to Other Operations

| Operation | Relationship |
|-----------|--------------|
| DIFF | Filter both manifests before diffing |
| SUBTREE | FILTER keeps/removes entries; SUBTREE extracts and rebases paths |
| COLLECT | FILTER operates on manifests; COLLECT creates manifests from filesystem |

## When to Use FILTER vs SUBTREE

| Use Case | Recommended |
|----------|-------------|
| Keep only certain file types | FILTER |
| Exclude backup directories | FILTER |
| Extract a subdirectory as new root | SUBTREE |
| Remove paths by pattern | FILTER |
| Convert absolute to relative paths | SUBTREE |
