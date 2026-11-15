//! A package that does not compile, and that Cargo cannot tell from the
//! stub the dependency layer compiled in its place. When this file is older
//! than the layer, only the removal of the stub's artifact makes Cargo
//! compile it and find the error.

/// Adds a number and a word, which no type allows.
pub fn add(left: u64) -> u64 {
    left + "right"
}
