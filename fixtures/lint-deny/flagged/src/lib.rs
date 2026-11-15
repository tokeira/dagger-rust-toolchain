//! A workspace that cargo-deny rejects: its dependency has an advisory.

/// Whether two texts differ.
pub fn differ(left: &str, right: &str) -> bool {
    difference::Changeset::new(left, right, "").distance > 0
}
