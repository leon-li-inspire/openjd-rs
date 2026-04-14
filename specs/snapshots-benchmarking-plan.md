# Benchmarking Plan for openjd-snapshots

This guide covers how to benchmark the three performance-critical operations in
`openjd-snapshots`: **HASH**, **HASH_UPLOAD**, and **DOWNLOAD**. It includes
dataset design, baseline comparisons against external tools like `s5cmd`, and
methodology for producing reproducible, meaningful results.

## Table of Contents

- [Quick Start](#quick-start)
- [Operations Under Test](#operations-under-test)
- [The Built-in Benchmark Tool](#the-built-in-benchmark-tool)
- [Dataset Design](#dataset-design)
- [Filesystem Benchmarks](#filesystem-benchmarks)
- [S3 Benchmarks](#s3-benchmarks)
- [Baseline Comparisons](#baseline-comparisons)
- [Interpreting Results](#interpreting-results)
- [Reproducing Results](#reproducing-results)
- [Benchmark Results](#benchmark-results)

---

## Quick Start

```bash
# Build the benchmark binary
cd crates/openjd-snapshots
cargo build --release --features bench --bin openjd-snapshots-bench

# Run a small filesystem-only benchmark
cargo run --release --features bench --bin openjd-snapshots-bench -- --preset tiny --max-workers 1,4,8

# Run with an existing dataset instead of generating one
cargo run --release --features bench --bin openjd-snapshots-bench -- --source-dir /path/to/data --max-workers 4
```

---

## Operations Under Test

### HASH (CPU-bound)

Reads files from disk and computes xxh128 hashes. Uses **rayon** thread pool
for parallelism. The bottleneck is either disk I/O (for cold reads) or CPU
(for warm page cache). This operation is the foundation — both HASH_UPLOAD and
DOWNLOAD depend on hashing performance.

Key parameters:
- `max_workers` — rayon thread count (default: CPU count)
- `file_chunk_size_bytes` — files larger than this are split into independently
  hashed chunks (default: 256 MB, use `-1` for whole-file mode)
- `hash_cache` — SQLite cache that skips re-hashing unchanged files

### HASH_UPLOAD (mixed CPU + I/O)

Hashes files then uploads them to a content-addressed data cache. Uses a
**tokio** async runtime with semaphore-bounded concurrency. Three code paths
depending on file size:

| File Size | Path | Behavior |
|-----------|------|----------|
| < 64 MB | Small | `spawn_blocking(read+hash)` → async `put_object` |
| ≥ 64 MB | Large | Streaming hash in 32 MB buffers → concurrent multipart upload |
| > chunk_size | Chunked | Read all chunks → hash each → upload each with dedup |

Key parameters:
- `max_workers` — tokio semaphore permits controlling concurrency (default: 10)
- `max_memory_bytes` — memory pool bound (default: `min(16GB, max(256MB, ram/4, available-1GB))`)
- `file_chunk_size_bytes` — chunking threshold

### DOWNLOAD (I/O-bound)

Downloads files from a data cache to local disk. Same tokio-based pipeline as
upload, with parallel byte-range GETs for large files and `posix_fallocate` for
sparse file pre-allocation on Linux.

Key parameters:
- `max_workers` — concurrency limit
- `max_memory_bytes` — memory pool bound
- `hash_cache` — enables skip-on-match for files already present on disk
- `file_conflict_resolution` — Skip / Overwrite / CreateCopy

---

## The Built-in Benchmark Tool

The `openjd-snapshots-bench` binary (`src/bin/bench.rs`) exercises the full pipeline:
COLLECT → HASH_UPLOAD (cold) → HASH_UPLOAD (warm) → DOWNLOAD (cold) →
DOWNLOAD (warm) → DIFF.

### Presets

| Preset | Small Files (1 KB) | Medium Files (50 MB) | Large Files (1 GB) | Approx Total |
|--------|-------------------|---------------------|-------------------|-------------|
| `tiny` | 400 | 40 | 2 | ~4 GB |
| `small` | 1,500 | 400 | 5 | ~25 GB |
| `medium` | 20,000 | 1,000 | 10 | ~60 GB |
| `large` | 1,000,000 | 10,000 | 10 | ~510 GB |

### Key Flags

```
--preset <name>          Use a named preset (tiny/small/medium/large)
--max-workers <list>     Comma-separated worker counts for scaling tests (e.g. "1,2,4,8,16")
--max-memory <MB>        Memory limit in MB
--chunk-size <MB>        File chunk size in MB (default: 256)
--no-chunking            Disable chunking (whole-file mode)
--no-hash-cache          Disable hash cache (forces rehash every time)
--source-dir <path>      Use existing directory instead of generating test data
--keep-files             Keep generated test files after run
--skip-download          Skip download and diff tests
```

### Output

The tool prints per-operation throughput (MB/s or paths/s) and, when multiple
worker counts are specified, a markdown scaling table:

```
| Operation        | Workers=1 | Workers=4 | Workers=8 | Workers=16 |
|------------------|-----------|-----------|-----------|------------|
| HASH_UPLOAD cold | 12.3s     | 4.1s      | 2.8s      | 2.7s       |
| HASH_UPLOAD warm | 8.2s      | 2.1s      | 1.1s      | 1.0s       |
| DOWNLOAD cold    | 15.1s     | 5.2s      | 3.1s      | 2.9s       |
```

---

## Dataset Design

The choice of dataset dramatically affects which bottleneck you measure. A
benchmark that only uses large files will never stress metadata handling; one
that only uses small files will never exercise multipart transfer.

### File Size Tiers

| Tier | Size | What It Stresses | Real-World Analog |
|------|------|-----------------|-------------------|
| Tiny | 1 KB | Per-file overhead, directory walking, S3 request latency | Config files, scripts, shader includes |
| Small | 100 KB–1 MB | Hash throughput vs. I/O setup cost | Source code, small textures |
| Medium | 10–100 MB | Balanced CPU + I/O | Compiled assets, medium textures |
| Large | 500 MB–2 GB | Multipart transfer, memory management, streaming | EXR sequences, simulation caches |
| Huge | 5+ GB | Memory pool backpressure, single-file throughput ceiling | Alembic caches, large scene files |

### Directory Structure Dimensions

| Dimension | Low | High | Effect |
|-----------|-----|------|--------|
| File count | 10 | 1,000,000 | Tests manifest building, memory for file entries |
| Directory depth | 1 | 20 | Tests path normalization, directory creation on download |
| Directory breadth | 1 | 10,000 | Tests parallel directory walking |
| Duplicate content | 0% | 50% | Tests content-addressed dedup effectiveness |

### Recommended Test Matrices

**For HASH benchmarks** (CPU-bound, no network):

| Test Name | Files | Sizes | Purpose |
|-----------|-------|-------|---------|
| `hash-many-tiny` | 100,000 | 1 KB each | Per-file overhead, ~100 MB total |
| `hash-few-large` | 10 | 1 GB each | Sustained throughput, ~10 GB total |
| `hash-mixed` | 20,000 small + 40 medium + 2 large | Mixed | Realistic workload, ~4 GB total |
| `hash-chunked` | 10 | 1 GB, chunk_size=64MB | Chunk hashing overhead |

**For HASH_UPLOAD benchmarks** (add data cache dimension):

| Test Name | Files | Cache | Purpose |
|-----------|-------|-------|---------|
| `upload-cold-fs` | Mixed preset | FileSystem | Baseline: hash + write to local cache |
| `upload-cold-s3` | Mixed preset | S3 | Network-bound: hash + S3 PutObject |
| `upload-warm` | Same as cold | Either | Cache effectiveness: should skip everything |
| `upload-dedup` | 1000 files, 50% identical | Either | Content-addressed dedup savings |

**For DOWNLOAD benchmarks**:

| Test Name | Files | Cache | Purpose |
|-----------|-------|-------|---------|
| `download-cold-fs` | Mixed preset | FileSystem | Baseline: read from local cache + write |
| `download-cold-s3` | Mixed preset | S3 | Network-bound: S3 GetObject + write |
| `download-warm` | Same as cold | Either | Hash cache skip effectiveness |
| `download-large-multipart` | 10 × 1 GB | S3 | Multipart download parallelism |

---

## Filesystem Benchmarks

Filesystem benchmarks isolate hashing and I/O performance from network effects.
Use `FileSystemDataCache` (the default in the benchmark tool).

### Running

```bash
# Hash-only scaling test
cargo run --release --features bench --bin openjd-snapshots-bench -- \
  --preset small --skip-download --max-workers 1,2,4,8,16

# Full pipeline with custom dataset
cargo run --release --features bench --bin openjd-snapshots-bench -- \
  --small-files 50000 --medium-files 100 --large-files 5 \
  --max-workers 8 --chunk-size 64

# Whole-file mode (no chunking)
cargo run --release --features bench --bin openjd-snapshots-bench -- \
  --preset small --no-chunking --max-workers 8

# Without hash cache (measure raw hash throughput)
cargo run --release --features bench --bin openjd-snapshots-bench -- \
  --preset small --no-hash-cache --max-workers 8
```

### What to Measure

1. **Cold hash throughput** (MB/s) — first run, no caches
2. **Warm hash throughput** — second run with hash cache populated
3. **Worker scaling** — throughput at 1, 2, 4, 8, 16 workers
4. **Chunk size sensitivity** — compare 64 MB, 128 MB, 256 MB, whole-file
5. **Memory limit sensitivity** — compare 256 MB, 1 GB, 4 GB limits

---

## S3 Benchmarks

S3 benchmarks measure end-to-end transfer performance including network
latency, connection pooling, and multipart parallelism.

### Prerequisites

```bash
# AWS credentials must be configured
aws sts get-caller-identity

# Create a test bucket (or use an existing one)
export BENCH_BUCKET=my-benchmark-bucket
export BENCH_REGION=us-west-2
aws s3 mb s3://$BENCH_BUCKET --region $BENCH_REGION
```

### Running S3 Benchmarks

The built-in benchmark tool currently uses `FileSystemDataCache`. To benchmark
against S3, you need to either:

1. **Extend the benchmark tool** to accept `--s3-bucket` and `--s3-prefix` flags
   and construct an `S3DataCache` instead of `FileSystemDataCache`, or
2. **Write a custom benchmark script** that calls the library API directly.

Example of what an S3 benchmark invocation would look like:

```bash
# Proposed extension to the benchmark tool
cargo run --release --features bench --bin openjd-snapshots-bench -- \
  --preset small \
  --s3-bucket $BENCH_BUCKET \
  --s3-prefix bench/$(date +%Y%m%d-%H%M%S)/ \
  --max-workers 1,4,8,16 \
  --region $BENCH_REGION
```

### S3 Performance Factors

| Factor | Impact | How to Control |
|--------|--------|---------------|
| Instance type | Network bandwidth ceiling | Use instances with ≥25 Gbps (e.g. c5n.4xlarge) |
| Same-region | Eliminates cross-region latency | Bucket and instance in same region |
| Connection count | Each S3 connection caps at ~5 Gbps | Increase `max_workers` |
| Multipart part size | Affects parallelism per large file | `multipart_part_size` on S3DataCache (default 32 MB) |
| S3 request rate | S3 supports 5,500 GET/s and 3,500 PUT/s per prefix | Use randomized key prefixes |
| S3 check cache | Avoids HeadObject calls on warm runs | Enabled by default |

### Instance Selection for S3 Benchmarks

To avoid the EC2 instance being the bottleneck:

| Workload | Recommended Instance | Network | Notes |
|----------|---------------------|---------|-------|
| Small datasets (<10 GB) | c5.2xlarge | Up to 10 Gbps | Sufficient for most tests |
| Medium datasets (10-100 GB) | c5n.4xlarge | Up to 25 Gbps | Good balance |
| Large datasets (100+ GB) | c5n.18xlarge | 100 Gbps | Saturate S3 throughput |

---

## Baseline Comparisons

Comparing against established tools provides context for whether the library's
performance is competitive. The key baselines are **s5cmd** (fastest general
S3 tool) and **aws s3 cp** (standard reference).

### Why These Baselines

- **s5cmd** — Written in Go, uses parallel workers (default 256) with per-file
  concurrency for multipart transfers. Widely cited as the fastest S3 CLI tool,
  reported at up to 12x faster than aws-cli for uploads and capable of
  saturating 40 Gbps links on downloads.
- **aws s3 cp** — The standard AWS CLI. Slower but universally available.
  Provides the "expected" baseline that users compare against.

### Important: What We're Comparing

`openjd-snapshots` does more work than raw S3 transfer tools:
1. **Content hashing** — xxh128 hash of every byte (s5cmd doesn't hash)
2. **Content-addressed dedup** — identical content stored once (s5cmd copies everything)
3. **Manifest building** — structured metadata about every file
4. **Hash cache** — skip unchanged files on subsequent runs

A fair comparison acknowledges this. The goal is not to beat s5cmd at raw
transfer speed, but to show that the overhead of hashing + dedup + manifest
management is acceptable, and that the library's transfer layer is competitive
when the hash is already computed.

### Installing Baselines

```bash
# s5cmd (Go binary, no dependencies)
curl -sL https://github.com/peak/s5cmd/releases/latest/download/s5cmd_2.3.0_linux_amd64.tar.gz \
  | tar xz -C /usr/local/bin/ s5cmd

# Verify
s5cmd version
aws --version
```

### Running Baseline Benchmarks

#### s5cmd Upload Baseline

```bash
# Generate test data (use the benchmark tool with --keep-files)
cargo run --release --features bench --bin openjd-snapshots-bench -- --preset small --keep-files --skip-download
# Note the source directory path from output

SOURCE_DIR=/tmp/bench-XXXXX  # from bench output
PREFIX=baseline/$(date +%Y%m%d-%H%M%S)

# s5cmd upload (default 256 workers)
time s5cmd cp "$SOURCE_DIR/*" "s3://$BENCH_BUCKET/$PREFIX/"

# s5cmd upload with controlled concurrency
time s5cmd --numworkers 8 cp --concurrency 10 "$SOURCE_DIR/*" "s3://$BENCH_BUCKET/$PREFIX-w8/"

# s5cmd upload with concurrency matching our benchmark
time s5cmd --numworkers 16 cp --concurrency 5 "$SOURCE_DIR/*" "s3://$BENCH_BUCKET/$PREFIX-w16/"
```

#### s5cmd Download Baseline

```bash
DEST_DIR=$(mktemp -d)

# s5cmd download
time s5cmd cp "s3://$BENCH_BUCKET/$PREFIX/*" "$DEST_DIR/"

# With controlled concurrency
time s5cmd --numworkers 8 cp --concurrency 10 "s3://$BENCH_BUCKET/$PREFIX/*" "$DEST_DIR-w8/"
```

#### aws s3 cp Baseline

```bash
# Upload
time aws s3 cp "$SOURCE_DIR" "s3://$BENCH_BUCKET/$PREFIX-awscli/" --recursive

# Download
time aws s3 cp "s3://$BENCH_BUCKET/$PREFIX-awscli/" "$DEST_DIR-awscli/" --recursive
```

### Comparison Methodology

Run each tool 3 times and take the median. For each run:

1. **Drop page cache** before cold runs: `sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'`
2. **Use the same dataset** generated by the benchmark tool with `--keep-files`
3. **Use the same S3 bucket and region**
4. **Record**: wall time, data size, file count, throughput (MB/s)

### Comparison Table Template

```markdown
| Tool | Operation | Files | Total Size | Workers | Time | Throughput |
|------|-----------|-------|-----------|---------|------|------------|
| openjd-snapshots | HASH_UPLOAD cold | 1,945 | 25 GB | 8 | | |
| openjd-snapshots | HASH_UPLOAD warm | 1,945 | 25 GB | 8 | | |
| openjd-snapshots | DOWNLOAD cold | 1,945 | 25 GB | 8 | | |
| s5cmd | cp (upload) | 1,945 | 25 GB | 8 | | |
| s5cmd | cp (download) | 1,945 | 25 GB | 8 | | |
| aws s3 cp | --recursive (up) | 1,945 | 25 GB | default | | |
| aws s3 cp | --recursive (down) | 1,945 | 25 GB | default | | |
```

### What to Look For

- **Cold upload**: openjd-snapshots will be slower than s5cmd because it hashes
  every file. The question is how much slower — if hashing adds <20% overhead
  to transfer time, the dedup benefits likely outweigh the cost.
- **Warm upload**: openjd-snapshots should be dramatically faster than s5cmd
  because it skips unchanged files via hash cache + S3 check cache. s5cmd has
  no equivalent — it must re-transfer or use `sync` with mtime comparison.
- **Cold download**: Should be competitive with s5cmd since both use parallel
  connections and multipart for large files.
- **Warm download**: openjd-snapshots with hash cache should skip files already
  on disk. s5cmd has no content-aware skip mechanism.
- **Many small files**: Per-file overhead matters. s5cmd's 256 default workers
  may outperform at high file counts. Test with `--numworkers` matching our
  `max_workers` for a fair comparison.
- **Few large files**: Multipart parallelism matters. Compare `--concurrency`
  (s5cmd's per-file part parallelism) against our `multipart_part_size` tuning.

---

## Interpreting Results

### Throughput Ceilings

Know your hardware limits to understand whether you're bottlenecked on CPU,
disk, or network:

| Resource | How to Measure | Typical Values |
|----------|---------------|----------------|
| Disk sequential read | `fio --name=read --rw=read --bs=1M --size=1G --direct=1` | NVMe: 3+ GB/s, EBS gp3: 1 GB/s |
| Disk sequential write | `fio --name=write --rw=write --bs=1M --size=1G --direct=1` | NVMe: 2+ GB/s, EBS gp3: 1 GB/s |
| Network to S3 | `iperf3` or observe during large transfer | 10-100 Gbps depending on instance |
| xxh128 hash rate | Single-core: ~10 GB/s, multi-core scales linearly | CPU-bound ceiling |
| S3 single connection | ~5 Gbps (~625 MB/s) max per connection | Need multiple connections |

### Scaling Analysis

Plot throughput vs. worker count. Look for:

- **Linear scaling region** — adding workers proportionally increases throughput
- **Plateau** — hitting a hardware ceiling (disk, network, or CPU)
- **Degradation** — too many workers cause contention (lock contention, memory
  pressure, S3 throttling)

The optimal worker count is at the knee of the curve, just before the plateau.

### Overhead Analysis

To isolate the overhead of hashing vs. pure transfer:

```
hash_overhead = (hash_upload_time - equivalent_s5cmd_time) / equivalent_s5cmd_time
```

If `hash_overhead` is consistently <20% across dataset sizes, the library's
transfer layer is competitive and the hashing cost is acceptable given the
dedup and integrity benefits.

---

## Reproducing Results

### Environment Checklist

- [ ] Instance type and region documented
- [ ] EBS volume type and IOPS documented (or local NVMe)
- [ ] `cargo build --release` (never benchmark debug builds)
- [ ] Page cache dropped before cold runs
- [ ] No other significant workloads running
- [ ] S3 bucket in same region as instance
- [ ] AWS SDK connection pool warmed (first run may be slower due to DNS/TLS)

### Reporting Template

When reporting benchmark results, include:

```
## Environment
- Instance: c5n.4xlarge (16 vCPU, 42 GB RAM, 25 Gbps network)
- Storage: 500 GB gp3 (3000 IOPS, 125 MB/s baseline)
- Region: us-west-2
- openjd-snapshots: commit <hash>
- s5cmd: v2.3.0
- aws-cli: v2.x.x

## Dataset
- Generated by: `cargo run --release --features bench --bin openjd-snapshots-bench -- --preset small --keep-files`
- Small files: 1,500 × 1 KB
- Medium files: 400 × 50 MB
- Large files: 5 × 1 GB
- Total: 1,905 files, ~25 GB

## Results
<table>
```

### Automation

For CI or repeated benchmarking, wrap the comparison in a script:

```bash
#!/bin/bash
set -euo pipefail

BUCKET=${BENCH_BUCKET:?Set BENCH_BUCKET}
PRESET=${1:-small}
WORKERS=${2:-"1,4,8,16"}

# Run openjd-snapshots benchmark
cargo run --release --features bench --bin openjd-snapshots-bench -- \
  --preset "$PRESET" \
  --max-workers "$WORKERS" \
  --keep-files 2>&1 | tee results-snapshots.txt

# Extract source dir from output
SOURCE_DIR=$(grep "Source directory:" results-snapshots.txt | awk '{print $NF}')

# Run s5cmd baseline at each worker count
for w in $(echo "$WORKERS" | tr ',' ' '); do
  PREFIX="bench-$(date +%s)-w$w"
  echo "=== s5cmd upload (workers=$w) ==="
  time s5cmd --numworkers "$w" cp "$SOURCE_DIR/*" "s3://$BUCKET/$PREFIX/" 2>&1

  DEST=$(mktemp -d)
  echo "=== s5cmd download (workers=$w) ==="
  time s5cmd --numworkers "$w" cp "s3://$BUCKET/$PREFIX/*" "$DEST/" 2>&1
  rm -rf "$DEST"

  # Cleanup S3
  s5cmd rm "s3://$BUCKET/$PREFIX/*"
done
```


---

## Benchmark Results

### HASH: Python vs Rust (2026-04-03)

**Environment**: 8 vCPU Intel Xeon Platinum 8175M @ 2.50GHz (c5.2xlarge equivalent)
**Dataset**: 442 files, 4,048 MB (400 × 1 KB, 40 × 50 MB, 2 × 1 GB)
**Chunk size**: 256 MB | **Hash algorithm**: xxh128 | **Page cache**: warm

| Implementation | Median Time | Throughput | Speedup |
|----------------|-------------|------------|---------|
| Python         | 3.063s      | 1,322 MB/s | 1.0×   |
| Rust           | 0.570s      | 7,108 MB/s | 5.4×   |

**Analysis**:
- The Python HASH implementation is single-threaded (sequential file iteration,
  no `concurrent.futures`). The Rust implementation uses a rayon thread pool
  across all 8 cores.
- Python's per-core throughput (~1.3 GB/s) is respectable — the xxhash C
  extension does the heavy lifting. The 5.4× gap is almost entirely due to
  parallelism, not per-byte hash speed.
- Rust's first cold-cache run was ~4.5s; subsequent warm runs were ~0.57s.
  Python was consistently ~3s because it was CPU-bound (page cache was warm).
- On workloads with many small files, the gap would widen further due to
  per-file overhead and Python's GIL limiting thread-based parallelism.

### HASH_UPLOAD (filesystem): Python vs Rust (2026-04-03)

**Environment**: 8 vCPU Intel Xeon Platinum 8175M @ 2.50GHz (c5.2xlarge equivalent)
**Dataset**: 442 files, 4,048 MB (400 × 1 KB, 40 × 50 MB, 2 × 1 GB)
**Chunk size**: 256 MB | **max_workers**: 10 | **Data cache**: FileSystemDataCache

#### Cold (hash cache writes enabled, force_rehash=True)

| Implementation | Median Time | Throughput | Speedup |
|----------------|-------------|------------|---------|
| Python         | 22.877s     | 177 MB/s   | 1.0×   |
| Rust           | 23.08s      | 175 MB/s   | 1.0×   |

#### Warm (hash cache + data cache populated)

| Implementation | Median Time | Throughput |
|----------------|-------------|------------|
| Python         | 0.075s      | 53,854 MB/s |
| Rust           | 0.01s       | 311,627 MB/s |

**Analysis**:
- **Cold HASH_UPLOAD**: Python and Rust are essentially tied (~175-178 MB/s).
  Both are bottlenecked on filesystem data cache writes. The Rust hashing
  advantage (5.4× from the HASH benchmark) is completely masked by I/O — the
  pipeline spends most of its time writing 4 GB of content to the data cache.
  Hash cache SQLite writes add overhead to both implementations equally.
- **Warm HASH_UPLOAD**: Both are extremely fast since they skip all I/O via
  hash cache lookups. Rust is ~7× faster (0.01s vs 0.075s) on the warm path,
  likely due to lower per-query overhead in the SQLite hash cache lookups.
- Note: running with `--no-hash-cache` disables the warm skip path entirely,
  causing the warm pass to re-hash all files (~2s in Rust). This is expected
  behavior, not a bug — without a hash cache, the only skip mechanism is the
  data cache `object_exists` check, which still requires re-reading and
  re-hashing each file to determine its content hash.

### DOWNLOAD (filesystem): Python vs Rust (2026-04-03)

**Environment**: 8 vCPU Intel Xeon Platinum 8175M @ 2.50GHz (c5.2xlarge equivalent)
**Dataset**: 442 files, 4,048 MB (400 × 1 KB, 40 × 50 MB, 2 × 1 GB)
**Chunk size**: 256 MB | **max_workers**: 10 | **Data cache**: FileSystemDataCache

#### Cold (empty download dir, hash cache from upload)

| Implementation | Median Time | Throughput | Speedup |
|----------------|-------------|------------|---------|
| Python         | 22.41s      | 181 MB/s   | 1.5×   |
| Rust           | 33.25s      | 122 MB/s   | 1.0×   |

#### Warm (files exist + hash cache populated)

| Implementation | Median Time |
|----------------|-------------|
| Python         | 0.308s      |
| Rust           | 0.06s       |

**Analysis**:
- **Cold DOWNLOAD**: Python is ~1.5× faster than Rust on cold downloads. Root
  cause analysis identified three issues in the Rust download path:

  1. **No zero-copy for filesystem cache**: Python uses `shutil.copy2()` which
     maps to `sendfile()` on Linux — a kernel-level file-to-file copy that never
     touches user space. Rust reads the entire file into a `Vec<u8>` via
     `get_object()`, then writes it back out via `std::fs::write()`. For a 50 MB
     file, this means 50 MB allocated, 50 MB read into user space, 50 MB written
     back — 3× the memory traffic of `sendfile()`.

  2. **Chunked files concatenate in memory**: For chunked files (the 2 × 1 GB
     files with 256 MB chunks), the code reads each chunk via `get_object()` and
     calls `combined.extend_from_slice()` to build a single `Vec<u8>`, then
     writes the entire file at once. This allocates up to 1 GB of contiguous
     memory per file. Python writes each chunk directly to the output file at
     the correct offset, never holding more than one chunk in memory.

  3. **Multipart download path unused for chunked files**: The efficient
     `download_multipart_to_file()` (which writes parts at offsets) only
     activates for files with a single `hash` ≥ 64 MB. Chunked files (which
     have `chunk_hashes` instead of `hash`) always take the concatenation path,
     even though they could use the same offset-write strategy.

  **Recommended fixes** (implemented in this commit):
  - Added `copy_object_to_file` method to `AsyncDataCache` — uses `std::fs::copy()`
    (sendfile on Linux) for filesystem, falls back to get+write for S3.
  - Added `write_object_to_file_at_offset` for chunked downloads — writes each
    chunk directly at its offset via `std::io::copy` instead of concatenating.
  - Non-chunked single files now use `copy_object_to_file` instead of get+write.
  - Post-fix benchmarking showed these eliminate unnecessary memory allocations
    but don't significantly change throughput on this instance (~140 MB/s raw
    disk write ceiling). The remaining gap is disk I/O scheduling — Python's
    `shutil.copy2` benefits from efficient page cache buffering. On faster
    storage (NVMe, ramdisk), the zero-copy improvements would show larger gains.
- **Warm DOWNLOAD**: Rust is ~5× faster (0.06s vs 0.31s). The warm path checks
  the hash cache to confirm files on disk already match, skipping all I/O. Rust's
  lower per-file overhead in the hash cache lookup loop gives it the advantage.
- High variance in cold runs (both implementations) is expected — the workload
  writes 4 GB to disk, and I/O scheduling varies between runs. First cold runs
  are consistently slower due to page cache effects from the preceding upload.
- The cold download throughput (~122-181 MB/s) is comparable to HASH_UPLOAD cold
  throughput (~175 MB/s), confirming both are disk-I/O-bound at similar rates.

### HASH_UPLOAD (S3): Python vs Rust (2026-04-04)

**Environment**: 8 vCPU Intel Xeon Platinum 8175M @ 2.50GHz, us-west-2
**Dataset**: 442 files, 4,048 MB (400 × 1 KB, 40 × 50 MB, 2 × 1 GB)
**Chunk size**: 256 MB | **max_workers**: 10 | **S3 bucket**: same region

#### Cold (fresh S3 prefix, force_rehash=True)

| Implementation | Before fix | After fix | Python |
|----------------|-----------|-----------|--------|
| Rust           | 28.3s (143 MB/s) | 14.8s (275 MB/s) | — |
| Python         | — | — | 11.1s (366 MB/s) |

Parallelizing chunk uploads improved Rust cold S3 upload by **2×**.
Remaining gap vs Python: **~1.3×**.

#### Warm (hash cache + S3 check cache populated)

| Implementation | Time |
|----------------|------|
| Python         | 0.29s |
| Rust           | 5.4s  |

Python is **~19× faster** on warm S3 skip path.

**Root cause analysis**:
- **Cold upload (fixed)**: The `process_chunked_async` function uploaded chunks
  sequentially — each chunk's `object_exists` + `put_object` was awaited before
  starting the next. For a 1 GB file with 4 × 256 MB chunks, that was 4
  sequential S3 round trips. Fix: spawn all chunk uploads as parallel tokio
  tasks. This brought Rust from 28s → 15s.
- **Cold upload (remaining gap)**: Python uploads 448 chunks vs Rust's 442,
  suggesting Python chunks the large files into more S3 objects. The remaining
  ~1.3× gap may be from: (a) Rust's `object_exists` HeadObject before each
  PutObject adding latency, (b) memory pool acquiring `file_size` bytes per
  file serializing large file processing, (c) tokio async overhead vs Python's
  simpler ThreadPoolExecutor.
- **Warm upload**: Rust takes 5.4s because it has no S3 check cache configured
  in the benchmark — every "skip" check does a HeadObject to S3 (442 calls ×
  ~12ms = ~5.3s). Python uses a local S3CheckCache (SQLite) that resolves
  instantly. This is a benchmark configuration gap, not a code bug — the Rust
  `S3DataCache` supports `s3_check_cache` but it wasn't wired up in the bench.

### DOWNLOAD (S3): Python vs Rust (2026-04-04)

**Environment**: 8 vCPU Intel Xeon Platinum 8175M @ 2.50GHz, us-west-2
**Dataset**: 442 files, 4,048 MB (400 × 1 KB, 40 × 50 MB, 2 × 1 GB)
**Chunk size**: 256 MB | **max_workers**: 10 | **S3 bucket**: same region

#### Cold (fresh download directory)

| Implementation | Best | Median | Throughput range |
|----------------|------|--------|-----------------|
| Python         | 15.8s (257 MB/s) | 32.1s (126 MB/s) | 122–257 MB/s |
| Rust           | 18.6s (218 MB/s) | 30.9s (131 MB/s) | 118–218 MB/s |

Effectively **at parity** — both show high variance due to S3 and disk
scheduling. Best runs are within 15% of each other.

#### Warm (files exist + hash cache populated)

| Implementation | Median |
|----------------|--------|
| Python         | 0.355s |
| Rust           | 0.033s |

Rust is **~10× faster** on the warm skip path.

**Analysis**:
- **Cold S3 download**: Python and Rust are at parity. Both are bottlenecked
  on the combination of S3 download bandwidth and local disk write speed.
  The high variance (best run ~2× faster than worst) is typical for S3
  workloads — first run benefits from S3's internal caching, subsequent runs
  may hit different S3 partitions.
- **Warm download**: Rust is 10× faster (0.033s vs 0.355s). Both use a local
  hash cache to skip files already on disk. Rust's lower per-file overhead
  in the hash cache lookup loop gives it a significant advantage.
- The parallel chunk download fix from earlier is working well here — the
  2 × 1 GB chunked files download their 4 chunks in parallel from S3,
  which is exactly the use case chunking was designed for.

### s5cmd Baseline (2026-04-04)

**Environment**: 8 vCPU Intel Xeon Platinum 8175M @ 2.50GHz, us-west-2
**Dataset**: 442 files, 4,048 MB (400 × 1 KB, 40 × 50 MB, 2 × 1 GB)
**s5cmd version**: v2.3.0

| Operation | Workers | Time | Throughput |
|-----------|---------|------|------------|
| Upload | 256 (default) | 5.2s | 778 MB/s |
| Upload | 64 (concurrency 10) | 7.4s | 547 MB/s |
| Upload | 10 | 7.3s | 554 MB/s |
| Download | 256 (default) | 11.2s | 361 MB/s |
| Download | 64 (concurrency 10) | 10.6s | 382 MB/s |
| Download | 10 | 11.0s | 368 MB/s |

**Comparison with openjd-snapshots** (at 10 workers):

| Tool | S3 Upload | S3 Download | Notes |
|------|-----------|-------------|-------|
| s5cmd | 7.3s (554 MB/s) | 11.0s (368 MB/s) | Raw transfer, no hashing |
| Python snapshots | 11.1s (366 MB/s) | 16–33s (126–257 MB/s) | Hash + upload + HeadObject |
| Rust snapshots | 14.8s (275 MB/s) | 19–33s (131–218 MB/s) | Hash + upload + HeadObject |

**Analysis**:
- s5cmd is the throughput ceiling for raw S3 transfer on this instance. At 256
  workers it achieves 778 MB/s upload — but even at 10 workers it gets 554 MB/s.
- **Upload overhead**: openjd-snapshots adds ~50-100% overhead vs raw s5cmd
  transfer due to: (a) xxh128 hashing of every byte, (b) HeadObject before
  PutObject to check existence, (c) content-addressed key computation. Python's
  366 MB/s vs s5cmd's 554 MB/s = 34% overhead, which is excellent given the
  extra work. Rust's 275 MB/s = 50% overhead, with room to improve.
- **Download overhead**: openjd-snapshots downloads are closer to s5cmd because
  downloads don't need hashing or existence checks. The gap is mainly from
  content-addressed key indirection and hash cache writes.
- s5cmd's default 256 workers significantly outperforms 10 workers on upload
  (778 vs 554 MB/s), suggesting openjd-snapshots could benefit from higher
  default concurrency for S3 operations.
