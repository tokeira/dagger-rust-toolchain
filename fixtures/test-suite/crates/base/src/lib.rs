//! The package the others build on: a unit test and a doctest.

/// The word every package of the fixture passes on.
pub fn word() -> &'static str {
    "base"
}

/// Doubles a number.
///
/// ```
/// assert_eq!(suite_base::double(21), 42);
/// ```
pub fn double(number: u64) -> u64 {
    number * 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn doubles() {
        assert_eq!(super::double(2), 4);
    }
}
