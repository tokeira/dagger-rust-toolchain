//! Depends on nothing, and nothing depends on it: an edit elsewhere leaves
//! its test run cached.

/// Whether a number is even.
pub fn even(number: u64) -> bool {
    number % 2 == 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn tells_even_from_odd() {
        assert!(super::even(2));
        assert!(!super::even(3));
    }
}
