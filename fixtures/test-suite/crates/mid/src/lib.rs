//! Depends on the base package and on a registry crate. Its integration test
//! reads a file of its own directory.

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        const FAMILY: &str = "unix";
    } else {
        const FAMILY: &str = "other";
    }
}

/// The base package's word, with the operating system's family.
pub fn phrase() -> String {
    format!("{} on {FAMILY}", suite_base::word())
}

#[cfg(test)]
mod tests {
    #[test]
    fn phrases() {
        assert!(super::phrase().ends_with(super::FAMILY));
    }
}
