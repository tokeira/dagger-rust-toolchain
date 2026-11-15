//! A package that is built, and tested, only where the environment has
//! `ACCEPTANCE_ORIGIN`.

/// What `ACCEPTANCE_ORIGIN` was when the package was built.
pub fn origin() -> &'static str {
    env!("BUILT_FOR")
}

#[cfg(test)]
mod tests {
    /// The test's own environment has the variable, with the value the
    /// build had.
    #[test]
    fn runs_where_it_was_built() {
        let here = std::env::var("ACCEPTANCE_ORIGIN");
        assert_eq!(
            here.as_deref(),
            Ok(super::origin()),
            "the test ran without the build's ACCEPTANCE_ORIGIN"
        );
    }
}
