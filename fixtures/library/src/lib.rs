//! A library as many are published: one package at the repository root, with
//! no `[workspace]` table, and a `Cargo.lock` that Git ignores.

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        const FAMILY: &str = "unix";
    } else {
        const FAMILY: &str = "other";
    }
}

/// The operating system's family.
pub fn family() -> &'static str {
    FAMILY
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_a_family() {
        assert!(!super::family().is_empty());
    }
}
