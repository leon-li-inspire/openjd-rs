// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Linux capability support for CAP_KILL.
//!
//! Mirrors Python `_linux/_capabilities.py`. Provides a guard that temporarily
//! elevates CAP_KILL into the thread's effective capability set, restoring the
//! original state on drop.

#[cfg(target_os = "linux")]
mod inner {
    use caps::{CapSet, Capability};

    /// Attempt to use CAP_KILL for direct signal delivery.
    ///
    /// Returns `(has_cap_kill, guard)`. If CAP_KILL was temporarily elevated from
    /// the permitted set into the effective set, the guard will clear it on drop.
    pub fn try_use_cap_kill() -> (bool, Option<CapKillGuard>) {
        // Check effective set first
        if caps::has_cap(None, CapSet::Effective, Capability::CAP_KILL).unwrap_or(false) {
            log::debug!(target: "openjd.sessions", "CAP_KILL is in the thread's effective set");
            return (true, None);
        }
        // Check permitted set
        if caps::has_cap(None, CapSet::Permitted, Capability::CAP_KILL).unwrap_or(false) {
            log::debug!(
                target: "openjd.sessions",
                "CAP_KILL is in the thread's permitted set. Temporarily adding to effective set"
            );
            if caps::raise(None, CapSet::Effective, Capability::CAP_KILL).is_ok() {
                return (true, Some(CapKillGuard));
            }
        }
        (false, None)
    }

    /// RAII guard that clears CAP_KILL from the effective set on drop.
    pub struct CapKillGuard;

    impl Drop for CapKillGuard {
        fn drop(&mut self) {
            log::debug!(target: "openjd.sessions", "Clearing CAP_KILL from the thread's effective set");
            let _ = caps::drop(None, CapSet::Effective, Capability::CAP_KILL);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod inner {
    /// No-op on non-Linux platforms. Always returns `(false, None)`.
    #[allow(dead_code)]
    pub fn try_use_cap_kill() -> (bool, Option<()>) {
        (false, None)
    }
}

#[allow(unused_imports)]
pub use inner::*;
