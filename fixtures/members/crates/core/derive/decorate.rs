//! A member nested in another member's directory. Its library is at a path
//! its manifest names, not one Cargo finds by itself.

/// Wraps a word in brackets.
pub fn decorate(word: &str) -> String {
    format!("[{word}]")
}
