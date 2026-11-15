//! Which packages Cargo compiled in a run, and which it documented, as read
//! from the target directory.
//!
//! Cargo keeps a directory for each unit it builds, and renews the file
//! `invoked.timestamp` there whenever it runs the compiler, rustdoc or a
//! build script for the unit. A unit it finds fresh keeps the time of its
//! last build. The units invoked since a moment therefore name the packages
//! compiled since then: the members, and any dependency. A unit of rustdoc
//! keeps its other files under names that start with `doc-`.
//!
//! Cargo has laid the units out in two ways, under each profile's directory.
//! The first was seen with Cargo 1.91, the second with Cargo 1.100:
//!
//! ```text
//! .fingerprint/<package>-<hash>/invoked.timestamp       a unit
//! build/<package>-<hash>/invoked.timestamp              a build script's run
//!
//! build/<package>/<hash>/fingerprint/invoked.timestamp  a unit
//! ```
//!
//! This is Cargo's own layout, which Cargo does not promise to keep. What is
//! read here is a report for whoever inspects a lane, and decides nothing: a
//! Cargo that lays its units out another way gets an empty report.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

/// The file Cargo renews when it invokes a unit's compiler.
const INVOKED: &str = "invoked.timestamp";

/// How many directories lie between the target directory and a unit's
/// `invoked.timestamp` at most: `<triple>/<profile>/build/<package>/<hash>/fingerprint`.
const DEPTH: usize = 6;

/// Directories that hold no unit's `invoked.timestamp`, and many files.
const SKIPPED: [&str; 6] = ["deps", "out", "incremental", "examples", "doc", "tmp"];

/// The modification time the file system gives a file written now: the
/// moment to compare the units' times with. It comes from the same clock as
/// theirs, which the process's own clock may be ahead of.
pub fn moment(target: &Path) -> io::Result<SystemTime> {
    // A name of this call's own: nothing else writes or removes it.
    static CALLS: AtomicU32 = AtomicU32::new(0);
    let call = CALLS.fetch_add(1, Ordering::Relaxed);
    let probe = target.join(format!(".rust-lane-helper-{}-{call}", process::id()));
    fs::write(&probe, "")?;
    let moment = fs::metadata(&probe)?.modified();
    fs::remove_file(&probe)?;
    moment
}

/// The packages Cargo invoked a unit of, each list in order.
#[derive(Debug, Default, PartialEq)]
pub struct Invoked {
    /// With rustc or a build script.
    pub compiled: Vec<String>,
    /// With rustdoc.
    pub documented: Vec<String>,
}

/// The packages with a unit that Cargo invoked since `moment`.
pub fn since(target: &Path, moment: SystemTime) -> io::Result<Invoked> {
    let (mut compiled, mut documented) = (BTreeSet::new(), BTreeSet::new());
    units(
        target,
        &mut Vec::new(),
        moment,
        &mut compiled,
        &mut documented,
    )?;
    Ok(Invoked {
        compiled: compiled.into_iter().collect(),
        documented: documented.into_iter().collect(),
    })
}

/// Looks for the units below a directory. `above` holds the names of the
/// directories from the target directory down to it.
fn units(
    directory: &Path,
    above: &mut Vec<String>,
    moment: SystemTime,
    compiled: &mut BTreeSet<String>,
    documented: &mut BTreeSet<String>,
) -> io::Result<()> {
    let (mut invoked, mut rustdoc) = (false, false);
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            if above.len() < DEPTH && !SKIPPED.contains(&name.as_str()) {
                above.push(name);
                units(&entry.path(), above, moment, compiled, documented)?;
                above.pop();
            }
        } else if name == INVOKED {
            invoked = entry.metadata()?.modified()? >= moment;
        } else if name.starts_with("doc-") {
            rustdoc = true;
        }
    }
    if invoked {
        if let Some(package) = package(above) {
            if rustdoc { documented } else { compiled }.insert(package);
        }
    }
    Ok(())
}

/// The package of the unit whose `invoked.timestamp` is in the last of
/// these directories.
fn package(above: &[String]) -> Option<String> {
    match above {
        // build/<package>/<hash>/fingerprint
        [.., build, package, _hash, last] if build == "build" && last == "fingerprint" => {
            Some(package.clone())
        }
        // .fingerprint/<package>-<hash> and build/<package>-<hash>: the hash
        // has no `-`, a package's name may.
        [.., parent, unit] if parent == ".fingerprint" || parent == "build" => Some(
            unit.rsplit_once('-')
                .map_or(unit.as_str(), |(package, _)| package)
                .to_owned(),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use super::{moment, since, Invoked};
    use crate::scratch::Scratch;

    fn compiled(target: &Path, moment: SystemTime) -> Vec<String> {
        since(target, moment).unwrap().compiled
    }

    fn dated(scratch: &Scratch, unit: &str, time: SystemTime) {
        scratch.file(&format!("target/{unit}/invoked.timestamp"), "");
        let path = scratch.path(&format!("target/{unit}/invoked.timestamp"));
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    fn times() -> (SystemTime, SystemTime, SystemTime) {
        let before = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        let at = before + Duration::from_secs(60);
        (before, at, at + Duration::from_secs(60))
    }

    #[test]
    fn names_the_packages_invoked_since_a_moment() {
        let scratch = Scratch::new("compiled");
        let target = scratch.dir("target");
        let (before, at, after) = times();
        // Two units of one package, one of them fresh; a dependency that is
        // fresh; a build script that ran; a unit under a target triple; a
        // unit that never compiled.
        dated(
            &scratch,
            "debug/.fingerprint/suite-mid-0123456789abcdef",
            after,
        );
        dated(
            &scratch,
            "debug/.fingerprint/suite-mid-fedcba9876543210",
            before,
        );
        dated(
            &scratch,
            "debug/.fingerprint/cfg-if-0011223344556677",
            before,
        );
        dated(
            &scratch,
            "debug/.fingerprint/suite-tool-8899aabbccddeeff",
            at,
        );
        dated(&scratch, "debug/build/kinds-user-0123456789abcdef", after);
        dated(
            &scratch,
            "wasm32-unknown-unknown/release/.fingerprint/suite-base-0123456789abcdef",
            after,
        );
        scratch.dir("target/debug/.fingerprint/suite-lone-0123456789abcdef");
        // Not units: in a directory that holds none, or below no profile.
        dated(
            &scratch,
            "debug/build/kinds-user-0123456789abcdef/out",
            after,
        );
        dated(&scratch, "debug/deps", after);
        dated(&scratch, "debug/elsewhere/stray-0123456789abcdef", after);

        assert_eq!(
            compiled(&target, at),
            ["kinds-user", "suite-base", "suite-mid", "suite-tool"]
        );
        assert_eq!(
            since(&target, after + Duration::from_secs(1)).unwrap(),
            Invoked::default()
        );
    }

    #[test]
    fn tells_a_package_documented_from_one_compiled() {
        let scratch = Scratch::new("documented");
        let target = scratch.dir("target");
        let (before, at, after) = times();
        // rustdoc ran for two packages, in each layout, and rustc for one of
        // them; an earlier run documented a third.
        dated(
            &scratch,
            "debug/.fingerprint/suite-base-f276e9465aab0f48",
            after,
        );
        scratch.file(
            "target/debug/.fingerprint/suite-base-f276e9465aab0f48/doc-lib-suite_base",
            "",
        );
        dated(
            &scratch,
            "debug/.fingerprint/suite-base-cd52e5490be5168c",
            after,
        );
        scratch.file(
            "target/debug/.fingerprint/suite-base-cd52e5490be5168c/lib-suite_base",
            "",
        );
        dated(
            &scratch,
            "debug/build/suite-tool/017ad7429b72941a/fingerprint",
            at,
        );
        scratch.file(
            "target/debug/build/suite-tool/017ad7429b72941a/fingerprint/doc-bin-suite-tool",
            "",
        );
        dated(
            &scratch,
            "debug/build/suite-lone/28a6ce389ed08407/fingerprint",
            before,
        );
        scratch.file(
            "target/debug/build/suite-lone/28a6ce389ed08407/fingerprint/doc-lib-suite_lone",
            "",
        );

        let invoked = since(&target, at).unwrap();
        assert_eq!(invoked.compiled, ["suite-base"]);
        assert_eq!(invoked.documented, ["suite-base", "suite-tool"]);
    }

    #[test]
    fn names_the_packages_in_the_layout_of_cargo_1_100() {
        let scratch = Scratch::new("layout");
        let target = scratch.dir("target");
        let (before, at, after) = times();
        dated(
            &scratch,
            "debug/build/suite-mid/e6864d5c40aba065/fingerprint",
            after,
        );
        dated(
            &scratch,
            "debug/build/suite-mid/0123456789abcdef/fingerprint",
            before,
        );
        dated(
            &scratch,
            "debug/build/cfg-if/12614b4c90629439/fingerprint",
            before,
        );
        dated(
            &scratch,
            "aarch64-unknown-linux-gnu/debug/build/suite-base/deee0e8634ebdf4a/fingerprint",
            at,
        );
        // A unit's outputs are not looked into.
        dated(
            &scratch,
            "debug/build/suite-lone/3f592e50fc168871/out",
            after,
        );

        assert_eq!(compiled(&target, at), ["suite-base", "suite-mid"]);
    }

    #[test]
    fn takes_its_moment_from_the_file_system() {
        let scratch = Scratch::new("moment");
        let target = scratch.dir("target");
        let moment = moment(&target).unwrap();
        scratch.file(
            "target/debug/.fingerprint/later-0123456789abcdef/invoked.timestamp",
            "",
        );
        assert_eq!(compiled(&target, moment), ["later"]);
        assert!(fs::read_dir(&target).unwrap().count() == 1);
    }
}
