//! A member in which clippy reports a warning and no error: a `return` that
//! the function does not need.

/// The answer.
pub fn answer() -> u32 {
    return 42;
}
