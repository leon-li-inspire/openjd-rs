# HASH_UPLOAD S3-Specific Behavior

[README](README.md) · [HASH_UPLOAD](snapshot_operation_hash_upload.md) · S3 Behavior

## Multipart Upload

Multipart upload is used when uploading to `S3DataCache` and the file/chunk size exceeds `2 × multipart_part_size` (default threshold: 64MB with 32MB parts).

### Multipart Coordination

The `AsyncDataCache` trait provides multipart operations:

```rust
async fn create_multipart_upload(&self, hash: &str, algorithm: &str) -> io::Result<String>;
async fn upload_part(&self, hash: &str, algorithm: &str, upload_id: &str,
                     part_number: i32, data: Vec<u8>) -> io::Result<String>;
async fn complete_multipart_upload(&self, hash: &str, algorithm: &str, upload_id: &str,
                                   parts: Vec<(i32, String)>) -> io::Result<()>;
async fn abort_multipart_upload(&self, hash: &str, algorithm: &str,
                                 upload_id: &str) -> io::Result<()>;
```

**Coordination flow:**
1. `create_multipart_upload` → get `upload_id`
2. `tokio::spawn` each part upload independently
3. Each part returns `(part_number, etag)` on success
4. Last part to complete calls `complete_multipart_upload`
5. On any failure, `abort_multipart_upload`

## Streaming Files (Larger Than Memory)

When file chunking is disabled (`WHOLE_FILE_CHUNK_SIZE`) and a file exceeds `max_memory_bytes`, a two-pass approach is used:

```
┌─────────────────────────────────────────────────────────────────┐
│  Pass 1: Compute Hash (discard data)                            │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Read part 1 → hash (full + part) → discard             │    │
│  │  Read part 2 → hash (full + part) → discard             │    │
│  │  ...                                                    │    │
│  │  Result: final_hash + per-part hashes                   │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  HeadObject check with final_hash                               │
│  └─► If exists, skip upload (done)                              │
│                                                                 │
│  Pass 2: Read, Verify, and Upload Parts (memory throttled)      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Create S3 multipart upload session                     │    │
│  │  For each part:                                         │    │
│  │    1. memory_pool.acquire(part_size).await               │    │
│  │    2. spawn_blocking: read part + compute part hash      │    │
│  │    3. Verify part hash matches pass 1                    │    │
│  │       └─► If mismatch: abort, return error               │    │
│  │    4. Upload part (async)                                │    │
│  │    5. Drop permit (releases memory)                      │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

**Why two passes?**
1. S3 object keys are based on content hash — must know hash before creating multipart session
2. Discarding data in pass 1 avoids buffering the entire file
3. If object already exists (HeadObject hit), skip pass 2 entirely

**Hash verification:** Per-part hashes computed in pass 1 are verified in pass 2. If any part's hash differs (file modified between passes), the multipart upload is aborted and an error is returned.

## Probabilistic S3 Cache Validation

The S3 check cache can become stale if objects are deleted from S3. HASH_UPLOAD performs inline validation:

```
For each item where S3 check cache says "exists":

  Item 1-100:     Always verify with HeadObject
  Item 101+:      1% random sampling

  If ANY verification fails:
  1. Mark cache as invalid
  2. Delete S3 check cache database
  3. Re-queue all previously skipped items
  4. Continue without cache
```

### Sampling Strategy

| Item Number | Verification | Rationale |
|-------------|--------------|-----------|
| 1–100 | Always HeadObject | Catch stale cache early |
| 101+ | 1% random sample | Balance coverage vs. performance |

### Expected HeadObject Overhead (Warm Cache)

| Total Items | Verified | Time (8 workers) |
|-------------|----------|------------------|
| 100 | 100 | ~0.2s |
| 1,000 | 109 | ~0.2s |
| 10,000 | 199 | ~0.4s |
| 100,000 | 1,099 | ~2.1s |

### Recovery Flow

1. **Detect:** HeadObject returns 404 for cached item
2. **Invalidate:** Delete S3 check cache database
3. **Re-queue:** Collect all items skipped due to cache
4. **Retry:** Re-submit skipped items (now use HeadObject directly)
5. **Continue:** Process remaining items without cache

## FileSystem Data Cache

For `FileSystemDataCache`, behavior is simpler:
- No multipart uploads (single-file writes)
- Existence check via filesystem `exists()`
- Streaming files use two-pass: hash first, then copy while re-verifying hash
