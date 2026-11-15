//! A member whose tests read and write the repository around it.

/// The other member's word, twice.
pub fn echo() -> String {
    format!("{0} {0}", whole_other::word())
}
