//! Runs a compile lane's Cargo command as one step of the rust module.
//!
//! A compile lane keeps its source and its target directory in cache
//! volumes, so that Cargo compiles only what changed. The engine keys a step
//! on its inputs and never on what a volume holds, so everything that reads
//! or writes the volumes happens here, in one step:
//!
//! 1. the source mirror is made equal to the lane's source view, by content;
//! 2. the target volume is checked to be one started for this command;
//! 3. the command runs, in the directory this program was started in;
//! 4. what later steps read of the build is copied into the step's own
//!    filesystem.
//!
//! ```text
//! rust-lane-helper [--source <view> --mirror <mirror> [--renew lost|all]]
//!                  [--marks <file>] [--stdout <file>]
//!                  [--keep <directory> --listing nextest|cargo]
//!                  [--report <file>] -- <command> [<argument>...]
//! ```
//!
//! The command's standard output goes to `--stdout`, or to this program's
//! own. Its exit code is this program's. When a signal stopped the command,
//! this program says so and exits with 128 plus the signal. When this
//! program itself fails, it exits with 125 and the step must not be kept.
//!
//! `--report` names a file that gets, for each run in turn: the command, what
//! the sync wrote, removed and gave a new time, the packages Cargo compiled
//! and those it documented, and the command's exit code.
//!
//! The target volume is Cargo's target directory, `CARGO_TARGET_DIR`.

mod compiled;
mod json;
mod keep;
mod mirror;
mod volume;

use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

use keep::Listing;
use mirror::Renew;

/// The exit code of a failure of this program's own, as `env` and `timeout`
/// report theirs. Cargo fails with 101.
const FAILED: i32 = 125;

fn main() {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    let options = Options::parse(arguments).map(|options| Options {
        target: env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
        ..options
    });
    let code = match options.and_then(|options| run(&options)) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("rust-lane-helper: {error}");
            FAILED
        }
    };
    process::exit(code);
}

/// What the arguments ask for.
#[derive(Debug, Default)]
struct Options {
    /// The source view, and the mirror to make equal to it.
    mirror: Option<(PathBuf, PathBuf)>,
    /// Which manifests of the mirror get a new time.
    renew: Renew,
    /// The file that holds what the target volume must have been started for.
    marks: Option<PathBuf>,
    /// Where the command's standard output goes.
    stdout: Option<PathBuf>,
    /// Where to keep what the listing in `stdout` names, and its kind.
    keep: Option<(PathBuf, Listing)>,
    /// Where to report what happened.
    report: Option<PathBuf>,
    /// Cargo's target directory, if the command has one.
    target: Option<PathBuf>,
    /// The command and its arguments.
    command: Vec<OsString>,
}

impl Options {
    fn parse(arguments: Vec<OsString>) -> Result<Options, String> {
        let mut options = Options::default();
        let (mut source, mut mirror, mut keep, mut listing) = (None, None, None, None);
        let mut arguments = arguments.into_iter();
        loop {
            let Some(flag) = arguments.next() else {
                return Err("no command: the arguments must end with `--` and a command".to_owned());
            };
            let flag = flag.to_string_lossy().into_owned();
            if flag == "--" {
                break;
            }
            let Some(value) = arguments.next() else {
                return Err(format!("{flag} needs a value"));
            };
            match flag.as_str() {
                "--source" => source = Some(PathBuf::from(value)),
                "--mirror" => mirror = Some(PathBuf::from(value)),
                "--renew" => {
                    let name = value.to_string_lossy();
                    options.renew = Renew::named(&name)
                        .ok_or_else(|| format!("--renew must be `lost` or `all`, not `{name}`"))?;
                }
                "--marks" => options.marks = Some(PathBuf::from(value)),
                "--stdout" => options.stdout = Some(PathBuf::from(value)),
                "--keep" => keep = Some(PathBuf::from(value)),
                "--listing" => {
                    let name = value.to_string_lossy();
                    listing = Some(Listing::named(&name).ok_or_else(|| {
                        format!("--listing must be `nextest` or `cargo`, not `{name}`")
                    })?);
                }
                "--report" => options.report = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {flag}")),
            }
        }
        options.command = arguments.collect();
        if options.command.is_empty() {
            return Err("no command after `--`".to_owned());
        }
        options.mirror = match (source, mirror) {
            (Some(source), Some(mirror)) => Some((source, mirror)),
            (None, None) => None,
            _ => return Err("--source and --mirror go together".to_owned()),
        };
        options.keep = match (keep, listing) {
            (Some(keep), Some(listing)) if options.stdout.is_some() => Some((keep, listing)),
            (None, None) => None,
            _ => return Err("--keep, --listing and --stdout go together".to_owned()),
        };
        Ok(options)
    }

    /// The command as one line, for messages.
    fn shown(&self) -> String {
        let words: Vec<_> = self
            .command
            .iter()
            .map(|word| word.to_string_lossy())
            .collect();
        words.join(" ")
    }
}

/// Runs the steps, and writes the report whether they succeed or not.
fn run(options: &Options) -> Result<i32, String> {
    let mut report = vec![format!("run {}", options.shown())];
    let result = steps(options, &mut report);
    if let Some(file) = &options.report {
        append(file, &report).map_err(|error| format!("writing {}: {error}", file.display()))?;
    }
    result
}

fn steps(options: &Options, report: &mut Vec<String>) -> Result<i32, String> {
    // The directory the command runs in. It may be in the mirror, and not
    // there yet.
    let directory =
        env::current_dir().map_err(|error| format!("the working directory: {error}"))?;

    if let Some((source, mirror)) = &options.mirror {
        let synced = mirror::sync(source, mirror, options.renew).map_err(|error| {
            format!(
                "mirroring {} to {}: {error}",
                source.display(),
                mirror.display()
            )
        })?;
        report.push(format!(
            "mirror wrote {} removed {} kept {}",
            synced.written.len(),
            synced.removed.len(),
            synced.kept
        ));
        report.extend(synced.written.iter().map(|path| format!("wrote {path}")));
        report.extend(synced.removed.iter().map(|path| format!("removed {path}")));
        report.extend(synced.renewed.iter().map(|path| format!("renewed {path}")));
    }

    let target = &options.target;
    if let Some(file) = &options.marks {
        let target = target
            .as_ref()
            .ok_or("CARGO_TARGET_DIR is not set, and --marks is about it")?;
        let marks = fs::read_to_string(file)
            .map_err(|error| format!("reading {}: {error}", file.display()))?;
        let emptied = volume::claim(target, &marks)
            .map_err(|error| format!("claiming {}: {error}", target.display()))?;
        if emptied {
            report.push("emptied the target volume, which was started for other marks".to_owned());
        }
    }
    let moment = match target {
        Some(target) if target.is_dir() => Some(
            compiled::moment(target)
                .map_err(|error| format!("writing to {}: {error}", target.display()))?,
        ),
        _ => None,
    };

    let mut command = Command::new(&options.command[0]);
    command.args(&options.command[1..]).current_dir(&directory);
    if let Some(file) = &options.stdout {
        command.stdout(Stdio::from(created(file)?));
    }
    let status = command
        .status()
        .map_err(|error| format!("running `{}`: {error}", options.shown()))?;

    if let (Some(target), Some(moment)) = (target, moment) {
        let invoked = compiled::since(target, moment)
            .map_err(|error| format!("reading {}: {error}", target.display()))?;
        let compiled = invoked.compiled.iter();
        report.extend(compiled.map(|package| format!("compiled {package}")));
        let documented = invoked.documented.iter();
        report.extend(documented.map(|package| format!("documented {package}")));
    }

    let Some(code) = status.code() else {
        let signal = status.signal().unwrap_or_default();
        report.push(format!("signal {signal}"));
        // Said as Cargo says it of a compiler, which is what the lane looks
        // for to run the step again.
        eprintln!(
            "rust-lane-helper: `{}` was stopped (signal: {signal})",
            options.shown()
        );
        return Ok(128 + signal);
    };
    report.push(format!("exit {code}"));

    if let (Some((into, listing)), Some(file), true) =
        (&options.keep, &options.stdout, status.success())
    {
        let target = target
            .as_ref()
            .ok_or("CARGO_TARGET_DIR is not set, and --keep copies from it")?;
        let text = fs::read_to_string(file)
            .map_err(|error| format!("reading {}: {error}", file.display()))?;
        fs::create_dir_all(into).map_err(|error| format!("making {}: {error}", into.display()))?;
        let kept = keep::keep(*listing, &text, target, into)?;
        report.push(format!("kept {kept}"));
    }
    Ok(code)
}

/// A new file, with the directories above it.
fn created(file: &Path) -> Result<File, String> {
    let wrong = |error| format!("writing {}: {error}", file.display());
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(wrong)?;
    }
    File::create(file).map_err(wrong)
}

/// Adds lines to a file, which a run before this one may have started.
fn append(file: &Path, lines: &[String]) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut report = File::options().create(true).append(true).open(file)?;
    for line in lines {
        writeln!(report, "{line}")?;
    }
    Ok(())
}

/// A directory of a test's own, removed when the test ends.
#[cfg(test)]
mod scratch {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    pub struct Scratch {
        root: PathBuf,
    }

    impl Scratch {
        pub fn new(name: &str) -> Scratch {
            let unique = format!(
                "rust-lane-helper-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir_all(&root).unwrap();
            // Without links in it: a path under it is then a path below it.
            Scratch {
                root: root.canonicalize().unwrap(),
            }
        }

        /// A path under the directory.
        pub fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        /// A directory under it, made with those above it.
        pub fn dir(&self, relative: &str) -> PathBuf {
            let path = self.path(relative);
            fs::create_dir_all(&path).unwrap();
            path
        }

        /// A file under it with this content, made with the directories above it.
        pub fn file(&self, relative: &str, content: &str) {
            let path = self.path(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use super::{run, Options, FAILED};
    use crate::keep::Listing;
    use crate::mirror::Renew;
    use crate::scratch::Scratch;

    fn parsed(arguments: &[&str]) -> Result<Options, String> {
        Options::parse(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn reads_its_arguments() {
        let options = parsed(&[
            "--source",
            "/view",
            "--mirror",
            "/src/app",
            "--report",
            "/report",
            "--stdout",
            "/kept/list.json",
            "--keep",
            "/kept",
            "--listing",
            "nextest",
            "--",
            "cargo",
            "clippy",
            "--",
            "--no-deps",
        ])
        .unwrap();
        assert_eq!(options.mirror, Some(("/view".into(), "/src/app".into())));
        assert_eq!(options.renew, Renew::Lost);
        assert_eq!(options.stdout, Some("/kept/list.json".into()));
        assert_eq!(options.keep, Some(("/kept".into(), Listing::Nextest)));
        assert_eq!(options.report, Some("/report".into()));
        assert_eq!(options.shown(), "cargo clippy -- --no-deps");

        let options = parsed(&["--renew", "all", "--", "cargo", "doc"]).unwrap();
        assert_eq!(options.renew, Renew::All);
        let options = parsed(&["--marks", "/marks", "--", "cargo", "clippy"]).unwrap();
        assert_eq!(options.marks, Some("/marks".into()));
    }

    #[test]
    fn refuses_arguments_it_cannot_follow() {
        for arguments in [
            &["cargo", "check"][..],
            &["--"],
            &["--source", "/view", "--", "cargo"],
            &["--keep", "/kept", "--listing", "nextest", "--", "cargo"],
            &["--stdout", "/out", "--keep", "/kept", "--", "cargo"],
            &[
                "--stdout",
                "/out",
                "--keep",
                "/kept",
                "--listing",
                "other",
                "--",
                "cargo",
            ],
            &["--renew", "some", "--", "cargo"],
            &["--unknown", "value", "--", "cargo"],
            &["--report"],
        ] {
            assert!(parsed(arguments).is_err(), "{arguments:?} must be refused");
        }
    }

    #[test]
    fn mirrors_then_runs_and_reports_each_run() {
        let scratch = Scratch::new("run");
        let (source, mirror) = (scratch.dir("source"), scratch.dir("mirror"));
        scratch.file("source/word.txt", "mirrored\n");
        let (report, output) = (scratch.path("out/report"), scratch.path("out/stdout"));
        let arguments = |command: &[&str]| {
            let mut arguments = vec![
                "--source".into(),
                source.clone().into_os_string(),
                "--mirror".into(),
                mirror.clone().into_os_string(),
                "--report".into(),
                report.clone().into_os_string(),
                "--stdout".into(),
                output.clone().into_os_string(),
                "--".into(),
            ];
            arguments.extend(command.iter().map(OsString::from));
            Options::parse(arguments).unwrap()
        };

        // The command sees the mirror as the sync left it.
        let word = mirror.join("word.txt");
        let code = run(&arguments(&["cat", word.to_str().unwrap()])).unwrap();
        assert_eq!(code, 0);
        assert_eq!(fs::read_to_string(&output).unwrap(), "mirrored\n");

        // The command's exit code is the run's, and the report goes on.
        fs::remove_file(source.join("word.txt")).unwrap();
        let code = run(&arguments(&["cat", word.to_str().unwrap()])).unwrap();
        assert_ne!(code, 0);
        assert_ne!(code, FAILED);
        let lines = fs::read_to_string(&report).unwrap();
        let lines: Vec<&str> = lines.lines().collect();
        assert_eq!(lines[0], format!("run cat {}", word.display()));
        assert_eq!(
            lines[1..4],
            [
                "mirror wrote 1 removed 0 kept 0",
                "wrote word.txt",
                "exit 0"
            ]
        );
        assert_eq!(lines[4], lines[0]);
        assert_eq!(
            lines[5..7],
            ["mirror wrote 0 removed 1 kept 0", "removed word.txt"]
        );
        assert_eq!(lines[7], format!("exit {code}"));
        assert_eq!(lines.len(), 8);
    }

    #[test]
    fn claims_the_target_volume_and_reports_what_was_compiled() {
        let scratch = Scratch::new("target");
        let target = scratch.dir("target");
        scratch.file(
            "target/debug/.fingerprint/stale-0123456789abcdef/invoked.timestamp",
            "",
        );
        scratch.file("marks", "packages:\ncrates/a\n");
        let unit = scratch.dir("target/debug/.fingerprint/built-0123456789abcdef");
        let options = |unit: &std::path::Path| Options {
            marks: Some(scratch.path("marks")),
            report: Some(scratch.path("report")),
            target: Some(target.clone()),
            command: vec!["touch".into(), unit.join("invoked.timestamp").into()],
            ..Options::default()
        };

        // The volume has no marks: it is emptied, and the command's unit goes with it.
        assert_ne!(run(&options(&unit)).unwrap(), 0);
        assert!(!target.join("debug").exists());
        assert_eq!(
            fs::read_to_string(target.join(crate::volume::MARKS)).unwrap(),
            "packages:\ncrates/a\n"
        );

        // It has the lane's marks now, and keeps what is there: a unit that an
        // earlier run compiled.
        let unit = scratch.dir("target/debug/.fingerprint/built-0123456789abcdef");
        let fresh = "target/debug/.fingerprint/fresh-0123456789abcdef/invoked.timestamp";
        scratch.file(fresh, "");
        let earlier =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        let invoked = fs::File::options()
            .write(true)
            .open(scratch.path(fresh))
            .unwrap();
        invoked.set_modified(earlier).unwrap();
        assert_eq!(run(&options(&unit)).unwrap(), 0);
        assert!(target
            .join("debug/.fingerprint/fresh-0123456789abcdef")
            .exists());
        let report = fs::read_to_string(scratch.path("report")).unwrap();
        let lines: Vec<&str> = report
            .lines()
            .filter(|line| !line.starts_with("run "))
            .collect();
        assert_eq!(
            lines,
            [
                "emptied the target volume, which was started for other marks",
                "exit 1",
                "compiled built",
                "exit 0"
            ]
        );
    }

    #[test]
    fn fails_itself_when_it_cannot_mirror_or_run() {
        let scratch = Scratch::new("fails");
        let absent = scratch.path("absent");
        let mirror = scratch.dir("mirror");
        let options = Options {
            mirror: Some((absent, mirror)),
            command: vec!["true".into()],
            ..Options::default()
        };
        assert!(run(&options).unwrap_err().contains("mirroring"));
        let options = Options {
            command: vec!["rust-lane-helper-no-such-command".into()],
            ..Options::default()
        };
        assert!(run(&options).unwrap_err().contains("running"));
    }

    /// Stops this process with a signal. Only the test below runs it, in a
    /// process of its own.
    #[test]
    #[ignore = "aborts the process it runs in"]
    fn aborts() {
        std::process::abort();
    }

    #[test]
    fn says_when_a_signal_stopped_the_command() {
        let scratch = Scratch::new("signal");
        let report = scratch.path("report");
        let tests = std::env::current_exe().unwrap().into_os_string();
        let options = Options {
            report: Some(report.clone()),
            command: [
                tests,
                "--exact".into(),
                "tests::aborts".into(),
                "--ignored".into(),
            ]
            .to_vec(),
            stdout: Some(scratch.path("stdout")),
            ..Options::default()
        };
        // SIGABRT is signal 6.
        assert_eq!(run(&options).unwrap(), 128 + 6);
        assert!(fs::read_to_string(report).unwrap().ends_with("signal 6\n"));
    }
}
