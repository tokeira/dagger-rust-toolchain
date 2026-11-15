//! A member that compiles, and in which clippy reports an error: comparing
//! a value with itself is a lint clippy denies by default.

/// Whether a number equals itself.
pub fn same(value: u32) -> bool {
    value == value
}
