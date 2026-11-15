//! A package that is a member of no workspace and has no lockfile: another
//! workspace depends on it by path.

/// Where this package's value comes from.
pub fn origin() -> &'static str {
    env!("OUTSIDE_SHARED_ORIGIN")
}
