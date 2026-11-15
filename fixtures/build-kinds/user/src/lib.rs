//! A package with every kind of Cargo target: a library, a binary, an
//! example, an integration test, a benchmark without the test harness and a
//! build script. It uses the workspace's procedural macro.

/// The answer, from the procedural macro.
pub fn answer() -> u32 {
    kinds_macros::answer!()
}

/// What gave this package its compile-time value.
pub fn built_by() -> &'static str {
    env!("KINDS_BUILT_BY")
}
