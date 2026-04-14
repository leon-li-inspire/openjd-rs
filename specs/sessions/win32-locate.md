# Windows Executable Location

## Purpose

On Windows, subprocess commands specified in OpenJD templates may use bare
executable names (e.g., `python`) that need to be resolved to full paths before
launching. This module mirrors the Python implementation's
`_win32/_locate_executable.py` to resolve executables using the working directory
and PATH environment variable.

## Status

This module is not yet integrated into the subprocess launch path. It is marked
`#[allow(dead_code)]` pending full Windows cross-user support.

## Function

```rust
pub fn locate_windows_executable(
    args: &[String],
    user: Option<&dyn SessionUser>,
    os_env_vars: Option<&HashMap<String, Option<String>>>,
    working_dir: &str,
) -> Vec<String>
```

Returns a copy of `args` with `args[0]` resolved to an absolute path if possible.
If resolution fails, returns `args` unchanged so the OS can produce its own error.

## Resolution rules

1. **Absolute paths** — returned as-is. The OS handles extension resolution
   (e.g., `C:\Python\python` → `C:\Python\python.exe`).

2. **Relative names** — resolved via `which::which_in` using a search path
   constructed as `{working_dir};{PATH}`. The working directory is prepended
   so that executables in the session working directory take precedence.

3. **PATH lookup** — case-insensitive key search in `os_env_vars` for the
   `PATH` variable. Falls back to the process environment's `PATH` if not
   found in the provided env vars.

## Cross-user behavior

The `user` parameter is accepted for API symmetry with the Python implementation
but is currently unused. Cross-user executable resolution would require querying
the target user's PATH, which is not yet implemented. The function falls back to
same-user resolution — the executable must be accessible to both users.

## Known issue

The process-environment PATH fallback has a bug where it always resolves to an
empty string instead of the actual PATH value. Since the module is not yet
integrated (`#[allow(dead_code)]`), this does not affect runtime behavior. It
should be fixed before the module is activated:

```rust
// Current (broken):
.or_else(|| std::env::var("PATH").ok().as_deref().map(|_| ""))

// Correct:
.unwrap_or_else(|| std::env::var("PATH").unwrap_or_default());
```

## Integration plan

When Windows cross-user support is implemented, this function should be called
from `run_subprocess` before command execution, matching the Python library's
`_locate_executable` call site. The resolved path ensures `CreateProcessAsUserW`
can find the executable even when the target user has a different PATH.
