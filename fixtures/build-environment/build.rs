//! A build script that fails unless the build's environment has
//! `ACCEPTANCE_ORIGIN`, and gives the package the variable's value.

use std::{env, process};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=ACCEPTANCE_ORIGIN");
    let Ok(origin) = env::var("ACCEPTANCE_ORIGIN") else {
        eprintln!("build-environment was built without ACCEPTANCE_ORIGIN");
        process::exit(1);
    };
    println!("cargo::rustc-env=BUILT_FOR={origin}");
}
