# rust

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![Dagger v1.0.0-beta.27](https://img.shields.io/badge/Dagger-v1.0.0--beta.27-informational)](https://github.com/dagger/dagger/tree/v1.0.0-beta.27)
[![Written in Dang](https://img.shields.io/badge/written%20in-Dang-informational)](https://github.com/vito/dang)
[![Built with Claude](https://img.shields.io/badge/built%20with-Claude-blueviolet)](#built-with-claude)

A [Dagger](https://dagger.io) module that runs the Rust toolchain's checks over the Cargo
workspaces of a repository: rustfmt, clippy and tests for each package, and cargo check,
cargo-deny and rustdoc for each workspace. It is written in Dang, Dagger's module
language, and takes its shape from [dagger.io/go](https://github.com/dagger/go). It is
maintained by Tokeira, and is not affiliated with Dagger, Inc.

Its slow steps are keyed on their inputs alone and mount no cache volume, so any engine
that has run a step can serve it again: the toolchain install, the crate download and the
dependency build. The lanes that compile the workspace's own packages keep their work in
cache volumes, so that after an edit Cargo compiles only what the edit reaches.

## Status

Every lane is written, and the repository's [checks](#checks) pass on a
`v1.0.0-beta.27` engine, the release the module declares and the only one it is tested
on:

- `fmt`, a generator whose `stale` check is the format check, and the checks `clippy`
  and `test`, for each package;
- the checks `check`, `deny` and `doc`, for each workspace.

Under them each workspace has a toolchain layer, one recipe, one fetch layer and a
dependency layer in two flavours, for `cargo check` and for `cargo test`. A source edit
runs none of them again.

check, clippy, doc and the test build each have two [cache volumes](#cache-volumes) of
their own: a target directory that starts from the dependency layer, and a mirror of the
source. On an engine that holds them, a lane that runs again compiles the packages an
edit reaches and no other. On an engine that does not, it compiles every member once, on
top of the dependency layer, as before.

[Measured](#measured) on one workspace and one machine: after a one-line edit, check,
clippy and doc took 2 to 5 seconds where they took 9 to 11, and the test build took 5
and 54 seconds where it took about 3 minutes.

Not there yet:

- Measurements on Buildkite, a pipeline for this repository, and a release. The
  repository has no remote, so the module can only be installed from a local path.

[docs/lanes.md](docs/lanes.md#known-limits) lists the module's known limits.

## Install

In the `dagger.toml` of a repository that holds Cargo workspaces:

```toml
[modules.rust]
source = "github.com/tokeira/dagger-rust-toolchain@<ref>"

[modules.rust.settings]
fmtToolchain = "nightly-2026-06-16"
clippyArgs = ["-D", "warnings"]
includeExtraFiles = ["proto/**"]
```

Until the repository is published, `source` is a path to a checkout, relative to the
`dagger.toml`, such as `"../dagger-rust-toolchain"`.

```console
$ dagger check -l --all --rust                       # each check, for each workspace and package
$ dagger check --rust                                # every check
$ dagger check --rust --check=clippy                 # clippy, every package
$ dagger check --rust-package=crates/tokeira-types   # every check of one package
$ dagger generate --rust                             # rustfmt, every package
$ dagger call rust workspace --path . fetched        # a layer, to inspect it
```

The module declares `v1.0.0-beta.27`, and is tested on an engine of that release only.

## Workspaces and packages

A workspace is a directory whose `Cargo.toml` has a `[workspace]` table, or a package with
its own `Cargo.lock`. Cargo decides: a candidate is a workspace when `cargo metadata` names
it as its own workspace's root, so a member that keeps a stray `Cargo.lock` is not one.
The member packages come from `cargo metadata --no-deps` too, so Cargo's rules for member
globs, `exclude` and nested packages apply.

Both are collections, keyed by directory relative to the repository root. `"."` is the
repository root.

| Collection | Key | Flag |
| --- | --- | --- |
| `workspaces` | The workspace root, such as `.` or `tools/xtask` | `--rust-workspace` |
| `packages`, of a workspace | The package's directory, such as `crates/tokeira-types` | `--rust-package` |

A command sees the workspaces at or below its working directory, and the one that holds
it. Files that `.gitignore` ignores are not read.

The `workspaceRoots` setting narrows the directories that may be workspace roots, before
Cargo loads any of them. A directory it leaves out is never a workspace, and a Cargo
project there that Cargo cannot load fails nothing. The setting does not choose packages:
a member below a directory it leaves out still belongs to its workspace, and the files of
a workspace it leaves out stay in the source of the workspace that encloses it.

Listing runs `cargo metadata` in the toolchain image and nothing else. It runs over the
manifests and an empty file in place of each source file that makes a Cargo target, so it
runs again when a manifest changes or a target file appears or goes, not when a source
file is edited.

## Checks and the generator

| Function | Kind | Unit | Address |
| --- | --- | --- | --- |
| `fmt` | Generator; its `stale` check is the format check | Package | `rust/workspaces/packages/fmt`, `rust/workspaces/packages/fmt/stale` |
| `clippy` | Check | Package | `rust/workspaces/packages/clippy` |
| `test` | Check | Package | `rust/workspaces/packages/test` |
| `check` | Check | Workspace | `rust/workspaces/check` |
| `deny` | Check | Workspace | `rust/workspaces/deny` |
| `doc` | Check | Workspace | `rust/workspaces/doc` |

### fmt

`fmt` returns the changes that `cargo fmt -p <package>` makes to a package's Rust files,
under the fmt toolchain layer. `dagger generate --rust` writes them to the repository, and
the `stale` check fails with `generated files are not up to date` and the diff when there
are any.

Each package has a step of its own, over the manifests, the set of target paths, the
package's Rust files and the rustfmt configuration that applies to them: `rustfmt.toml`
or `.rustfmt.toml` in the package and in each directory above it, up to the repository
root. An edit to one package formats that package again and no other. A manifest edit
formats every package of the workspace again. The step downloads no crate.

A package's files are those in its directory, without the packages nested below it.
rustfmt follows `mod` declarations from the package's targets, so a module file outside
the package's directory, reached with `#[path]`, is not there and fails the step.

### clippy

All the selected packages of a workspace share one command, which starts from the check
flavour of the [dependency layer](#layers):

```console
$ cargo clippy <packages> --all-targets --keep-going --message-format=json <settings' flags> -- --no-deps <clippyArgs>
```

`<packages>` is `--workspace`, or `-p <name>` for each package when the selection or the
`clippy` setting leaves some out. Each package's check then reads its own share:

- A package fails with the errors clippy reports in it. Warnings fail nothing, unless
  `clippyArgs` or the workspace's own lint policy denies them.
- A package that needs a member whose library did not compile is `not checked: blocked by
  <member>`.
- `--keep-going` lets every other package finish, and pass.
- When clippy fails and no error in a member explains it, every package that the run did
  not clear is `not checked`.

clippy's `--no-deps` keeps the lints to the packages the command names. A member that is
compiled only because a named package needs it is not linted.

### test

Each package's tests run in a step of their own, under cargo-nextest, and then its
doctests. [Tests](#tests) has the steps.

### check

clippy compiles every target under the settings' features and reports what `cargo check`
would. The check lane covers the two ends of the feature space that clippy does not reach:

```console
$ cargo check --workspace <--locked, --profile, --target> --no-default-features
$ cargo check --workspace <--locked, --profile, --target> --all-features
```

An end is left out when the settings already put clippy there: `--all-features` under
`allFeatures`, and `--no-default-features` under `noDefaultFeatures` with no `features`.

### deny

`deny` runs `cargo deny check` in the workspace root when the root has a `deny.toml`, and
passes without running anything when it has none. It fails with
`rust: cargo deny check failed in <root>:` and cargo-deny's report.

- cargo-deny is release 0.19.9, from the project's release archive for `linux/amd64` or
  `linux/arm64`, which the engine checks against a pinned SHA-256 digest.
- It runs off the network. The crates come from the fetch layer. The advisory database is
  `https://github.com/RustSec/advisory-db`, fetched by the engine at the commit its default
  branch is at when the check runs, and never pinned in `dagger.lock`. A `deny.toml` that
  names another database with `db-urls` or `db-path` is not supported.
- The step is keyed on the toolchain layer, the fetch layer, cargo-deny, the database's
  commit, and the workspace without the contents of its Rust files. It runs again when
  `deny.toml`, a manifest, `Cargo.lock` or another file that is not Rust source changes,
  and when RustSec publishes. An edit to a Rust file leaves it cached.
- `locked` passes `--locked`, and `target`, `features`, `allFeatures` and
  `noDefaultFeatures` pass cargo-deny's flags of the same names, which choose the
  dependency graph it checks over what `deny.toml` says.
- Whether a crate is yanked is read from the registry index that the fetch layer
  downloaded, so it is as old as that layer.

### doc

```console
$ RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps <settings' flags>
```

A warning fails the check, with rustdoc's message.

## Settings

Settings are the constructor's arguments, set under `[modules.rust.settings]`.

| Setting | Type | Default | Meaning |
| --- | --- | --- | --- |
| `image` | string | `rust:1.100.0-bookworm`, pinned by the digest of its multi-platform index | Toolchain image. |
| `base` | container | none | Replaces `image`, for system dependencies or for services that tests need. It must carry rustup and Cargo. |
| `toolchain` | string | The workspace's `rust-toolchain.toml` or `rust-toolchain`, otherwise the image's toolchain | Rust toolchain for every workspace. |
| `fmtToolchain` | string | `toolchain` | Rust toolchain for rustfmt. |
| `systemPackages` | list | `[]` | Debian packages that build scripts need, installed in the toolchain layer. |
| `features` | list | `[]` | Passed to every Cargo command as `--features`. |
| `allFeatures` | bool | `false` | Pass `--all-features`. |
| `noDefaultFeatures` | bool | `false` | Pass `--no-default-features`. |
| `buildProfile` | string | Cargo's default | Passed as `--profile`. |
| `target` | string | Cargo's default | One target triple, passed as `--target`. The toolchain layer adds its standard library. |
| `locked` | bool | `true` | Pass `--locked`. |
| `volumePrefix` | string | The repository's name | Start of the name of every [cache volume](#cache-volumes). A workspace that is not at the repository root adds its path. A new value gives every lane empty volumes. |
| `test` | patterns | `["**"]` | Packages to test. |
| `clippy` | patterns | `["**"]` | Packages to lint. |
| `clippyArgs` | list | `[]` | Arguments after `--`, such as `["-D", "warnings"]`. |
| `testRunner` | `"nextest"` or `"cargo"` | `"nextest"` | Test runner: cargo-nextest, or the test binaries run as `cargo test` runs them. |
| `includeExtraFiles` | patterns | `[]` | Files outside the workspace that its build reads, such as `proto/**`, as patterns relative to the repository root. A package among them can be a path dependency. |
| `mountPath` | path | `/src/<workspace name>` | Where the repository is mounted. A fixed path keeps compiled outputs the same on every engine. |
| `workspaceRoots` | patterns | `["**"]` | Directories that may be workspace roots, relative to the repository root, such as `["**", "!fixtures"]`. |

A pattern of `test` or `clippy` selects a package directory and everything below it.
`**` selects every package, a pattern that starts with `!` excludes, and an exclusion wins
whatever the order. A package that the setting leaves out keeps its check, which passes
without running. A pattern of `workspaceRoots` selects workspace roots by the same rules.

## Layers

Each workspace has five layers. `dagger call rust workspace --path <root> <layer>`
returns one as a container.

| Layer | What runs | Keyed on |
| --- | --- | --- |
| `toolchain` | The image, `systemPackages`, then `rustup toolchain install`, the `target` and the components a lane asks for | The image, `systemPackages`, the toolchain files or the `toolchain` setting, `target`, the components |
| `fmt-toolchain` | The same under `fmtToolchain`, with rustfmt | The same, with `fmtToolchain` |
| `fetched` | `cargo fetch` over the recipe, in the toolchain layer, with the crates left in Cargo's home. Every lane that needs crates takes them from here | The toolchain layer, the recipe and `locked` |
| `check-dependencies` | On the fetch layer, `cargo check` over the recipe, once for each shape of command that clippy, doc and check run. The workspace's dependencies are left compiled in `/var/cache/rust/target-check`, and nothing of its members | The fetch layer, and the settings `features`, `allFeatures`, `noDefaultFeatures`, `buildProfile`, `target` and `locked` |
| `test-dependencies` | `cargo build --tests` over the recipe, with the fetch layer's crates. The workspace's dependencies are left compiled in `/var/cache/rust/target-test`, and nothing of its members | The toolchain layer, the fetch layer, the recipe and the same settings |

None mounts a cache volume. Editing a source file re-executes none of them. Any engine
cache can serve them, and they travel to another engine with it.

The recipe (`dagger call rust workspace --path <root> recipe`) is the workspace as the
fetch layer and the dependency layers see it: the members' manifests with their versions
masked and their lint tables dropped, `Cargo.lock` with the same versions masked, the
Cargo configuration, and a stub that compiles in place of each target's source.
[cargo-chef](https://github.com/LukeMathWalker/cargo-chef) 0.1.78 writes it, pinned by
checksum for `linux/amd64` and `linux/arm64`. A workspace has one recipe and one fetch,
so a cold run downloads each crate once.

The recipe is keyed on what it says. A manifest edit writes it again. When it comes out
the same, as after a release that only changes a member's version, or after an edit to a
comment, the fetch layer and the dependency layers stay cached. When it differs, as after
a change to a dependency, a feature or a target, they run again.

A lane keeps Cargo's failure as its result: the same source fails the same way without
running again. When a signal stopped a compiler or a build script, as the kernel stops a
process that runs out of memory, the lane runs its command once more, and does not keep
a second failure.

A `base` container needs `tar` and `xz` too, to unpack cargo-chef.

## Cache volumes

check, clippy, doc and the test build compile the workspace's own packages, each with
one Cargo command. A lane runs again when a file of the workspace changes. So that it
then compiles only what changed, each of the four keeps two cache volumes of its own:

| Volume | Holds |
| --- | --- |
| `<volumePrefix>-target-check`, `-target-clippy`, `-target-doc`, `-target-test` | The lane's target directory. It starts from the compiled dependencies of the lane's dependency layer, and the lane adds what it compiles of the members. |
| `<volumePrefix>-source-check`, `-source-clippy`, `-source-doc`, `-source-test` | A mirror of the workspace's files, where Cargo runs. |

`<volumePrefix>` is the `volumePrefix` setting, or the repository's name, and then the
workspace's root when it is not the repository's: `tokeira-crates-core-target-check`.
There is no volume for the crates: they are in the `fetched` layer, which travels.

How they work:

- **A lane's volumes are its own.** They are `LOCKED`: one step uses a volume at a time.
  No lane shares one, so the four lanes run side by side.
- **The target volume follows the dependency layer.** A new volume starts from the
  layer's compiled dependencies. When the layer changes, as after a
  change to `Cargo.lock`, the toolchain or a feature setting, the lane gets a new volume
  that starts from the new layer, and compiles every member once. When it changes back,
  as on another branch, the lane gets the earlier volume back.
- **The mirror keeps each file's time.** Cargo decides by modification time what to
  compile again. Before Cargo runs, the lane makes its mirror equal to the workspace's
  files by comparing content: a file that is the same keeps its time, a file that
  differs is written again, and a file that is gone is removed.
- **rustdoc runs again for every package.** Cargo does not track the files rustdoc
  reads, so the doc lane documents every package whenever a file changed. It still
  compiles only the packages the edit reaches.
- **clippy has a target volume for each thing it is asked.** What clippy reports
  depends on the packages its command names and on `clippy.toml`, and Cargo tracks
  neither. A clippy command over other packages, as with `--rust-package`, or under
  another `clippy.toml` or other `clippyArgs`, gets a target volume of its own under
  the same name.
- **One step does it all.** The engine keys a step on its inputs, never on what a volume
  holds. A lane therefore syncs its mirror, runs Cargo, and copies out what later steps
  read, such as the test binaries, in a single step. A small program does this, built
  from the module's own `helper` directory. The module runs no shell.
- **Unchanged inputs run nothing.** With the same files and settings a lane's step is
  answered from the engine's cache, as every other step is, and no volume is touched.
- **A volume changes speed, never a verdict.** Over empty volumes a lane compiles every
  member on top of the dependency layer, and reports what it reports over warm ones.
  [docs/lanes.md](docs/lanes.md#known-limits) lists the cases where Cargo itself cannot
  tell that a file changed.

A cache volume stays on the engine that wrote it. On Buildkite agents hosted by
Namespace, the engine's state is a Namespace cache volume, which is local to a node. The
module was not run there yet. From how the volumes work, and from what Namespace says of
its cache:

- a job on a node whose latest cache generation holds the volumes compiles only what
  changed since that generation;
- a job on a node with an older generation finds older volumes. The mirror sync brings
  the source up to date, and Cargo compiles what differs since then;
- a job on a node with no generation, or after the engine collected the volumes,
  starts from the dependency layer, which the same job may also have to build;
- Namespace commits a job's cache only when the job exits 0, so a failed build leaves
  the volumes as the last green build left them.

The engine counts cache volumes against its storage limit, and collects them like any
other cached data. Each dependency layer has target volumes of its own, so the volumes
of a `Cargo.lock` that is no longer in use stay until the engine collects them.

Two repositories with the same name, or two checkouts of one, share volumes on an
engine unless `volumePrefix` tells them apart. That costs time, not correctness: each run
then compiles what differs from the run before it.

`compiled` returns a lane's step, and the step's report says what it did. The report
helps inspect a lane: its format may change between releases, and no check's verdict
depends on it. After an edit to one file, for example:

```console
$ dagger call rust workspace --path . compiled --lane clippy file --path /var/cache/rust/report contents
run cargo clippy --quiet -p ... --all-targets --keep-going --message-format=json --locked -- --no-deps
mirror wrote 1 removed 0 kept 412
wrote crates/tokeira-types/src/lib.rs
compiled tokeira-types
compiled tokeira-server
exit 0
```

### Measured

One workspace, one machine, one run of each:
[tokeira/dagger-rust-sdk](https://github.com/tokeira/dagger-rust-sdk) at v0.6.0 (five
members, 276 locked packages, Rust 1.99.1), on a `v1.0.0-beta.27` engine in Docker on an
Apple-silicon Mac, with `workspaceRoots = ["."]`. "Before" is the module without
volumes, and "after" the module with them. Each figure is the wall time of one `dagger
check --check=<lane>`, in seconds, with the layers already built.

| | check | clippy | doc | test |
| --- | --- | --- | --- | --- |
| First run, empty volumes: before | 11.9 | 12.2 | 8.5 | 214 |
| First run, empty volumes: after | 11.7 | 11.6 | 7.6 | 549 |
| Again, nothing changed: before | 2.2 | 1.0 | 1.3 | 1.7 |
| Again, nothing changed: after | 1.7 | 0.8 | 0.8 | 1.3 |
| One line in a package nothing needs: before | 11.3 | 11.3 | 9.0 | 351 |
| One line in a package nothing needs: after | 2.7 | 1.8 | 3.7 | 99 |
| One line in a package two others need: before | 10.2 | 10.8 | 9.0 | 455 |
| One line in a package two others need: after | 3.6 | 3.9 | 4.6 | 346 |

The test check's time is mostly the tests themselves, which the volumes do not change:
this workspace's slowest package runs for four to six minutes. The test build alone,
by its step's own log:

| | Before | After |
| --- | --- | --- |
| First run, empty volumes | 3m12s | 3m07s |
| One line in a package nothing needs | 3m25s | 5s |
| One line in a package two others need | 2m44s | 54s |

- Over empty volumes a lane costs what it cost before: the engine starts the target
  volume from the dependency layer without copying it.
- The first "after" test check ran every package's tests, and the first "before" one
  found them in the engine's cache from an earlier run of the same binaries. That
  accounts for 549 against 214.
- The engine reached its storage limit during these runs and collected cached results,
  among them test runs that the last "after" check then ran again.
- With the layers not built yet, the first check lane took 58 seconds.

## Tests

`dagger check --check=test` runs each package's tests, then its doctests. Five steps do
it for a workspace:

| Step | What runs | Runs again after a source edit |
| --- | --- | --- |
| Runner | Nothing: the engine fetches cargo-nextest 0.9.151 from its GitHub release, verifies the archive's checksum for `linux/amd64` or `linux/arm64`, and unpacks it | No |
| Dependency layer, test flavour | `cargo build --tests --workspace --profile test` over the recipe, then `cargo clean -p` for each of the workspace's own packages | No |
| Test build | `cargo nextest list --list-type binaries-only` over the selected packages, in the lane's [cache volumes](#cache-volumes) | Yes. Over warm volumes it compiles the packages the edit reaches |
| Run, one for each package | `cargo nextest run` over the package's test binaries and its own files | Only when the package's binaries or files changed |
| Doctests | `cargo test --doc` over the selected packages, in the test build's container | Yes |

The dependency layer compiles the workspace's dependencies as `cargo test` builds them.
It is keyed on the toolchain layer, the fetch layer, the recipe, and the settings
`locked`, `buildProfile`, `target`, `features`, `allFeatures` and `noDefaultFeatures`.
`dagger call rust workspace --path <root> test-dependencies` returns it as a container,
with the compiled dependencies under `/var/cache/rust/target-test`.

The test build runs once for all the packages of a selection: with `--workspace` when
every package is selected, and otherwise with `-p` for each one. It builds with
`CARGO_INCREMENTAL=0` at the mount path, so that a package whose code did not change
has the same binaries, byte for byte, whether its volumes were warm or empty. The step
copies the binaries out of its target volume, and the runs read that copy: no run mounts
a volume.

A package's run is keyed on the content of its binaries and of its files. After an edit
to one package, the runs of the packages that do not link it are answered from the cache
on any engine that has run them. The run sees:

- the package's own directory, without the directories of members below it;
- every manifest of the workspace, with an empty file in place of each other source file;
- the workspace's `.config/nextest.toml`, and the files of `includeExtraFiles`.

A test that reads a file of another package finds it only if `includeExtraFiles` names
it.

rustdoc compiles doctests when they run, against the libraries in the test build's
target directory. They therefore run in one step for all the selected packages, which
runs again whenever the test build does. Each package's check reads its own share.

With `testRunner = "cargo"` the test build is `cargo test --no-run`, and a package's run
executes each of its test binaries in the package's directory, as `cargo test` does.
`CARGO_MANIFEST_DIR` and `CARGO_PKG_NAME` are set, and the toolchain's libraries are on
the loader's path. The other variables that `cargo test` sets for a test are not.

A failed test or doctest fails its own package's check, with the runner's report. A
compile error fails the check of every selected package: cargo-nextest and `cargo test`
have no `--keep-going`. So does a doctest step that fails without saying in which package:
a package passes it only when Cargo names another package for every failure.

## Engine version

`dagger-module.toml` declares `engineVersion = "v1.0.0-beta.27"`, the one release the
module is tested on. The engine gives a module the API and the naming rules of the
release it declares.

The module was not run on any other release. As the engine's source has it at
`v1.0.0-beta.27`, an engine of an earlier beta does not refuse a module that declares a
later one: it gives the module its own, older API. Whether the module loads there was
not tried.

From `v1.0.0-beta.26` the engine spells names that contain acronyms by new rules. No
type, function or argument of the module contains one, so its flags and check addresses
are spelled as the source has them.

The module has no `dang.toml`, as dagger.io/go has none. The engine then reads its source
in the current language version of the Dang it embeds. The `v1.0.0-beta.27` engine embeds
Dang's library at a pre-release of v2.3.1, commit `00dc3801b409` of
[github.com/vito/dang](https://github.com/vito/dang), and the language version of that
commit is 2.1. The [format check](#format) builds `dang` from the same commit, so
it formats the language the engine reads.

## Development

The module's source is the `.dang` files at the repository root.
[docs/lanes.md](docs/lanes.md) describes the layout and the contract that a lane follows.

The dev module, `.dagger/modules/dev`, is written in Dang, like the module: the engine
interprets it, and nothing is compiled before a check starts. It is the workspace's
main module, so its checks and its generator are the repository's own, and their
addresses carry no module name:

```console
$ dagger check                       # every check, side by side
$ dagger check -l                    # the checks, each with what it covers
$ dagger check --check=test-lane     # one check
$ dagger generate                    # format the Dang source
```

They need the `v1.0.0-beta.27` CLI and an engine of that release, and network access to
Docker Hub, dl.dagger.io, static.rust-lang.org, crates.io, deb.debian.org, github.com
and proxy.golang.org.

### Checks

Each check copies fixtures under `fixtures/` into repositories of their own, installs
the module there, and drives it with the Dagger CLI, as a repository that uses the
module would. It fails with the first assertion that does not hold, and the assertion's
message.

| Check | Covers |
| --- | --- |
| `single-package` | A package at the repository root: its workspace and package keys, the check addresses, and the effect of every setting. |
| `workspace-members` | A workspace of several members: its keys; the `clippy` and `test` settings; the toolchain, fmt toolchain and fetch layers; that a second run and a source edit re-execute no layer step, and that a manifest that gains a feature re-executes the recipe and the fetch, and not the toolchain install; that listing runs the skeleton's commands and `cargo metadata`, and no other container command. |
| `two-workspaces` | A repository of two workspaces: their keys, selection by flag and by working directory, the default volume prefixes, `includeExtraFiles`, and that one workspace's manifests are no input of the other's layers. |
| `nested-workspaces` | A workspace in a directory of another: their keys, and that each one's inputs leave out the other's files. |
| `lint-lanes` | That `workspaceRoots` keeps Cargo from loading a root it leaves out; what the fmt lane's `stale` check and `dagger generate` do, and that an edit to one package formats no other; when the deny lane runs cargo-deny, that it reads the advisory database, and which edits run it again. |
| `build-lanes` | What the check, clippy and doc lanes report over the `build-` fixtures, package by package for clippy; that the check flavour of the dependency layer holds the dependencies compiled and nothing of the members, that a source edit or a member's release bump re-executes none of its steps, and that a manifest that gains a feature re-executes them. |
| `test-lane` | That the archives of cargo-nextest for `linux/amd64` and `linux/arm64` have the pinned checksums; what the test lane runs, what each kind of edit runs again, how it reports a failing test, a failing doctest and a compile error, and the effect of `test`, `testRunner` and `locked` on it; that a failing doctest fails its check in a workspace with a single member, where Cargo names no package. |
| `cache-volumes` | That the helper's own tests pass; which cache volumes each compile lane mounts; what a lane compiles over empty volumes, after an edit to one file, and after a change to the dependency layer; that a deleted file is gone from the build; that test binaries of untouched packages stay the same, byte for byte; that every check's verdict is the same over warm volumes and over empty ones, through an error, its correction and the error again. |
| `shared-layers` | That the six lanes of one workspace run over one recipe and one `cargo fetch`. |
| `dang-format/stale` | That the Dang source is formatted: the module's, and the dev module's. It is the `stale` check of the generator `dang-format`. |

The checks are independent. Each passes alone, on an engine that has run nothing, and
beside the others, in any order:

- **Volumes.** A check gives the module a `volumePrefix` of its own in every repository
  it makes, `<check>-<fixture>`, so no two checks mount the same cache volume.
  `cache-volumes` takes a new prefix on every run, so that its volumes start empty, and
  leaves them on the engine until it collects them.
- **Steps.** Where a check asserts that a step runs, the step reads a value of that run,
  so no earlier run and no other check has run it. Where it asserts that a step is
  answered from the cache, the same check ran the step before.

No check is answered from the engine's cache: each is declared
`@cache(policy: FunctionCachePolicy.NEVER)`, and runs whenever it is asked for. A
check's verdict depends on more than the files it is given. It asserts what the engine
in hand does, step by step, and `lint-lanes` and `shared-layers` read RustSec's
advisory database as it is when they run. A verdict kept from an earlier run would say
nothing of either.

`lint-lanes` fails if RustSec withdraws the advisory its fixture relies on,
RUSTSEC-2020-0095, or publishes between two runs that the check expects to share a
cached step.

Wall time in seconds, on a `v1.0.0-beta.27` engine in Docker on an Apple-silicon Mac
(10 CPUs and 9.8 GiB of memory for the Docker VM). "Cold" is after `dagger core engine
local-cache prune`, which empties the engine's cache, before each figure. "Warm" is on
an engine that has run every check. A check alone was run once:

| | Cold | Warm |
| --- | --- | --- |
| `single-package` alone | 29 | 8 |
| `workspace-members` alone | 46 | 11 |
| `two-workspaces` alone | 20 | 6 |
| `nested-workspaces` alone | 17 | 3 |
| `lint-lanes` alone | 32 | 14 |
| `build-lanes` alone | 69 | 24 |
| `test-lane` alone | 52 | 38 |
| `cache-volumes` alone | 58 | 48 |
| `shared-layers` alone | 20 | 2 |
| Every check at once, with `dagger check` | 131 and 160 | 76 to 181, in eight runs |
| Every check, one at a time, with `dagger check -j 1` | not measured | 189 to 384, in three runs |

- On a cold engine a check starts at once: the engine interprets the dev module, and
  `dagger check -l` answers within a second. A cold run spends its time on what the
  checks pull, download and build.
- At once, the checks keep the ten CPUs busy, and the times vary with it. Most of the
  work is the engine's own, in the `dagger` sessions that the checks start. The
  engine's memory stayed below 4.3 GiB.
- `dagger check -j <n>` runs at most `n` checks at once. On a machine that all of them
  at once overload, the engine can close a session whose client it has not heard from
  for a few seconds, and the check that started the session fails with `engine closed
  the session`. That happened to one check, in one of eleven runs of every check at
  once, on an engine that was also at its storage limit.
- A run of every check leaves about 2 GB on the engine, in cached steps and in the
  volumes of `cache-volumes`. An engine at its storage limit collects them while the
  checks run: every check at once then took 209 and 242 seconds warm.

After a change to a `.dang` file, the module's or the dev module's, run `dagger
generate` before `dagger check`: it formats the file, which `dang-format/stale` would
otherwise fail. The engine reads the dev module's source as it is, so there is nothing
to compile and no generated file.

The helper is a Rust crate with no dependency, in `helper/`. Its tests run with `cargo
test` there, and `cache-volumes` runs them too.

### Format

`dang fmt` formats the Dang source. The generator `dang-format` returns the changes that
`dang fmt -w` makes to the `.dang` files at the repository root and to those of the dev
module. `dagger generate` writes them, and the check `dang-format/stale` fails with the
diff when there are any.

`dang` is built from Dang's source, at the commit that the `v1.0.0-beta.27` engine
embeds, in a Go image pinned by the digest of its multi-platform index. The engine
fetches the commit by its full name, and Go checks every dependency against the
`go.sum` of that commit.

## Built with Claude

The overwhelming majority of contributions to this module come from
[Claude](https://www.anthropic.com/claude), the AI model made by
[Anthropic](https://www.anthropic.com), working through
[Claude Code](https://claude.com/claude-code). The maintainer directs the work, reviews
every change, and approves every merge and release.

Commits credit every agent that took part with a `Co-authored-by` or `Assisted-by`
trailer, such as `Co-authored-by: Claude <noreply@anthropic.com>`, and pull requests name
them, as [`AGENTS.md`](AGENTS.md) sets out.

## Acknowledgements

[dagger.io/go](https://github.com/dagger/go), Dagger's modules for Go, gave this module
its shape: workspaces and packages as collections, a check for each package, and the
selection patterns of its settings. Some of its helpers are adapted here, as
[NOTICE](NOTICE) records.

The source mirror, which lets a compile lane rebuild only what changed, follows the
approach of Guillaume de Rouville's Rust module for Dagger,
[grouville/dagger-rust](https://github.com/grouville/dagger-rust).

The module runs [cargo-chef](https://github.com/LukeMathWalker/cargo-chef),
[cargo-nextest](https://nexte.st) and
[cargo-deny](https://github.com/EmbarkStudios/cargo-deny), each pinned by version and
checksum, and checks dependencies against the
[RustSec advisory database](https://rustsec.org).

## Licence

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). Some helpers and setting
descriptions, and the way the module builds its helper, are adapted from
[dagger.io/go](https://github.com/dagger/go), as NOTICE records.
