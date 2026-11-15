//! A member found by the workspace's `crates/*` glob, with one registry
//! dependency and a member nested in its directory.

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        /// The family of the operating system this was built for.
        pub const FAMILY: &str = "unix";
    } else {
        /// The family of the operating system this was built for.
        pub const FAMILY: &str = "other";
    }
}

/// A greeting built with the nested member.
pub fn greeting() -> String {
    members_derive::decorate("hello")
}
