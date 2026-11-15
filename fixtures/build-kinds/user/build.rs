//! A build script that gives the package a value at compile time.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-env=KINDS_BUILT_BY=a build script");
}
