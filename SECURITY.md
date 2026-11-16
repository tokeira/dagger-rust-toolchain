# Security policy

## Reporting a vulnerability

Report a vulnerability privately, through GitHub's private vulnerability reporting:
<https://github.com/tokeira/dagger-rust-toolchain/security/advisories/new>.

Never report one in a public issue.

Say which release or commit of the module you ran, on which Dagger release, and what
happened.

## Supported versions

The [latest release](https://github.com/tokeira/dagger-rust-toolchain/releases/latest) is
supported. A fix is made there, in a new release.

## Scope

In scope:

- the module's Dang source, the `.dang` files at the repository root;
- the helper, in `helper/`, which a compile lane's step runs;
- the dev module, in `.dagger/modules/dev`, whose checks are the module's acceptance;
- the images and tools that the module and the dev module pin: the Rust and Go images by
  digest, cargo-chef, cargo-nextest, cargo-deny and the Dagger CLI by version and
  checksum, and Dang by commit. A pin that resolves to something other than what it
  should is in scope.
