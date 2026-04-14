# Action Filter

## Overview

`ActionFilter` in `action_filter.rs` parses `openjd_*` directives from subprocess stdout
lines. It is the Rust equivalent of the Python `ActionMonitoringFilter`, but implemented
as a standalone struct rather than a `logging.Filter` subclass.

## Directive Protocol

The OpenJD specification defines stdout messages that actions emit to communicate with
the runtime. Each directive is a single line matching `^openjd_<kind>: <payload>$`:

| Directive | Payload | Effect |
|-----------|---------|--------|
| `openjd_progress` | Float 0–100 | Update action progress percentage |
| `openjd_status` | String | Update human-readable status message |
| `openjd_fail` | String | Set failure reason, trigger cancelation |
| `openjd_env` | `NAME=VALUE` | Set environment variable for subsequent actions |
| `openjd_unset_env` | `NAME` | Unset environment variable |
| `openjd_redacted_env` | `NAME=VALUE` | Set env var + redact value in logs |
| `openjd_session_runtime_loglevel` | `DEBUG\|INFO\|WARNING\|ERROR` | Change log level |

## ActionFilter Interface

```rust
pub struct ActionFilter {
    session_id: String,
    suppress_filtered: bool,
    redactions_enabled: bool,
    redacted_values: HashSet<String>,
    redacted_lines: HashSet<String>,
    log_level: u32,
}

impl ActionFilter {
    pub fn new(session_id: &str, suppress_filtered: bool, redactions_enabled: bool) -> Self;

    pub fn filter_message(&mut self, line: &str, session_id: &str)
        -> (Vec<FilterCallback>, bool, String);

    pub fn min_log_level(&self) -> u32;

    pub fn add_redacted_values(&mut self, values: &[String]);
}
```

### FilterCallback

Rather than returning `ActionMessage` values directly, the filter returns
`FilterCallback` structs that decouple the filter from the session's message types:

```rust
pub struct FilterCallback {
    pub kind: ActionMessageKind,
    pub value: ActionMessageValue,
    pub cancel: bool,
}
```

The `cancel` flag indicates that the action should be canceled and marked as failed
(e.g., due to a malformed directive). The caller in `subprocess.rs` maps these to
`ActionMessage` variants and sends them through the mpsc channel.

### Why `session_id` is a parameter

The filter stores a `session_id` at construction and compares it against the
`session_id` parameter on each call. Lines from a different session are passed through
unmodified. This supports shared log streams where multiple sessions interleave output.

### Why `filter_message` returns a tuple

The return value `(Vec<FilterCallback>, bool, String)` contains:

1. `Vec<FilterCallback>` — Zero or more parsed callbacks (usually 0 or 1)
2. `bool` — Whether the line should be passed through to logging (false for directives
   that are fully consumed when `suppress_filtered` is true)
3. `String` — The (possibly redacted) line for logging

This three-part return avoids the Python pattern of mutating shared state from a logging
filter callback. The caller decides what to do with each part.

### Why Vec instead of Option for callbacks

A single line can produce multiple callbacks in edge cases (e.g., a malformed directive
that produces both a `Fail` callback and a cancel callback). Using `Vec`
avoids special-casing these.

### Dynamic log level via openjd_session_runtime_loglevel

The filter tracks a `log_level` (default: 20 = INFO) that can be changed at runtime
by the `openjd_session_runtime_loglevel` directive. The subprocess stdout loop checks
`filter.min_log_level()` before logging command output lines, allowing actions to
suppress verbose output by raising the level to WARNING or ERROR.

## Regex-Based Parsing

A single compiled regex with named groups matches all directive types:

```rust
static FILTER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^openjd_(?:(?P<progress>progress)|(?P<status>status)|(?P<fail>fail)|(?P<env>env)|(?P<redacted_env>redacted_env)|(?P<unset_env>unset_env)|(?P<session_runtime_loglevel>session_runtime_loglevel)): (.+)$"
    ).unwrap()
});
```

Additional regexes validate env var names and detect near-miss malformed commands:

```rust
static ENV_ACTIONS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(openjd_env|openjd_redacted_env|openjd_unset_env)").unwrap()
});
static ENVVAR_SET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^"?[A-Za-z_][A-Za-z0-9_]*=.*$"#).unwrap()
});
static ENVVAR_UNSET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").unwrap()
});
```

### Why a single regex with named groups

Using one regex avoids testing each directive type sequentially. The named groups
identify which directive matched, and the final capture group extracts the payload.
`LazyLock` (std) replaces `lazy_static` for zero-dependency static initialization.

## Malformed Command Detection

If a line looks like an `openjd_*` directive but doesn't match any known pattern (e.g.,
`openjd_ENV: FOO=bar` with wrong case), the filter:

1. Performs a case-insensitive near-match check
2. If it matches a known directive name, emits `ActionMessage::CancelMarkFailed` with
   an error message explaining the malformed command
3. The session cancels the action with `Failed` state

This catches common mistakes like wrong capitalization or typos in directive names,
preventing silent failures where the action thinks it set an env var but the runtime
ignored it.

## Redaction

### How redaction works

When `openjd_redacted_env: NAME=VALUE` is processed:

1. The value is added to `redacted_values: HashSet<String>`
2. For multiline values, the first and last lines are added to `redacted_values`,
   and middle lines are added to `redacted_lines: HashSet<String>`
3. All subsequent log lines are checked against both sets
4. Matches are replaced with `"********"` (fixed-length, 8 asterisks)

### Why fixed-length replacement

Variable-length replacement (matching the original value's length) would leak information
about the value's size. Fixed-length replacement is a security best practice for
credential redaction.

### JSON-encoded env var format

Environment variable values can be JSON-encoded (e.g., `openjd_env: {"NAME": "VALUE"}`).
The filter detects JSON format and decodes it, supporting values that contain newlines
or special characters that can't be represented in the simple `NAME=VALUE` format.

## Integration with Subprocess

The subprocess stdout loop calls `filter.filter_message(line)` for each line:

```rust
let (messages, pass_through, modified_line) = filter.filter_message(&line);
for msg in messages {
    let _ = message_tx.send(msg);
}
if pass_through {
    session_log!(INFO, session_id, LogContent::COMMAND_OUTPUT, "{}", modified_line);
}
```

Messages flow through the mpsc channel to `Session::drive_action()`. Non-directive lines
are logged with redaction applied.
