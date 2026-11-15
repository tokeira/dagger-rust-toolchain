//! A member with nothing for clippy to report, which depends on the member
//! that fails. clippy cannot check it until that member passes.

/// Whether two equals itself.
pub fn two() -> bool {
    lints_faulty::same(2)
}
