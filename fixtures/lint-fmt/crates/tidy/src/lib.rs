//! A package that rustfmt leaves alone under the repository's rustfmt.toml.

/// The larger of two numbers.
pub fn larger(left: u64, right: u64) -> u64 {
  if left > right {
    left
  } else {
    right
  }
}
