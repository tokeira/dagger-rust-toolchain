//! What a target volume was started for, where Cargo cannot tell.
//!
//! clippy lints the packages its command names and only compiles the other
//! members, and it reads `clippy.toml`. Cargo sees neither: it takes what one
//! command left in a target directory for what another would make. With
//! Cargo 1.100, a member that `cargo clippy -p <another>` had compiled passed
//! `cargo clippy --workspace` unlinted, and an edit to `clippy.toml` left
//! every member that had passed as it was.
//!
//! The lane therefore puts what its command covers, its marks, in a file of
//! the directory its volume starts from. The engine names a volume by that
//! directory, so other marks get another volume. This module checks that
//! they did: if the volume in hand holds other marks, or none, it is
//! emptied, and Cargo compiles everything again.

use std::fs;
use std::io;
use std::path::Path;

/// The file of a target volume that holds its marks.
pub const MARKS: &str = ".rust-lane-volume";

/// Makes sure the target volume is one started for `marks`, and returns
/// whether it had to be emptied for that.
pub fn claim(target: &Path, marks: &str) -> io::Result<bool> {
    let file = target.join(MARKS);
    if fs::read_to_string(&file).is_ok_and(|found| found == marks) {
        return Ok(false);
    }
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    fs::write(file, marks)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{claim, MARKS};
    use crate::scratch::Scratch;

    #[test]
    fn keeps_a_volume_started_for_the_same_marks() {
        let scratch = Scratch::new("claims");
        let target = scratch.dir("target");
        scratch.file(&format!("target/{MARKS}"), "packages:\ncrates/a\n");
        scratch.file("target/debug/deps/liba.rmeta", "compiled");

        assert!(!claim(&target, "packages:\ncrates/a\n").unwrap());
        assert!(target.join("debug/deps/liba.rmeta").exists());
    }

    #[test]
    fn empties_a_volume_started_for_other_marks_or_none() {
        let scratch = Scratch::new("empties");
        let target = scratch.dir("target");
        scratch.file(&format!("target/{MARKS}"), "packages:\ncrates/a\n");
        scratch.file("target/debug/deps/liba.rmeta", "compiled");
        scratch.file("target/CACHEDIR.TAG", "");

        assert!(claim(&target, "packages:\ncrates/a\ncrates/b\n").unwrap());
        assert!(!target.join("debug").exists());
        assert!(!target.join("CACHEDIR.TAG").exists());
        assert_eq!(
            fs::read_to_string(target.join(MARKS)).unwrap(),
            "packages:\ncrates/a\ncrates/b\n"
        );
        // It is now a volume for those marks.
        assert!(!claim(&target, "packages:\ncrates/a\ncrates/b\n").unwrap());

        fs::remove_file(target.join(MARKS)).unwrap();
        scratch.file("target/debug/deps/libb.rmeta", "compiled");
        assert!(claim(&target, "packages:\ncrates/a\ncrates/b\n").unwrap());
        assert!(!target.join("debug").exists());
    }
}
