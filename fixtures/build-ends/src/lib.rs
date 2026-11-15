//! A package that compiles with no feature and not with every feature. It
//! has no default feature, so clippy and `cargo check --no-default-features`
//! pass. `cargo check --all-features` does not.

#[cfg(feature = "experimental")]
compile_error!("build-ends does not compile with its experimental feature");

/// Whether the package was built as it is meant to be.
pub fn ready() -> bool {
    true
}
