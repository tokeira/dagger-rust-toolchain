//! A member of the first workspace, in a directory that the acceptance leaves out of
//! `workspaceRoots`. Cargo decides membership, so it stays a package of its workspace.

/// Whether this package is kept.
pub const KEPT: bool = true;
