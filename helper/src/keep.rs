//! What later steps read of a build: the files that the build's own listing
//! names, copied out of the target directory at the paths they have there.
//!
//! The target directory is a cache volume. A later step cannot read a volume
//! through the engine, and what the volume holds by then may be another
//! run's. So the step that builds copies what the test runs need into its own
//! filesystem, which is part of its result.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::json::{self, Value};

/// The listing a build printed, by the tool that printed it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Listing {
    /// `cargo nextest list --list-type binaries-only --message-format json`:
    /// one document.
    Nextest,
    /// `cargo test --no-run --message-format json-render-diagnostics`: one
    /// message on each line.
    Cargo,
}

impl Listing {
    pub fn named(name: &str) -> Option<Listing> {
        match name {
            "nextest" => Some(Listing::Nextest),
            "cargo" => Some(Listing::Cargo),
            _ => None,
        }
    }
}

/// A path the listing names, and whether a run cannot do without it.
struct Named {
    path: String,
    required: bool,
}

/// Copies the files and directories that `listing` names from `target` to
/// the same paths under `into`, and returns how many files that made.
pub fn keep(kind: Listing, listing: &str, target: &Path, into: &Path) -> Result<usize, String> {
    let named = match kind {
        Listing::Nextest => nextest(listing)?,
        Listing::Cargo => cargo(listing)?,
    };
    let mut kept = 0;
    for Named { path, required } in named {
        let Some(relative) = within(target, Path::new(&path)) else {
            // Outside the target directory: nothing of the build's to keep.
            continue;
        };
        let from = target.join(&relative);
        let to = into.join(&relative);
        let copied = match fs::symlink_metadata(&from) {
            Ok(_) => copy(&from, &to),
            Err(error) if error.kind() == io::ErrorKind::NotFound && !required => Ok(0),
            Err(error) => Err(error),
        };
        kept += copied.map_err(|error| format!("keeping {}: {error}", from.display()))?;
    }
    Ok(kept)
}

/// What cargo-nextest's list of binaries names: the test binaries by their
/// absolute paths, and relative to the target directory the binaries that
/// tests run, the build scripts' output directories and the directories of
/// libraries that the binaries load.
fn nextest(listing: &str) -> Result<Vec<Named>, String> {
    let document =
        json::parse(listing).map_err(|error| format!("cargo-nextest's list: {error}"))?;
    let mut named = Vec::new();
    let mut name = |value: Option<&Value>, required: bool| {
        if let Some(path) = value.and_then(Value::as_str) {
            named.push(Named {
                path: path.to_owned(),
                required,
            });
        }
    };
    if let Some(binaries) = document.get("rust-binaries") {
        for binary in binaries.values() {
            name(binary.get("binary-path"), true);
        }
    }
    if let Some(build) = document.get("rust-build-meta") {
        if let Some(packages) = build.get("non-test-binaries") {
            for binary in packages.values().into_iter().flat_map(Value::values) {
                name(binary.get("path"), true);
            }
        }
        if let Some(directories) = build.get("build-script-out-dirs") {
            for directory in directories.values() {
                name(Some(directory), false);
            }
        }
        if let Some(directories) = build.get("linked-paths") {
            for directory in directories.values() {
                name(Some(directory), false);
            }
        }
    }
    Ok(named)
}

/// What Cargo's messages name, of the packages Cargo takes from a path: the
/// executable of each target that has one, and the output directory of each
/// build script that ran.
fn cargo(listing: &str) -> Result<Vec<Named>, String> {
    let mut named = Vec::new();
    for line in listing.lines() {
        // Only the lines that can name a path are read: a build prints one
        // for every artifact and every diagnostic.
        let artifact = line.contains("\"reason\":\"compiler-artifact\"");
        let script = line.contains("\"reason\":\"build-script-executed\"");
        if !(artifact || script) || !line.contains("path+file://") {
            continue;
        }
        let message = json::parse(line).map_err(|error| format!("Cargo's messages: {error}"))?;
        let local = message
            .get("package_id")
            .and_then(Value::as_str)
            .is_some_and(|package| package.contains("path+file://"));
        if !local {
            continue;
        }
        let (member, required) = if artifact {
            ("executable", true)
        } else {
            ("out_dir", false)
        };
        if let Some(path) = message.get(member).and_then(Value::as_str) {
            named.push(Named {
                path: path.to_owned(),
                required,
            });
        }
    }
    Ok(named)
}

/// A named path relative to the target directory, or nothing when it is
/// outside it.
fn within(target: &Path, path: &Path) -> Option<PathBuf> {
    let relative = if path.is_absolute() {
        path.strip_prefix(target).ok()?
    } else {
        path
    };
    let plain = relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    (plain && relative.components().next().is_some()).then(|| relative.to_path_buf())
}

/// Copies a file, a link or a directory with all it holds, and returns how
/// many files and links that made.
fn copy(from: &Path, to: &Path) -> io::Result<usize> {
    let kind = fs::symlink_metadata(from)?.file_type();
    if kind.is_dir() {
        fs::create_dir_all(to)?;
        let mut copied = 0;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copied += copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(copied);
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::symlink_metadata(to).is_ok() {
        // Named twice: once is enough.
        return Ok(0);
    }
    if kind.is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(from)?, to)?;
    } else if kind.is_file() {
        fs::copy(from, to)?;
    } else {
        return Ok(0);
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{keep, Listing};
    use crate::scratch::Scratch;

    #[test]
    fn keeps_what_nextest_lists() {
        let scratch = Scratch::new("nextest");
        let (target, into) = (scratch.dir("target"), scratch.path("kept"));
        scratch.file(
            "target/debug/deps/suite_base-0123456789abcdef",
            "test binary",
        );
        scratch.file(
            "target/debug/deps/libsuite_base-0123456789abcdef.rlib",
            "library",
        );
        scratch.file("target/debug/suite-tool", "binary");
        scratch.file(
            "target/debug/build/user-0123456789abcdef/out/generated.rs",
            "generated",
        );
        scratch.file(
            "target/debug/build/user-0123456789abcdef/out/deep/more.rs",
            "more",
        );
        scratch.file(
            "target/debug/build/native-0123456789abcdef/out/libnative.a",
            "archive",
        );
        let listing = format!(
            r#"{{
              "rust-build-meta": {{
                "target-directory": "{target}",
                "base-output-directories": ["debug"],
                "non-test-binaries": {{
                  "path+file:///src/suite/crates/tool#suite-tool@0.1.0": [
                    {{"name": "suite-tool", "kind": "bin", "path": "debug/suite-tool"}}
                  ]
                }},
                "build-script-out-dirs": {{
                  "path+file:///src/suite/user#0.1.0": "debug/build/user-0123456789abcdef/out",
                  "path+file:///src/suite/gone#0.1.0": "debug/build/gone-0123456789abcdef/out"
                }},
                "linked-paths": ["debug/build/native-0123456789abcdef/out", "/usr/lib/outside"]
              }},
              "rust-binaries": {{
                "suite-base": {{
                  "binary-id": "suite-base",
                  "binary-path": "{target}/debug/deps/suite_base-0123456789abcdef"
                }}
              }}
            }}"#,
            target = target.display()
        );

        let kept = keep(Listing::Nextest, &listing, &target, &into).unwrap();

        assert_eq!(kept, 5);
        let read = |path: &str| fs::read_to_string(into.join(path)).unwrap();
        assert_eq!(
            read("debug/deps/suite_base-0123456789abcdef"),
            "test binary"
        );
        assert_eq!(read("debug/suite-tool"), "binary");
        assert_eq!(
            read("debug/build/user-0123456789abcdef/out/generated.rs"),
            "generated"
        );
        assert_eq!(
            read("debug/build/user-0123456789abcdef/out/deep/more.rs"),
            "more"
        );
        assert_eq!(
            read("debug/build/native-0123456789abcdef/out/libnative.a"),
            "archive"
        );
        // Not listed, so not kept: the whole profile directory is not.
        assert!(!into
            .join("debug/deps/libsuite_base-0123456789abcdef.rlib")
            .exists());
    }

    #[test]
    fn keeps_what_cargo_lists_of_local_packages() {
        let scratch = Scratch::new("cargo");
        let (target, into) = (scratch.dir("target"), scratch.path("kept"));
        scratch.file("target/debug/deps/data-0123456789abcdef", "test binary");
        scratch.file("target/debug/suite-tool", "binary");
        scratch.file(
            "target/debug/build/user-0123456789abcdef/out/generated.rs",
            "generated",
        );
        scratch.file(
            "target/debug/build/libc-0123456789abcdef/out/probe",
            "a dependency's",
        );
        let at = target.display();
        let listing = [
            format!(r#"{{"reason":"compiler-artifact","package_id":"path+file:///src/suite/crates/mid#suite-mid@0.1.0","target":{{"kind":["test"],"name":"data"}},"profile":{{"test":true}},"filenames":["{at}/debug/deps/data-0123456789abcdef"],"executable":"{at}/debug/deps/data-0123456789abcdef","fresh":false}}"#),
            format!(r#"{{"reason":"compiler-artifact","package_id":"path+file:///src/suite/crates/mid#suite-mid@0.1.0","target":{{"kind":["lib"],"name":"suite_mid"}},"profile":{{"test":false}},"filenames":["{at}/debug/deps/libsuite_mid-0123456789abcdef.rlib"],"executable":null,"fresh":true}}"#),
            format!(r#"{{"reason":"compiler-artifact","package_id":"suite-tool 0.1.0 (path+file:///src/suite/crates/tool)","target":{{"kind":["bin"],"name":"suite-tool"}},"profile":{{"test":false}},"filenames":["{at}/debug/suite-tool"],"executable":"{at}/debug/suite-tool","fresh":false}}"#),
            format!(r#"{{"reason":"build-script-executed","package_id":"path+file:///src/suite/user#0.1.0","linked_libs":[],"linked_paths":[],"cfgs":[],"env":[],"out_dir":"{at}/debug/build/user-0123456789abcdef/out"}}"#),
            format!(r#"{{"reason":"build-script-executed","package_id":"registry+https://github.com/rust-lang/crates.io-index#libc@0.2.0","linked_libs":[],"linked_paths":[],"cfgs":[],"env":[],"out_dir":"{at}/debug/build/libc-0123456789abcdef/out"}}"#),
            r#"{"reason":"compiler-message","package_id":"path+file:///src/suite/crates/mid#suite-mid@0.1.0","message":{"rendered":"warning: unused"}}"#.to_owned(),
            "warning: a line that is no message".to_owned(),
            r#"{"reason":"build-finished","success":true}"#.to_owned(),
        ]
        .join("\n");

        let kept = keep(Listing::Cargo, &listing, &target, &into).unwrap();

        assert_eq!(kept, 3);
        assert!(into.join("debug/deps/data-0123456789abcdef").exists());
        assert!(into.join("debug/suite-tool").exists());
        assert!(into
            .join("debug/build/user-0123456789abcdef/out/generated.rs")
            .exists());
        assert!(!into.join("debug/build/libc-0123456789abcdef").exists());
    }

    #[test]
    fn fails_when_a_listed_binary_is_missing_or_the_listing_is_broken() {
        let scratch = Scratch::new("missing");
        let (target, into) = (scratch.dir("target"), scratch.path("kept"));
        let listing = format!(
            r#"{{"rust-binaries": {{"a": {{"binary-path": "{}/debug/deps/absent"}}}}}}"#,
            target.display()
        );
        let error = keep(Listing::Nextest, &listing, &target, &into).unwrap_err();
        assert!(error.contains("debug/deps/absent"), "{error}");
        assert!(keep(Listing::Nextest, "{", &target, &into).is_err());
    }

    #[test]
    fn keeps_nothing_from_outside_the_target_directory() {
        let scratch = Scratch::new("outside");
        let (target, into) = (scratch.dir("target"), scratch.path("kept"));
        scratch.file("secret", "not the build's");
        let listing = format!(
            r#"{{"rust-binaries": {{
                "a": {{"binary-path": "{}"}},
                "b": {{"binary-path": "../secret"}},
                "c": {{"binary-path": "debug/../../secret"}}
            }}}}"#,
            scratch.path("secret").display()
        );
        assert_eq!(keep(Listing::Nextest, &listing, &target, &into).unwrap(), 0);
        assert!(!into.exists() || fs::read_dir(&into).unwrap().count() == 0);
    }
}
