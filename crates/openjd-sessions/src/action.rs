// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Action state and result types.

/// State of an action execution.
///
/// ```
/// use openjd_sessions::ActionState;
///
/// let state = ActionState::Success;
/// assert_eq!(state, ActionState::Success);
/// assert_ne!(state, ActionState::Failed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionState {
    Running,
    Success,
    Failed,
    Canceled,
    Timeout,
}

/// A parsed openjd stdout message from a running action.
#[derive(Debug, Clone)]
pub enum ActionMessage {
    /// `openjd_progress: <number>`
    Progress(f64),
    /// `openjd_status: <message>`
    Status(String),
    /// `openjd_fail: <message>`
    Fail(String),
    /// `openjd_env: <var>=<value>`
    SetEnv { name: String, value: String },
    /// `openjd_unset_env: <var>`
    UnsetEnv { name: String },
    /// `openjd_redacted_env: <var>=<value>`
    RedactedEnv { name: String, value: String },
    /// Request to cancel the action and mark it as failed (from malformed env commands)
    CancelMarkFailed { fail_message: String },
}

/// Result of running an action.
#[derive(Debug)]
pub struct ActionResult {
    pub state: ActionState,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
