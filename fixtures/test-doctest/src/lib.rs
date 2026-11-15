//! A package that is its own Cargo workspace, with a unit test that passes
//! and a doctest that fails. With one member, Cargo runs the doctests under
//! `--workspace` and names no package when they fail.

/// Halves a number.
///
/// ```
/// assert_eq!(doctest_alone::halve(42), 20);
/// ```
pub fn halve(number: u64) -> u64 {
    number / 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn halves() {
        assert_eq!(super::halve(4), 2);
    }
}
