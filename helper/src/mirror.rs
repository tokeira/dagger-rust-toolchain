//! The source mirror: a directory kept equal to a source view, by content.
//!
//! Cargo decides by modification time whether a package it takes from a path
//! is up to date. A file whose content is the same in the view and in the
//! mirror is therefore left alone, with the time it has. A file that differs
//! is written again and gets a new time. A file that the view no longer has
//! is removed.
//!
//! A manifest can get a new time although its content is the same. Cargo
//! tracks what rustc read file by file. For rustdoc, and for a build script
//! that names no file, it goes by the newest time among the files of the
//! package, of which the manifest is one. That misses two things, both seen
//! with Cargo 1.100 and `cargo doc`:
//!
//! - a file that is gone leaves the newest time as it was. So the manifest of
//!   a package that lost a file always gets a new time;
//! - a file outside the package that its documentation includes is not one
//!   of the package's files. So for rustdoc, every manifest gets a new time
//!   whenever the sync changed anything: [`Renew::All`].

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, FileType};
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::SystemTime;

/// Which manifests get a new time although their content is the same.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Renew {
    /// The manifest of each package that lost a file.
    #[default]
    Lost,
    /// Every manifest, when the sync wrote or removed anything.
    All,
}

impl Renew {
    pub fn named(name: &str) -> Option<Renew> {
        match name {
            "lost" => Some(Renew::Lost),
            "all" => Some(Renew::All),
            _ => None,
        }
    }
}

/// What a sync did, with paths relative to the mirror.
#[derive(Debug, Default)]
pub struct Synced {
    /// Files and links written, new or with other content.
    pub written: Vec<String>,
    /// Files, links and directories removed. A directory ends with `/`.
    pub removed: Vec<String>,
    /// How many files and links were the same, and kept their time.
    pub kept: usize,
    /// The manifests given a new time, their content being the same.
    pub renewed: Vec<String>,
    /// The manifests whose content was the same.
    manifests: Vec<String>,
}

/// Makes the directory `mirror` equal to the directory `source`.
pub fn sync(source: &Path, mirror: &Path, renew: Renew) -> io::Result<Synced> {
    let mut synced = Synced::default();
    directory(source, mirror, "", &mut synced)?;
    synced.written.sort_unstable();
    synced.removed.sort_unstable();
    let manifests = match renew {
        Renew::Lost => bereft(mirror, &synced),
        Renew::All if synced.written.is_empty() && synced.removed.is_empty() => BTreeSet::new(),
        Renew::All => synced.manifests.iter().cloned().collect(),
    };
    let now = SystemTime::now();
    for manifest in manifests {
        // One that the sync wrote has a new time already.
        if synced.written.binary_search(&manifest).is_err() {
            let file = File::options().write(true).open(mirror.join(&manifest))?;
            file.set_modified(now)?;
            synced.renewed.push(manifest);
        }
    }
    Ok(synced)
}

/// The manifest of each package that lost a file: the nearest `Cargo.toml`
/// above what was removed.
fn bereft(mirror: &Path, synced: &Synced) -> BTreeSet<String> {
    let mut manifests = BTreeSet::new();
    for removed in &synced.removed {
        let mut above = Path::new(removed.trim_end_matches('/')).parent();
        while let Some(directory) = above {
            let manifest = directory.join("Cargo.toml");
            if mirror.join(&manifest).is_file() {
                manifests.insert(manifest.to_string_lossy().into_owned());
                break;
            }
            above = directory.parent();
        }
    }
    manifests
}

/// The entries of a directory by name, each with its kind. A link is a link,
/// whatever it points to.
fn entries(directory: &Path) -> io::Result<BTreeMap<OsString, FileType>> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        entries.insert(entry.file_name(), entry.file_type()?);
    }
    Ok(entries)
}

fn remove(path: &Path, kind: FileType) -> io::Result<()> {
    if kind.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn directory(source: &Path, mirror: &Path, at: &str, synced: &mut Synced) -> io::Result<()> {
    let wanted = entries(source)?;
    let present = entries(mirror)?;
    for (name, kind) in &present {
        // A view holds files, links and directories. Anything else in it is
        // not mirrored, and has no place in the mirror either.
        let mirrored = wanted
            .get(name)
            .is_some_and(|kind| kind.is_dir() || kind.is_file() || kind.is_symlink());
        if !mirrored {
            remove(&mirror.join(name), *kind)?;
            let slash = if kind.is_dir() { "/" } else { "" };
            synced
                .removed
                .push(format!("{at}{}{slash}", name.to_string_lossy()));
        }
    }
    for (name, kind) in &wanted {
        let from = source.join(name);
        let to = mirror.join(name);
        let path = format!("{at}{}", name.to_string_lossy());
        let existing = present.get(name).copied();
        if kind.is_dir() {
            match existing {
                Some(existing) if existing.is_dir() => {}
                Some(existing) => {
                    remove(&to, existing)?;
                    fs::create_dir(&to)?;
                }
                None => fs::create_dir(&to)?,
            }
            directory(&from, &to, &format!("{path}/"), synced)?;
        } else if kind.is_file() {
            if existing.is_some_and(|existing| existing.is_file()) && same(&from, &to)? {
                same_mode(&from, &to)?;
                synced.kept += 1;
                if name == "Cargo.toml" {
                    synced.manifests.push(path);
                }
            } else {
                // Removed first: a new file has a new time whatever the old
                // one was, and no old permission stands in the way.
                if let Some(existing) = existing {
                    remove(&to, existing)?;
                }
                fs::copy(&from, &to)?;
                synced.written.push(path);
            }
        } else if kind.is_symlink() {
            let target = fs::read_link(&from)?;
            if existing.is_some_and(|existing| existing.is_symlink())
                && fs::read_link(&to)? == target
            {
                synced.kept += 1;
            } else {
                if let Some(existing) = existing {
                    remove(&to, existing)?;
                }
                std::os::unix::fs::symlink(&target, &to)?;
                synced.written.push(path);
            }
        }
    }
    Ok(())
}

/// Whether two files hold the same bytes.
fn same(left: &Path, right: &Path) -> io::Result<bool> {
    if fs::metadata(left)?.len() != fs::symlink_metadata(right)?.len() {
        return Ok(false);
    }
    let mut left = File::open(left)?;
    let mut right = File::open(right)?;
    let mut ours = vec![0; 64 * 1024];
    let mut theirs = vec![0; 64 * 1024];
    loop {
        let read = filled(&mut left, &mut ours)?;
        if read != filled(&mut right, &mut theirs)? || ours[..read] != theirs[..read] {
            return Ok(false);
        }
        if read < ours.len() {
            return Ok(true);
        }
    }
}

/// Reads until the buffer is full or the file ends, and returns how much.
fn filled(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut read = 0;
    while read < buffer.len() {
        match file.read(&mut buffer[read..]) {
            Ok(0) => break,
            Ok(more) => read += more,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(read)
}

/// Gives a kept file the view's permissions, such as the executable bit of a
/// script. Changing them does not change the file's modification time.
fn same_mode(from: &Path, to: &Path) -> io::Result<()> {
    let wanted = fs::metadata(from)?.permissions();
    if fs::metadata(to)?.permissions().mode() & 0o7777 != wanted.mode() & 0o7777 {
        fs::set_permissions(to, wanted)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use super::{sync, Renew};
    use crate::scratch::Scratch;

    /// A time well before now, which a file written again cannot have.
    fn long_ago() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000)
    }

    fn dated(path: &Path) {
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(long_ago()).unwrap();
    }

    fn modified(path: &Path) -> SystemTime {
        fs::metadata(path).unwrap().modified().unwrap()
    }

    #[test]
    fn fills_an_empty_mirror() {
        let scratch = Scratch::new("fills");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        scratch.file("source/Cargo.toml", "[package]\n");
        scratch.file("source/crates/a/src/lib.rs", "pub fn a() {}\n");
        fs::create_dir(source.join("empty")).unwrap();

        let synced = sync(&source, &mirror, Renew::Lost).unwrap();

        assert_eq!(synced.written, ["Cargo.toml", "crates/a/src/lib.rs"]);
        assert!(synced.removed.is_empty());
        assert_eq!(synced.kept, 0);
        assert_eq!(
            fs::read_to_string(mirror.join("crates/a/src/lib.rs")).unwrap(),
            "pub fn a() {}\n"
        );
        assert!(mirror.join("empty").is_dir());
    }

    #[test]
    fn keeps_the_time_of_a_file_that_is_the_same_and_renews_one_that_differs() {
        let scratch = Scratch::new("times");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        scratch.file("source/same.rs", "fn same() {}\n");
        scratch.file("source/edited.rs", "fn before() {}\n");
        scratch.file("source/resized.rs", "fn short() {}\n");
        sync(&source, &mirror, Renew::Lost).unwrap();
        for file in ["same.rs", "edited.rs", "resized.rs"] {
            dated(&mirror.join(file));
        }

        // The same length with other bytes, and another length.
        scratch.file("source/edited.rs", "fn after_() {}\n");
        scratch.file("source/resized.rs", "fn rather_longer() {}\n");
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();

        assert_eq!(synced.written, ["edited.rs", "resized.rs"]);
        assert_eq!(synced.kept, 1);
        assert_eq!(modified(&mirror.join("same.rs")), long_ago());
        assert!(modified(&mirror.join("edited.rs")) > long_ago());
        assert!(modified(&mirror.join("resized.rs")) > long_ago());
        assert_eq!(
            fs::read_to_string(mirror.join("edited.rs")).unwrap(),
            "fn after_() {}\n"
        );

        // A file put back as it was differs from the mirror again.
        scratch.file("source/edited.rs", "fn before() {}\n");
        dated(&mirror.join("edited.rs"));
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();
        assert_eq!(synced.written, ["edited.rs"]);
        assert!(modified(&mirror.join("edited.rs")) > long_ago());
    }

    #[test]
    fn removes_what_the_source_no_longer_has() {
        let scratch = Scratch::new("removes");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        scratch.file("source/src/lib.rs", "mod gone;\n");
        scratch.file("source/src/gone.rs", "");
        scratch.file("source/tests/old/data.txt", "data\n");
        sync(&source, &mirror, Renew::Lost).unwrap();
        // What a build left in the mirror goes too.
        scratch.file("mirror/src/generated.rs", "");

        fs::remove_file(source.join("src/gone.rs")).unwrap();
        fs::remove_dir_all(source.join("tests")).unwrap();
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();

        assert_eq!(
            synced.removed,
            ["src/generated.rs", "src/gone.rs", "tests/"]
        );
        assert!(synced.written.is_empty());
        assert_eq!(synced.kept, 1);
        assert!(synced.renewed.is_empty());
        assert!(!mirror.join("src/gone.rs").exists());
        assert!(!mirror.join("src/generated.rs").exists());
        assert!(!mirror.join("tests").exists());
        assert!(mirror.join("src/lib.rs").exists());
    }

    #[test]
    fn renews_the_manifest_of_a_package_that_lost_a_file() {
        let scratch = Scratch::new("renews");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        let manifests = [
            "Cargo.toml",
            "crates/kept/Cargo.toml",
            "crates/lost/Cargo.toml",
            "crates/lost/inner/Cargo.toml",
            "crates/edited/Cargo.toml",
        ];
        for manifest in manifests {
            scratch.file(&format!("source/{manifest}"), "[package]\n");
        }
        scratch.file("source/crates/kept/src/lib.rs", "");
        scratch.file("source/crates/lost/src/deep/gone.rs", "");
        scratch.file("source/crates/lost/inner/src/lib.rs", "");
        scratch.file("source/crates/edited/src/gone.rs", "");
        scratch.file("source/notes/gone.txt", "");
        sync(&source, &mirror, Renew::Lost).unwrap();
        for manifest in manifests {
            dated(&mirror.join(manifest));
        }

        // A file goes from a package, from a package whose manifest changes
        // too, and from a directory that is in no package but the root's.
        fs::remove_dir_all(source.join("crates/lost/src/deep")).unwrap();
        fs::remove_file(source.join("crates/edited/src/gone.rs")).unwrap();
        scratch.file(
            "source/crates/edited/Cargo.toml",
            "[package]\nedition = \"2021\"\n",
        );
        fs::remove_file(source.join("notes/gone.txt")).unwrap();
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();

        assert_eq!(
            synced.removed,
            [
                "crates/edited/src/gone.rs",
                "crates/lost/src/deep/",
                "notes/gone.txt"
            ]
        );
        assert_eq!(synced.written, ["crates/edited/Cargo.toml"]);
        assert_eq!(synced.renewed, ["Cargo.toml", "crates/lost/Cargo.toml"]);
        for renewed in [
            "Cargo.toml",
            "crates/lost/Cargo.toml",
            "crates/edited/Cargo.toml",
        ] {
            assert!(modified(&mirror.join(renewed)) > long_ago(), "{renewed}");
        }
        // The packages beside it and below it lost nothing.
        for kept in ["crates/kept/Cargo.toml", "crates/lost/inner/Cargo.toml"] {
            assert_eq!(modified(&mirror.join(kept)), long_ago(), "{kept}");
        }
        assert_eq!(
            fs::read_to_string(mirror.join("crates/lost/Cargo.toml")).unwrap(),
            "[package]\n"
        );
    }

    #[test]
    fn renews_every_manifest_when_asked_and_anything_changed() {
        let scratch = Scratch::new("renews-all");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        let manifests = ["Cargo.toml", "crates/a/Cargo.toml", "crates/b/Cargo.toml"];
        for manifest in manifests {
            scratch.file(&format!("source/{manifest}"), "[package]\n");
        }
        scratch.file("source/NOTES.md", "notes\n");
        scratch.file("source/crates/a/src/lib.rs", "");
        sync(&source, &mirror, Renew::All).unwrap();
        for file in [
            "Cargo.toml",
            "crates/a/Cargo.toml",
            "crates/b/Cargo.toml",
            "crates/a/src/lib.rs",
        ] {
            dated(&mirror.join(file));
        }

        // Nothing changed: nothing is renewed.
        let synced = sync(&source, &mirror, Renew::All).unwrap();
        assert!(synced.renewed.is_empty());
        assert_eq!(modified(&mirror.join("crates/a/Cargo.toml")), long_ago());

        // A file outside every package but the root's changes.
        scratch.file("source/NOTES.md", "other notes\n");
        let synced = sync(&source, &mirror, Renew::All).unwrap();
        assert_eq!(synced.written, ["NOTES.md"]);
        assert_eq!(synced.renewed, manifests);
        for manifest in manifests {
            assert!(modified(&mirror.join(manifest)) > long_ago(), "{manifest}");
        }
        // Only manifests are: a source file keeps its time.
        assert_eq!(modified(&mirror.join("crates/a/src/lib.rs")), long_ago());

        // The same change renews none of them otherwise.
        for manifest in manifests {
            dated(&mirror.join(manifest));
        }
        scratch.file("source/NOTES.md", "notes again\n");
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();
        assert!(synced.renewed.is_empty());
        assert_eq!(modified(&mirror.join("Cargo.toml")), long_ago());
    }

    #[test]
    fn replaces_an_entry_of_another_kind() {
        let scratch = Scratch::new("kinds");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        scratch.file("source/a", "a file\n");
        scratch.file("source/b/inner", "in a directory\n");
        std::os::unix::fs::symlink("a", source.join("c")).unwrap();
        // The mirror has a directory where the file is, a file where the
        // directory is, and a file where the link is.
        scratch.file("mirror/a/stale", "");
        scratch.file("mirror/b", "");
        scratch.file("mirror/c", "");

        let synced = sync(&source, &mirror, Renew::Lost).unwrap();

        assert_eq!(synced.written, ["a", "b/inner", "c"]);
        assert_eq!(fs::read_to_string(mirror.join("a")).unwrap(), "a file\n");
        assert_eq!(
            fs::read_to_string(mirror.join("b/inner")).unwrap(),
            "in a directory\n"
        );
        assert_eq!(fs::read_link(mirror.join("c")).unwrap(), Path::new("a"));

        // A link that points elsewhere is made again, and one that is the
        // same is kept.
        fs::remove_file(source.join("c")).unwrap();
        std::os::unix::fs::symlink("b/inner", source.join("c")).unwrap();
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();
        assert_eq!(synced.written, ["c"]);
        assert_eq!(
            fs::read_link(mirror.join("c")).unwrap(),
            Path::new("b/inner")
        );
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();
        assert!(synced.written.is_empty());
        assert_eq!(synced.kept, 3);
    }

    #[test]
    fn follows_the_permissions_without_a_new_time() {
        let scratch = Scratch::new("modes");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        scratch.file("source/script.sh", "#!/bin/sh\n");
        sync(&source, &mirror, Renew::Lost).unwrap();
        dated(&mirror.join("script.sh"));

        fs::set_permissions(source.join("script.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();

        assert!(synced.written.is_empty());
        let mode = fs::metadata(mirror.join("script.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
        assert_eq!(modified(&mirror.join("script.sh")), long_ago());
    }

    #[test]
    fn writes_over_a_file_it_may_not_write_to() {
        let scratch = Scratch::new("readonly");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        scratch.file("source/locked.rs", "fn first() {}\n");
        fs::set_permissions(source.join("locked.rs"), fs::Permissions::from_mode(0o444)).unwrap();
        sync(&source, &mirror, Renew::Lost).unwrap();

        fs::set_permissions(source.join("locked.rs"), fs::Permissions::from_mode(0o644)).unwrap();
        scratch.file("source/locked.rs", "fn second() {}\n");
        let synced = sync(&source, &mirror, Renew::Lost).unwrap();

        assert_eq!(synced.written, ["locked.rs"]);
        assert_eq!(
            fs::read_to_string(mirror.join("locked.rs")).unwrap(),
            "fn second() {}\n"
        );
    }

    #[test]
    fn compares_files_larger_than_one_read() {
        let scratch = Scratch::new("large");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        let mut bytes = vec![b'x'; 200 * 1024];
        fs::write(source.join("large"), &bytes).unwrap();
        sync(&source, &mirror, Renew::Lost).unwrap();
        assert_eq!(sync(&source, &mirror, Renew::Lost).unwrap().kept, 1);

        // One byte differs, past the first read.
        bytes[150 * 1024] = b'y';
        fs::write(source.join("large"), &bytes).unwrap();
        assert_eq!(
            sync(&source, &mirror, Renew::Lost).unwrap().written,
            ["large"]
        );
        assert_eq!(fs::read(mirror.join("large")).unwrap(), bytes);
    }
}
