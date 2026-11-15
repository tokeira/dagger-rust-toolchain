//! A member whose data file a test of another member reads.

/// The word in `data/word.txt`.
///
/// ```
/// assert_eq!(whole_other::word(), "elsewhere");
/// ```
pub fn word() -> &'static str {
    include_str!("../data/word.txt").trim()
}

#[cfg(test)]
mod tests {
    #[test]
    fn has_a_word() {
        assert!(!super::word().is_empty());
    }
}
