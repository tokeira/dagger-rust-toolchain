//! A build script, so that the dependency layer's stub of this package has
//! one to compile and run.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-env=OUTSIDE_SHARED_ORIGIN=a build script");
}
