# Path Mapping

## Overview

Path mapping transforms filesystem paths between different formats and locations. This
is essential for render farms where jobs are authored on one OS but executed on another,
or where storage mount points differ between submission and execution hosts.

Defined in `path_mapping.rs` (PathFormat, PathMappingRule) and `uri_path.rs` (URI-aware
path operations).

## PathFormat

```rust
pub enum PathFormat {
    Posix,    // Forward slashes, case-sensitive
    Windows,  // Backslashes (normalized to forward), case-insensitive matching
    Uri,      // scheme://authority/path — no normalization
}
```

`PathFormat::host()` returns the format for the current platform (`Posix` on Linux/macOS,
`Windows` on Windows).

The evaluator carries a `PathFormat` that controls how path values are normalized. PATH
values carry their own format and are validated against the evaluator's format. URI paths
bypass normalization entirely.

## PathMappingRule

```rust
pub struct PathMappingRule {
    pub source_path_format: PathFormat,
    pub source_path: String,
    pub destination_path: String,
}
```

### Application

`rule.apply(path)` returns `(matched: bool, result: String)`:

1. Check if `path` starts with `source_path` (using format-appropriate comparison)
2. If matched, replace the `source_path` prefix with `destination_path`
3. Preserve trailing slashes

Format-appropriate comparison means:
- **Posix**: exact byte comparison
- **Windows**: case-insensitive, normalizes backslashes to forward slashes
- **URI**: scheme and authority compared case-insensitively, path compared exactly

### apply_with_format

`rule.apply_with_format(path, output_format)` additionally converts the result path
to the specified output format (e.g., converting Windows backslashes to POSIX forward
slashes).

### Serde

`PathMappingRule` implements `Serialize` and `Deserialize` for JSON interchange:

```json
{
    "source_path_format": "WINDOWS",
    "source_path": "C:/projects",
    "destination_path": "/mnt/projects"
}
```

## URI Path Operations

`uri_path.rs` provides URI-aware path manipulation for paths with `scheme://authority/path`
structure. These are used when `PathFormat::Uri` is detected.

| Function | Purpose |
|----------|---------|
| `is_uri(s)` | Check if string is a URI (contains `://`) |
| `split_uri(s)` | Split into `(scheme://authority, path_portion)` |
| `uri_parts(s)` | Split path into components |
| `uri_name(s)` | Last component (like `Path.name`) |
| `uri_parent(s)` | Parent URI (like `Path.parent`) |
| `uri_suffix(s)` | File extension |
| `uri_suffixes(s)` | All extensions (e.g., `.tar.gz` → `[".tar", ".gz"]`) |
| `uri_stem(s)` | Filename without extension |
| `uri_join(s, parts)` | Join path components |
| `uri_from_parts(parts)` | Reconstruct URI from components |

URI paths are NOT normalized — consecutive slashes, `.`, and `..` are preserved verbatim.
This matches the specification's requirement that URI paths are opaque to the expression
language.

## Expression Language Integration

Path-related operations in the expression language:

### Path Properties (via `__property_NAME__`)

| Property | Type | Description |
|----------|------|-------------|
| `.name` | string | Last component of the path |
| `.stem` | string | Last component without extension |
| `.suffix` | string | File extension (including dot) |
| `.suffixes` | list[string] | All extensions |
| `.parent` | path | Parent directory |
| `.parts` | list[string] | All path components |

### Path Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `with_name(name)` | `(path, string) -> path` | Replace last component |
| `with_stem(stem)` | `(path, string) -> path` | Replace stem, keep extension |
| `with_suffix(suffix)` | `(path, string) -> path` | Replace extension |
| `with_number(n)` | `(path, int) -> path` / `(string, int) -> path` | Append frame number |
| `as_posix()` | `(path) -> string` | Convert to POSIX string |
| `is_absolute()` | `(path) -> bool` | Check if path is absolute |
| `is_relative_to(other)` | `(path, path) -> bool` | Check prefix relationship |
| `relative_to(other)` | `(path, path) -> path` | Compute relative path |

### Path Operators

| Operator | Signature | Description |
|----------|-----------|-------------|
| `/` | `(path, string) -> path` | Join path components |
| `/` | `(path, path) -> path` | Join paths |
| `+` | `(path, string) -> path` | Append to last component |

### apply_path_mapping

`apply_path_mapping(path_string)` is a host-context-only function that applies the
evaluator's path mapping rules to a string, returning a path value. It's unavailable
during template validation (no path mapping rules in that context) and available during
runtime evaluation on a worker host.

## Divergence from Python

The Rust implementation uses the same `PathMappingRule` structure and application logic
as the Python version. The Python version uses `PurePosixPath` / `PureWindowsPath` for
path manipulation; the Rust version uses string operations directly, which avoids the
overhead of constructing path objects for simple prefix matching.

URI path handling is functionally identical between implementations.
