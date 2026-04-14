# Content-Addressed Data Cache

[README](README.md) · Data Cache

**Location:** `data_cache.rs`

## Overview

A content-addressed data cache stores data using the content's hash as the key, enabling deduplication, efficient retrieval, and integrity verification. The crate provides two trait abstractions and two concrete implementations:

```
ContentAddressedDataCache (sync trait)
AsyncDataCache            (async trait, used by HASH_UPLOAD and DOWNLOAD)
├── S3DataCache           - Amazon S3 storage with multipart transfer support
└── FileSystemDataCache   - Local or network filesystem storage
```

## Storage Key Format

Files are stored with keys derived from their content hash:

```
{prefix}/{hash}.{algorithm}
```

Examples:
- S3: `Data/a1b2c3d4e5f67890abcdef1234567890.xxh128`
- Filesystem: `/mnt/cache/a1b2c3d4e5f67890abcdef1234567890.xxh128`

## Design Principles

### Hash-Then-Upload Invariant

HASH_UPLOAD always computes the hash while reading data for upload, never uploading based on a pre-computed hash alone. This guarantees the content-addressed storage invariant: data stored for a hash key always equals its hash.

**Large file exception:** When a file exceeds `max_memory_bytes` with `WHOLE_FILE_CHUNK_SIZE` mode, a two-pass approach is used. Per-part hashes are re-verified during the upload pass; mismatches abort the upload.

### Existence Checking Strategy

**S3DataCache:**
1. Check local `S3CheckCache` (SQLite database of known-existing hashes)
2. If miss, make `HeadObject` API call to S3
3. Cache positive results for future checks

**FileSystemDataCache:**
- Direct filesystem `exists()` check (no caching)

## Sync Trait: `ContentAddressedDataCache`

```rust
pub trait ContentAddressedDataCache: Send + Sync {
    fn object_key(&self, hash: &str, algorithm: &str) -> String;
    fn object_exists(&self, hash: &str, algorithm: &str) -> std::io::Result<bool>;
    fn put_object(&self, hash: &str, algorithm: &str, data: &[u8]) -> std::io::Result<String>;
    fn get_object(&self, hash: &str, algorithm: &str) -> std::io::Result<Vec<u8>>;
}
```

## Async Trait: `AsyncDataCache`

Used by HASH_UPLOAD and DOWNLOAD pipelines. Extends the sync interface with multipart and streaming operations:

```rust
#[async_trait]
pub trait AsyncDataCache: Send + Sync {
    fn object_key(&self, hash: &str, algorithm: &str) -> String;
    async fn object_exists(&self, hash: &str, algorithm: &str) -> std::io::Result<bool>;
    async fn put_object(&self, hash: &str, algorithm: &str, data: Vec<u8>) -> std::io::Result<String>;
    async fn get_object(&self, hash: &str, algorithm: &str) -> std::io::Result<Vec<u8>>;
    fn multipart_part_size(&self) -> usize;  // Default: 32MB

    // Multipart upload
    async fn create_multipart_upload(&self, hash: &str, algorithm: &str) -> std::io::Result<String>;
    async fn upload_part(&self, hash: &str, algorithm: &str, upload_id: &str,
                         part_number: i32, data: Vec<u8>) -> std::io::Result<String>;
    async fn complete_multipart_upload(&self, hash: &str, algorithm: &str, upload_id: &str,
                                       parts: Vec<(i32, String)>) -> std::io::Result<()>;
    async fn abort_multipart_upload(&self, hash: &str, algorithm: &str,
                                     upload_id: &str) -> std::io::Result<()>;

    // Byte-range download
    async fn get_object_range(&self, hash: &str, algorithm: &str,
                              start: u64, end: u64) -> std::io::Result<Vec<u8>>;

    // Streaming helpers (with default implementations)
    async fn stream_range_to_file_at_offset(/* ... */) -> std::io::Result<u64>;
    async fn copy_object_to_file(/* ... */) -> std::io::Result<u64>;
    async fn write_object_to_file_at_offset(/* ... */) -> std::io::Result<u64>;
}
```

The streaming helpers have default implementations that read into memory then write. `S3DataCache` and `FileSystemDataCache` can override for efficiency.

## FileSystemDataCache

Content-addressed storage backed by a local or network filesystem.

```rust
pub struct FileSystemDataCache {
    pub root_path: PathBuf,  // Must be absolute
}
```

### Construction

```rust
let cache = FileSystemDataCache::new("/tmp/debug_snapshot/Data")?;
// Validates root_path is absolute, creates directory if needed
```

### Behavior

- `object_key()` → `{root_path}/{hash}.{algorithm}`
- `object_exists()` → filesystem `exists()` check
- `put_object()` → write file to `{root_path}/{hash}.{algorithm}`
- `get_object()` → read file from cache
- Multipart operations are no-ops (single-file writes)

### Use Cases

- Debug snapshots: create portable zip files with manifest + data
- Local testing: test upload/download logic without S3
- Network storage: NFS or SMB mounted paths as shared caches

## S3DataCache

Content-addressed storage backed by Amazon S3.

```rust
pub struct S3DataCache {
    pub bucket: String,
    pub key_prefix: String,
    pub client: aws_sdk_s3::Client,
    pub s3_check_cache: Option<Arc<S3CheckCache>>,
    pub multipart_part_size: usize,  // Default: 32MB
    pub account_id: AccountId,
}

pub enum AccountId {
    Auto(String),       // Resolved at construction via STS
    Explicit(String),
    NoCheck,
}
```

### Account ID and Security

The `account_id` controls `ExpectedBucketOwner` on S3 API calls, preventing confused deputy attacks:

| Value | Behavior |
|-------|----------|
| `AccountId::Auto(id)` | Auto-detected from STS at construction |
| `AccountId::Explicit(id)` | Use provided account ID |
| `AccountId::NoCheck` | Disable ExpectedBucketOwner checks |

### Multipart Transfers

Files larger than `2 × multipart_part_size` (default 64MB) use S3 multipart upload/download with parallel parts.

### S3 Check Cache

Local SQLite database tracking `(s3_key)` tuples known to exist in S3. Entries expire after 30 days. Avoids redundant `HeadObject` calls (~15ms each).

## Cache Hierarchy

| Cache | Scope | Purpose |
|-------|-------|---------|
| `HashCache` | Local filesystem | Maps (path, mtime, byte range) → hash to skip re-hashing |
| `S3CheckCache` | Per S3 bucket | Tracks which hashes exist in S3 to skip HeadObject calls |
| `AsyncDataCache` | Remote storage | The actual content storage (S3 or filesystem) |

**Upload flow with all caches:**
1. `HashCache` hit → skip reading file, use cached hash
2. `S3CheckCache` hit → skip HeadObject, assume exists
3. `HeadObject` confirms existence → skip upload
4. Otherwise → read, hash, upload

**Download flow with hash cache:**
1. Local file exists with matching mtime in `HashCache`
2. Cached hash matches manifest hash → skip download
3. Otherwise → download from data cache

**Cache sync flow:**
1. `destination.object_exists(hash, alg)` → skip if present
2. Otherwise → `source.get_object` → `destination.put_object`
