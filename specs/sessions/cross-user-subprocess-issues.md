# Cross-User Subprocess Startup Issues

## Status

Open — plan documented, implementation pending.

## Observed Problems

During integration testing of the Rust-backed worker agent on Deadline Cloud
SMF (Linux, cross-user execution with `sudo -u job-user`), two issues were
observed in the subprocess startup path.

### 1. ~1 second startup latency per subprocess

Every cross-user subprocess takes approximately 1 second before producing any
output. The delay occurs between `Command started as pid` and the first line
of actual output.

**Example from session log:**
```
[02:54:00.527] Command started as pid: 5223
[02:54:00.527] Output:
[02:54:01.568] Exiting Conda Queue Environment!
```

The 1-second gap is consistent across all cross-user subprocesses. For a job
with many short tasks (e.g. 50 render frames at ~3 seconds each), this adds
~50 seconds of pure overhead.

**Root cause:** The `sudo -u job-user -i` invocation uses the `-i` (login shell)
flag, which causes sudo to:
1. Start a login shell for the target user
2. Source `/etc/profile`, `~/.bash_profile`, `~/.bashrc`, etc.
3. Set up the full login environment

This initialization takes ~1 second on AL2023 SMF instances.

**Current code path** (`subprocess.rs`):
```rust
let sudo_args = vec![
    "sudo".to_string(),
    "-u".to_string(),
    _user.user().to_string(),
    "-i".to_string(),       // ← login shell, expensive
    "setsid".to_string(),
    "-w".to_string(),
    script_path.to_string_lossy().to_string(),
];
```

Note: The Python implementation (`openjd-sessions-for-python`) has the same
`sudo -u user -i setsid -w` approach and the same ~1s overhead.

### 2. Logged command shows wrapper script instead of actual command

The session log shows the `sudo` wrapper command instead of the user's actual
command from the job template:

```
Running command sudo -u job-user -i setsid -w /sessions/.../_openjd_run_3418.sh
```

This is unhelpful for debugging — the user wants to see what command their
template is running (e.g. `maya-openjd daemon stop`), not the internal wrapper.

**Root cause:** The `format_command_for_log` function formats `final_args` which
is the sudo wrapper, not the original `args` from the template. The original
command is inside the generated shell script.

The Python implementation appends the original args directly to the sudo command
line (no wrapper script), so its logs show the full command.

## Solution: Embedded Cross-User Helper Binary

See [embedded-cross-user-helper.md](embedded-cross-user-helper.md) for the
detailed design.

Summary: Embed a small Rust helper binary in the `openjd-sessions` crate. At
session start, write it to disk and launch it once via `sudo -u job-user -i`.
The helper persists for the session lifetime and executes all subprocesses via
a stdin/stdout JSON protocol. This pays the 1-second sudo login cost once
instead of per-action, and the helper knows the actual command for logging.

### Immediate fix (command logging)

Before implementing the helper, fix the log to show the original command.
Log the original `args` before constructing the sudo wrapper:

```rust
// Log the user's command
session_log!(info, ..., "Running command {}", format_command_for_log(args));
// Log the wrapper at debug level
log::debug!(target: "openjd.sessions", "Wrapper: {}", format_command_for_log(&final_args));
```

## Measurements

From the turntable render job (50 frames, SMF Linux, cross-user):

| Metric | Current (sudo per action) | With helper |
|--------|--------------------------|-------------|
| First action startup | ~1.0s | ~1.0s (one-time) |
| Subsequent action startup | ~1.0s | ~1ms (pipe write) |
| 20-action session overhead | ~20s | ~1s |
| Overhead as % of 3s task | ~33% | ~1.7% |

## References

- `openjd-rs/crates/openjd-sessions/src/subprocess.rs` — subprocess execution
- `openjd-rs/specs/sessions/cross-user.md` — cross-user design
- `openjd-rs/specs/sessions/embedded-cross-user-helper.md` — helper design
- Python reference: `openjd-sessions-for-python` `src/openjd/sessions/_subprocess.py`
  lines 247-330 (`_start_subprocess` method) — also uses `sudo -u user -i setsid -w`
