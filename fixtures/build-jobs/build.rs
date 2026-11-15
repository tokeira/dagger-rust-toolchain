//! A build script that fails unless Cargo runs the build with the job count
//! that `jobs.txt` names. Cargo gives a build script its job count as
//! `NUM_JOBS`, so the package compiles only where the count reaches Cargo.

use std::{env, fs, process};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=jobs.txt");
    println!("cargo::rerun-if-env-changed=CARGO_BUILD_JOBS");
    let expected = fs::read_to_string("jobs.txt").expect("jobs.txt, beside the manifest");
    let jobs = env::var("NUM_JOBS").expect("NUM_JOBS, which Cargo sets");
    if jobs != expected.trim() {
        eprintln!(
            "build-jobs was built with {jobs} jobs, and jobs.txt names {}",
            expected.trim()
        );
        process::exit(1);
    }
}
