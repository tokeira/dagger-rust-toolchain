//! A build script that a signal stops, as the kernel stops a compiler that
//! runs out of memory. Cargo reports the signal, and nothing in the source
//! says why.

fn main() {
    std::process::abort();
}
