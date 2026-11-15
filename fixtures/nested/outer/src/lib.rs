//! The only member of the outer workspace. It keeps a Cargo.lock of its own,
//! which does not make it a workspace: Cargo finds its workspace above it.

/// The name of this workspace.
pub const WORKSPACE: &str = "outer";
