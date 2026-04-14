// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Embedded cross-user helper binary — written to disk at session start.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::error::SessionError;
use crate::session_user::SessionUser;

const HELPER_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/openjd_helper"));

/// Write the embedded helper binary to `working_dir/openjd_helper`, set
/// permissions to 0o750, and chown the group to the session user's group.
pub(crate) fn write_helper(
    working_dir: &Path,
    user: &dyn SessionUser,
) -> Result<PathBuf, SessionError> {
    let path = working_dir.join("openjd_helper");
    std::fs::write(&path, HELPER_BINARY).map_err(|source| SessionError::WorkingDirectory {
        path: path.clone(),
        source,
    })?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o750)).map_err(|source| {
        SessionError::WorkingDirectory {
            path: path.clone(),
            source,
        }
    })?;
    if let Ok(Some(grp)) = nix::unistd::Group::from_name(user.group()) {
        let _ = nix::unistd::chown(&path, None, Some(grp.gid));
    }
    Ok(path)
}
