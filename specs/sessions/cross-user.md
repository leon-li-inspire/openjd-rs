# Cross-User Execution

## Overview

Cross-user execution allows the session runtime (running as a service user) to execute
actions as a different user (the job's designated user). This is required for production
deployments where the worker agent runs as root or a service account but actions must
run with the job submitter's permissions.

Currently fully implemented for POSIX/Linux (including the embedded cross-user helper).
Windows has partial support — see the Windows section below.

## SessionUser Trait

```rust
pub trait SessionUser: Send + Sync {
    fn user(&self) -> &str;
    fn is_process_user(&self) -> bool;
}
```

### Why a trait instead of a concrete type

The Python library has `SessionUser` (ABC), `PosixSessionUser`, and `WindowsSessionUser`.
The trait allows future Windows support without changing the session API. The `Send + Sync`
bounds are required because the user is stored in `Session` and passed to async tasks.

## PosixSessionUser

```rust
pub struct PosixSessionUser {
    pub user: String,
    pub group: String,
}

impl PosixSessionUser {
    pub fn new(user: impl Into<String>, group: Option<String>) -> Self;
}
```

### Why group defaults to process effective group

When `group` is `None`, it defaults to the process's effective group via
`nix::unistd::getegid()`. This matches the Python library's behavior and is the common
case — the worker agent typically doesn't specify a group explicitly.

### is_process_user

Compares the user name against the process's effective UID via `nix::unistd::geteuid()`
and `nix::unistd::User::from_uid()`. When the session user matches the process user,
cross-user machinery is bypassed — no sudo, no chown, no shell script wrapper.

## Subprocess Launch

When `SubprocessConfig.user` is set and `!user.is_process_user()`:

1. **Shell script generation**: A wrapper script is written to the session working
   directory containing env var exports, `cd`, and `exec` of the actual command.
   This is necessary because `sudo -i` resets the environment.

2. **Launch command**: `sudo -u <user> -i setsid -w <script_path>`
   - `-u <user>`: run as the target user
   - `-i`: login shell (loads user's profile)
   - `setsid -w`: create new process group, wait for child
   - The script path is the generated wrapper

3. **Process group discovery**: `find_sudo_child_pgid()` finds the actual child's
   PGID by reading `/proc/<sudo_pid>/task/*/children` (Linux procfs), with a
   `pgrep -P <sudo_pid>` fallback. Retries for up to 1 second since the child
   may not appear immediately.

### Why setsid is in the sudo command, not pre_exec

For same-user execution, `setsid` is called in a `pre_exec` hook on the `Command`.
For cross-user execution, this doesn't work because the `pre_exec` hook runs before
`sudo` starts — the process group would be created for the sudo process, not the
actual command. Instead, `setsid -w` is part of the sudo command line, creating the
process group for the target user's process.

## Signal Delivery

### Same-user signals

Direct `nix::sys::signal::killpg(pgid, signal)` — the process has permission to signal
its own children.

### Cross-user signals

The process may not have permission to signal processes owned by another user. The
fallback chain:

1. **Direct killpg**: Try `killpg(pgid, signal)`. Works if the process has `CAP_KILL`
   capability.
2. **sudo kill**: Fall back to `sudo -u <user> -i kill -s <signal> -- -<pgid>`. The
   negative PGID with `--` separator ensures `kill` interprets it as a process group.

### SIGTERM delivery to sudo

For SIGTERM (notify), the signal is sent to the sudo process directly rather than the
child's process group. Sudo forwards SIGTERM to its child, which is the expected behavior
for graceful shutdown. This avoids the need to discover the child PGID for the notify
phase.

For SIGKILL (terminate), the signal must go to the child's process group directly because
sudo doesn't forward SIGKILL.

## File Ownership

### Working directory

`TempDir::new()` accepts an optional `&PosixSessionUser`. When provided:
- `nix::unistd::chown(path, None, Some(group_gid))` sets group ownership
- Permissions are set to 0o770 (owner + group rwx) instead of 0o700

### Embedded files

`chown_for_user()` in `embedded_files.rs`:
- Sets group ownership to the session user's group
- Sets group read/write permissions (and execute if `runnable`)

### Cleanup

`Session::cleanup()` for cross-user sessions:
1. `sudo rm -rf <working_dir>` as the session user — removes files owned by that user
2. `std::fs::remove_dir_all()` as the process user — removes any remaining files

The two-phase cleanup is necessary because files created by the cross-user subprocess
are owned by that user and may not be deletable by the process user.

## CAP_KILL Capability

`capabilities.rs` provides Linux-specific `CAP_KILL` support:

```rust
pub fn try_use_cap_kill() -> Option<CapKillGuard>;

pub struct CapKillGuard { /* RAII — clears CAP_KILL on drop */ }
```

When the process has `CAP_KILL` in its permitted set, `try_use_cap_kill()` temporarily
elevates it to the effective set, returning a guard that clears it on drop. This allows
direct `killpg()` for cross-user signal delivery without falling back to sudo.

The `caps` crate provides the Linux capability API. On non-Linux platforms, these
functions are no-ops.

### Why RAII for capability management

Capabilities should be held for the minimum necessary duration (principle of least
privilege). The guard pattern ensures `CAP_KILL` is cleared even if the caller returns
early or panics.

## Windows Support

The Python library supports Windows cross-user execution via:
- `WindowsSessionUser` with password or logon token
- `CreateProcessWithLogonW` / `CreateProcessAsUserW` via ctypes
- `WindowsPermissionHelper` for ACL management
- `PopenWindowsAsUser` subclass of `Popen`

The Rust crate has partial Windows support implemented:

### WindowsSessionUser (`session_user.rs`)

`WindowsSessionUser` supports two authentication modes:
- **Password mode** (non-Session 0): validates credentials via `LogonUserW` at construction
- **Logon token mode** (Session 0 / services / SSH): accepts a pre-existing `HANDLE`

If the user matches the process owner, neither password nor token is needed.

### Process spawning (`win32.rs`)

`spawn_as_user()` creates a cross-user process via `CreateProcessWithLogonW` (password
mode) or `CreateProcessAsUserW` (token mode). Environment variables are passed as a
Win32 environment block. Stdout and stderr are merged via a shared anonymous pipe
(mirroring the POSIX `dup2` approach).

### Signal delivery (`subprocess.rs`)

- **Notify**: `CTRL_BREAK_EVENT` via `GenerateConsoleCtrlEvent` with console
  attach/detach dance (mirrors Python's `_signal_win_subprocess.py`)
- **Terminate**: `TerminateProcess` on the entire process tree via
  `CreateToolhelp32Snapshot` traversal (mirrors Python's `_windows_process_killer.py`)

### Permissions (`win32_permissions.rs`)

`set_permissions()` sets DACLs on files and directories: full control for the process
user, modify access for the session user. Used by `TempDir` and `EmbeddedFiles` for
cross-user file access.

### Not yet implemented

- Cross-user helper binary (Windows equivalent of the POSIX embedded helper)
- Full integration testing (no Windows Docker test infrastructure yet)
