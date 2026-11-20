//! A package whose failures differ each time they happen: a test and a
//! doctest panic with the moment they ran at. Two runs that report the same
//! moment were one run, and the second report came from a cache.

use std::time::{SystemTime, UNIX_EPOCH};

/// The time, in nanoseconds since the Unix epoch.
///
/// ```
/// panic!("documented-at-{}", test_moment::moment());
/// ```
pub fn moment() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is set after 1970")
        .as_nanos()
}

#[cfg(test)]
mod tests {
    #[test]
    fn tells_the_time() {
        assert!(super::moment() > 0);
    }
}
