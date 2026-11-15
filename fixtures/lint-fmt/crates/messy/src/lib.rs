//! A package that rustfmt changes, in two files.

mod extra;

pub use extra::doubled;

/// The smaller of two numbers.
pub fn smaller(left: u64,right: u64)->u64 {
    if left<right { left } else { right }
}
