//! A package that compiles and passes clippy, and whose documentation links
//! to an item that does not exist. rustdoc warns about the link.

/// Doubles a number, unlike [`halve`].
pub fn double(value: u32) -> u32 {
    value * 2
}
