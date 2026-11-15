//! A workspace that cargo-deny accepts: one dependency, under a licence the policy allows.

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        /// The family of the operating system this was built for.
        pub const FAMILY: &str = "unix";
    } else {
        /// The family of the operating system this was built for.
        pub const FAMILY: &str = "other";
    }
}
