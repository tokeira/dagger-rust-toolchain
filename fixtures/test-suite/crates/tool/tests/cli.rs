//! Runs the package's binary, which Cargo builds for integration tests.

use std::process::Command;

#[test]
fn prints_the_phrase() {
    let output = Command::new(env!("CARGO_BIN_EXE_suite-tool"))
        .output()
        .expect("the package's binary");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(" on "));
}
