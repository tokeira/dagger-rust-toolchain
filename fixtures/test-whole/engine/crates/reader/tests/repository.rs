//! Tests that use the repository around their package, as the tests of a
//! large workspace do. They pass only where the run has the whole
//! repository, can write into it, and finds `git` and `cargo`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The workspace root: two directories above the package.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The repository root: the directory above the workspace root.
fn repository() -> PathBuf {
    workspace().join("..")
}

#[test]
fn reads_a_file_of_another_member() {
    let word = fs::read_to_string(workspace().join("crates/other/data/word.txt"))
        .expect("a file of the member crates/other");
    assert_eq!(whole_reader::echo(), format!("{0} {0}", word.trim()));
}

#[test]
fn reads_a_file_outside_every_member() {
    let note = fs::read_to_string(repository().join("notes/outside.txt"))
        .expect("a file of the repository, outside the workspace");
    assert!(note.contains("outside every member"));
}

#[test]
fn writes_into_the_tree() {
    let written = repository().join(".generated/written");
    fs::create_dir_all(&written).expect("a new directory in the repository");
    fs::write(written.join("note.txt"), "written by a test\n").expect("a new file there");
    let read = fs::read_to_string(written.join("note.txt")).expect("the file again");
    assert_eq!(read, "written by a test\n");
}

#[test]
fn runs_git() {
    let nested = repository().join(".generated/repository");
    fs::create_dir_all(&nested).expect("a new directory in the repository");
    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .current_dir(&nested)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(arguments)
            .output()
            .expect("git, on the PATH");
        let errors = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "git {arguments:?} failed: {errors}"
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    git(&["init", "--quiet", "--initial-branch", "main"]);
    assert_eq!(git(&["symbolic-ref", "--short", "HEAD"]), "main");
}

/// A project that the test writes into the repository and builds there, as
/// a test of a generated workspace does. Cargo must be on the PATH, and
/// must build where its defaults say: in the project's own `target`.
#[test]
fn builds_a_generated_project_with_cargo() {
    let project = repository().join(".generated/project");
    fs::create_dir_all(project.join("src")).expect("a new directory in the repository");
    let manifest =
        "[package]\nname = \"generated\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";
    let lock = "version = 4\n\n[[package]]\nname = \"generated\"\nversion = \"0.1.0\"\n";
    fs::write(project.join("Cargo.toml"), manifest).expect("the project's manifest");
    fs::write(project.join("Cargo.lock"), lock).expect("the project's lockfile");
    fs::write(project.join("src/main.rs"), "fn main() {}\n").expect("the project's source");
    let status = Command::new("cargo")
        .current_dir(&project)
        .args(["build", "--locked"])
        .status()
        .expect("cargo, on the PATH");
    assert!(
        status.success(),
        "cargo build --locked failed in the generated project"
    );
    let binary = project.join("target/debug/generated");
    assert!(binary.is_file(), "the build left no {}", binary.display());
}

/// Fails under cargo-nextest, which sets `NEXTEST`. The workspace's
/// `.config/nextest.toml` leaves it out of every run.
#[test]
fn left_out_by_the_nextest_configuration() {
    assert!(
        std::env::var_os("NEXTEST").is_none(),
        "cargo-nextest ran a test that .config/nextest.toml leaves out"
    );
}
