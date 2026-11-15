//! A package that compiles with its default feature and not without it.
//! clippy, which runs under the default features, passes. `cargo check
//! --no-default-features` does not.

#[cfg(not(feature = "std"))]
compile_error!("build-features needs its std feature");

/// Whether the package was built with what it needs.
pub fn ready() -> bool {
    true
}
