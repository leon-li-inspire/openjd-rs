// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Open Job Description sessions — local job execution runtime.
//!
//! Mirrors the Python `openjd-sessions-for-python` library.

pub mod action;
pub(crate) mod action_filter;
pub mod action_status;
pub(crate) mod capabilities;
pub mod embedded_files;
pub mod error;
pub mod let_bindings;
pub mod logging;
pub mod runner;
pub mod session;
pub mod session_user;
pub(crate) mod subprocess;
#[cfg(unix)]
pub(crate) mod helper_binary;
#[cfg(unix)]
pub(crate) mod cross_user_helper;
#[cfg(windows)]
pub mod win32;
#[cfg(windows)]
pub(crate) mod win32_permissions;
#[cfg(windows)]
pub(crate) mod win32_locate;
pub mod tempdir;

// Re-export path mapping from openjd-expr (mirrors Python where sessions re-exports from expr)
pub use openjd_expr::path_mapping;

pub use action::{ActionState, ActionResult, ActionMessage};
pub use action_status::ActionStatus;
pub use error::SessionError;
pub use logging::LogContent;
pub use openjd_expr::path_mapping::{PathFormat, PathMappingRule};
pub use runner::{CancelMethod, ScriptRunnerState};
pub use session::{Session, SessionState, SessionConfig, EnvironmentIdentifier};
pub use subprocess::SubprocessResult;
pub use tempdir::TempDir;
pub use session_user::SessionUser;
#[cfg(unix)]
pub use session_user::PosixSessionUser;
#[cfg(windows)]
pub use session_user::WindowsSessionUser;
#[cfg(windows)]
pub use session_user::BadCredentialsError;
