//! A procedural macro: a library the compiler loads and runs, which the
//! dependency layer's stub of this package must still be.

use proc_macro::TokenStream;

/// Expands to the answer.
#[proc_macro]
pub fn answer(_input: TokenStream) -> TokenStream {
    "42".parse().expect("a number is an expression")
}
