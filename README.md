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

## Install

In the `dagger.toml` of a repository that holds Cargo workspaces:

```toml
[modules.rust]
source = "github.com/tokeira/dagger-rust-toolchain@v0.2.0"

[modules.rust.settings]
fmtToolchain = "nightly-2026-06-16"
includeExtraFiles = ["proto/**"]
```

The module needs Dagger `v1.0.0-beta.27` or later:
[Supported engines](#supported-engines) has the policy.

```console
$ dagger check -l --all --rust                       # each check, for each workspace and package
$ dagger check --rust                                # every check
$ dagger check --rust --check=clippy                 # clippy, every package
$ dagger check --rust-package=crates/tokeira-types   # every check of one package
$ dagger generate --rust                             # rustfmt, every package
$ dagger call rust workspace --path . fetched        # a layer, to inspect it
```

[Large workspaces](#large-workspaces) has the settings for a workspace of many packages.

## First run

The first `dagger check --rust` builds each workspace's [layers](#layers): it installs
the toolchain, downloads the crates and compiles the dependencies. Later runs reuse them,
and compile only what changed since.

### The settings most repositories change

| Setting | When to change it |
| --- | --- |
| [`clippyArgs`](#clippy) | clippy denies warnings by default. Set `clippyArgs = []` to allow them. |
| [`checkAllFeatures`, `checkNoDefaultFeatures`](#check) | Both are on by default, so `check` builds the workspace under `--all-features` and under `--no-default-features`. Turn one off where that build turns on a feature that cannot build in a plain container. tokeira/tokeira turns both off. |
| [`wholeWorkspace`](#one-item-for-the-workspace) | Turn it on to have one clippy check and one test check for each workspace, so that the check names stay the same as crates come and go. |
| [`workspaceRoots`](#workspaces-and-packages) | Leave out fixtures and vendored workspaces, as `["**", "!fixtures"]` does. |
| [`includeExtraFiles`](#settings) | The build reads files outside the workspace. |
| [`systemPackages`](#settings) | The build needs Debian packages. |
| [`cargoEnvironment`](#fewer-builds-fewer-jobs) | The build or the tests need environment variables. |
| [`fmtToolchain`](#settings) | A `rustfmt.toml` uses nightly options. |

[Settings](#settings) has every setting.

### Gating merges on Dagger Cloud

With [Cloud Checks](#on-ci) on, each check has a commit status of its own.

- Require each check by its status name.
- Never require the `Run Dagger Checks` total: it can be green while a check fails.
- Copy the exact names from a commit's statuses, because a name can carry its trigger: a
  push to a branch posts `rust/workspaces/check (push)`, where a push to main posts
  `rust/workspaces/check`.

## Supported engines

The module declares `engineVersion = "v1.0.0-beta.27"`, and needs Dagger `v1.0.0-beta.27`
or later. Dagger's stable line, v0.21.x, cannot run it: an engine refuses a module that
declares a later release than its own.

- **Supported** means that the module's own checks passed on that engine. Today that is
  `v1.0.0-beta.27` alone.
- **A new beta.** When Dagger releases one, the module's checks run on it before this
  section lists it as supported. A beta that breaks the module gets a fix in a new
  release of the module.
- **`engineVersion`** is raised only in a minor release, and that release's notes say so.
- **Release notes.** Each release's notes name the betas it was tested on.

[CONTRIBUTING.md](CONTRIBUTING.md#testing-a-new-dagger-beta) says how to run the checks
on a new beta.

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

Under the [`wholeWorkspace`](#large-workspaces) setting a workspace's `packages` has one
item, keyed by the workspace's root, which stands for every member.

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
| `advisories` | Check | Workspace | `rust/workspaces/advisories` |
| `doc` | Check | Workspace | `rust/workspaces/doc` |

Under the [`wholeWorkspace`](#large-workspaces) setting the unit of `fmt`, `clippy` and
`test` is the workspace: each is there once, for the item at the workspace's root.

[docs/lanes.md](docs/lanes.md#known-limits) lists the module's known limits.

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

- A package fails with the errors clippy reports in it. Under the default `clippyArgs`,
  `["-D", "warnings"]`, each warning is an error. Under `clippyArgs = []` a warning fails
  nothing, unless the workspace's own lint policy denies it.
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
The settings `checkNoDefaultFeatures` and `checkAllFeatures`, both on by default, each
turn one end off: for a workspace whose features were never all compiled together, say.

When no end is left, the lane runs one command under the settings' own features:

```console
$ cargo check --workspace <settings' flags>
```

A `check` that passed has therefore always compiled the workspace. To have no `check` at
all, skip it in `dagger.toml`, as [Large workspaces](#large-workspaces) shows.

### deny

`deny` runs `cargo deny check bans licenses sources` in the workspace root when the root
has a `deny.toml`, and passes without running anything when it has none. It fails with
`rust: cargo deny check bans licenses sources failed in <root>:` and cargo-deny's report.

- cargo-deny is release 0.19.9, from the project's release archive for `linux/amd64` or
  `linux/arm64`, which the engine checks against a pinned SHA-256 digest.
- It runs off the network, and reads no advisory database. The crates come from the fetch
  layer.
- The step is keyed on the toolchain layer, the fetch layer, cargo-deny, and the workspace
  without the contents of its Rust files. It runs again when `deny.toml`, a manifest,
  `Cargo.lock` or another file that is not Rust source changes. An edit to a Rust file
  leaves it cached, and so does anything that RustSec publishes: only a change to the
  repository changes its verdict.
- `locked` passes `--locked`, and `target`, `features`, `allFeatures` and
  `noDefaultFeatures` pass cargo-deny's flags of the same names, which choose the
  dependency graph it checks over what `deny.toml` says.

### advisories

`advisories` runs `cargo deny check advisories`, cargo-deny's fourth check, where `deny`
applies and with the same flags. It fails with
`rust: cargo deny check advisories failed in <root>:` and cargo-deny's report.

- The advisory database is `https://github.com/RustSec/advisory-db`, fetched by the engine
  at the commit its default branch is at when the check runs, and never pinned in
  `dagger.lock`. A `deny.toml` that names another database with `db-urls` or `db-path` is
  not supported.
- The step is keyed on what `deny` is keyed on, and on the database's commit. So it runs
  again whenever RustSec publishes, and a new advisory against a dependency fails it with
  no change to the repository. It is a check of its own for that reason: a repository
  can require `deny` to pass, and watch `advisories`.
- Whether a crate is yanked is part of this check. It is read from the registry index
  that the fetch layer downloaded, so it is as old as that layer.

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
| `checkAllFeatures` | bool | `true` | Build the workspace under `--all-features` in the [check lane](#check). |
| `checkNoDefaultFeatures` | bool | `true` | Build the workspace under `--no-default-features` in the check lane. |
| `buildProfile` | string | Cargo's default | Passed as `--profile`. |
| `target` | string | Cargo's default | One target triple, passed as `--target`. The toolchain layer adds its standard library. |
| `locked` | bool | `true` | Pass `--locked`. |
| `buildJobs` | integer | Cargo's choice, one job for each processor | Cargo's job count, as `CARGO_BUILD_JOBS`, in every step that compiles. `0` is refused. |
| `cargoEnvironment` | list | `[]` | Environment variables, each as `NAME=value`, for the steps that compile the workspace or run its tests, such as `["CI=1"]`. Never a secret: see [Large workspaces](#large-workspaces). |
| `volumePrefix` | string | The repository's name | Start of the name of every [cache volume](#cache-volumes). A workspace that is not at the repository root adds its path. A new value gives every lane empty volumes. |
| `wholeWorkspace` | bool | `false` | Treat each workspace's packages as one item: one `fmt`, one `clippy` and one `test` for the workspace, and its tests run together over a writable copy of the whole repository. See [Large workspaces](#large-workspaces). |
| `test` | patterns | `["**"]` | Packages to test. |
| `clippy` | patterns | `["**"]` | Packages to lint. |
| `clippyArgs` | list | `["-D", "warnings"]` | Arguments after `--`. The default denies warnings, and `[]` allows them. |
| `testRunner` | `"nextest"` or `"cargo"` | `"nextest"` | Test runner: cargo-nextest, or the test binaries run as `cargo test` runs them. |
| `includeExtraFiles` | patterns | `[]` | Files outside the workspace that its build reads, such as `proto/**`, as patterns relative to the repository root. A package among them can be a path dependency. |
| `mountPath` | path | `/src/<workspace name>` | Where the repository is mounted. A fixed path keeps compiled outputs the same on every engine. |
| `workspaceRoots` | patterns | `["**"]` | Directories that may be workspace roots, relative to the repository root, such as `["**", "!fixtures"]`. |

A pattern of `test` or `clippy` selects a package directory and everything below it.
`**` selects every package, a pattern that starts with `!` excludes, and an exclusion wins
whatever the order. A package that the setting leaves out keeps its check, which passes
without running. A pattern of `workspaceRoots` selects workspace roots by the same rules.
The exception is `.`, the repository root, which selects the root alone and nothing below
it, so `workspaceRoots = ["."]` checks the root workspace only. Write other patterns
without a leading `./`: such a pattern matches no directory.

## Large workspaces

Five settings are for a workspace of many packages, or one whose tests do not stay in
their own package:

```toml
[modules.rust.settings]
wholeWorkspace = true           # one fmt, one clippy and one test for the workspace
checkAllFeatures = false        # no --all-features build in the check lane
checkNoDefaultFeatures = false  # no --no-default-features build either
buildJobs = 1                   # CARGO_BUILD_JOBS, where memory is short
cargoEnvironment = ["CI=1"]     # for build scripts and tests that read it
```

### One item for the workspace

A check has a name for each item it runs on: `clippy` and `test` have one for each
package, and so has `fmt`'s `stale`. A workspace of forty packages then has a hundred
and twenty of them, and the list changes whenever a package is added. That is unwieldy
where checks are required by name.

Under `wholeWorkspace = true` a workspace's `packages` has one item, keyed by the
workspace's root, and that item stands for every member:

```console
$ dagger check -l --all -f=link --rust
dag+check://?check=advisories&rust-workspace=.
dag+check://?check=check&rust-workspace=.
dag+check://?check=clippy&rust-workspace=.&rust-package=.
dag+check://?check=deny&rust-workspace=.
dag+check://?check=doc&rust-workspace=.
dag+check://?check=stale&rust-workspace=.&rust-package=.
dag+check://?check=test&rust-workspace=.&rust-package=.
```

- **`fmt`** is one generator. It formats each member in a step of its own, as before, and
  returns their changes as one changeset.
- **`clippy`** is one check over the command that the packages' checks share. It fails
  with `rust: clippy failed in <root>:` and, under each package's directory, what that
  package's own check would say.
- **`test`** is one check, and its tests run together: [Tests](#whole-workspace-tests)
  has the steps.
- **The `clippy` and `test` settings still choose the packages.** A command names every
  member with `--workspace`, or the selected ones with `-p` when a setting leaves some
  out. When it leaves them all out, the check passes without running. `--rust-package`
  selects the one item or nothing.

The engine lists a module's checks from its types, so a setting cannot take a check off
the list: it can only change what a collection holds. That is why the mode is one item in
`packages`, and why `fmt` changes with `clippy` and `test`.

### Fewer builds, fewer jobs

- **`checkAllFeatures` and `checkNoDefaultFeatures`** each turn off one of the check
  lane's two builds. With both off, the lane runs one `cargo check --workspace` under the
  settings' features: [check](#check) has the reason.
- **To have no `check` at all**, or no other check, skip it with Dagger's own setting,
  beside the module's `source`. A check that is skipped is not listed and does not run:

  ```toml
  [modules.rust]
  check.skip = ["workspaces/check"]
  ```

- **`buildJobs`** sets `CARGO_BUILD_JOBS` in the steps that compile: the dependency
  layers, the compile lanes and the doctests. Every Cargo that such a step starts reads
  it: cargo-nextest's, and under `wholeWorkspace` one that a test runs. A change of the
  count compiles the dependency layers again, and leaves the toolchain layer, the recipe
  and the fetch layer cached.
- **`cargoEnvironment`** gives variables to the steps that compile the workspace or run
  its tests. A lane sets its own variables after them, so they cannot replace
  `CARGO_TARGET_DIR`, `CARGO_NET_OFFLINE`, `CARGO_BUILD_JOBS` under `buildJobs`, or the
  doc lane's `RUSTDOCFLAGS`. Like the job count, they are part of each such step's key.
  **A value is written in `dagger.toml`, kept in those keys and shown in traces, so the
  setting is no place for a secret.**

## Layers

Each workspace has five layers. `dagger call rust workspace --path <root> <layer>`
returns one as a container.

| Layer | What runs | Keyed on |
| --- | --- | --- |
| `toolchain` | The image, `systemPackages`, then `rustup toolchain install`, the `target` and the components a lane asks for | The image, `systemPackages`, the toolchain files or the `toolchain` setting, `target`, the components |
| `fmt-toolchain` | The same under `fmtToolchain`, with rustfmt | The same, with `fmtToolchain` |
| `fetched` | `cargo fetch` over the recipe, in the toolchain layer, with the crates left in Cargo's home. Every lane that needs crates takes them from here | The toolchain layer, the recipe and `locked` |
| `check-dependencies` | On the fetch layer, `cargo check` over the recipe, once for each shape of command that clippy, doc and check run. The workspace's dependencies are left compiled in `/var/cache/rust/target-check`, and nothing of its members | The fetch layer, and the settings `features`, `allFeatures`, `noDefaultFeatures`, `buildProfile`, `target`, `locked`, `buildJobs` and `cargoEnvironment` |
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
- **Under `wholeWorkspace` the test lane's step is the run of the tests.** Its mirror is
  then of the whole repository, and the tests run in it:
  [Whole-workspace tests](#whole-workspace-tests).
- **A volume changes speed, never a verdict.** Over empty volumes a lane compiles every
  member on top of the dependency layer, and reports what it reports over warm ones.
  [docs/lanes.md](docs/lanes.md#known-limits) lists the cases where Cargo itself cannot
  tell that a file changed.

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

### On CI

The layers are cached like any other step, so any engine that has built one can serve it
again. A cache volume is part of the engine's state, so the volumes help where that state
survives from one run to the next.

On Dagger Cloud, [Cloud Checks](https://docs.dagger.io/cloud-checks/) run the checks that
`dagger check` runs, on Dagger's engines, for every push and pull request, with no CI file.
`dagger cloud checks on`, run from the repository's root, turns them on. Dagger chooses
the engine for each run, and with it the layers and volumes the run starts from.

On Buildkite, with
[agents hosted by Namespace](https://namespace.so/docs/solutions/buildkite-agents), the
`nsc-cache-tag` agent tag gives a step a Namespace cache volume
([agent configuration](https://namespace.so/docs/solutions/buildkite-agents/configuration)).
Keep the engine's state on it, and stop the engine gracefully before the job ends: an
engine that is killed wipes its cache when it next starts. Namespace may hold several
versions of a cache, typically one on each machine that has it, and passes the newest to
other machines in the background
([cache volumes](https://namespace.so/docs/architecture/storage/cache-volumes)). So a job
finds:

- the latest state: each lane compiles only what changed since;
- an older one: the mirror catches up, and Cargo compiles what differs since then;
- none, or volumes the engine has since collected: each lane starts from its dependency
  layer, which the same job may have to build first.

Namespace commits a job's cache only when the job exits cleanly, so after a failed job the
next one finds what the last clean job left.

### What the volumes save

These figures come from
[tokeira/dagger-rust-sdk](https://github.com/tokeira/dagger-rust-sdk) at v0.6.0 (five
members, 276 locked packages, Rust 1.99.1), on a `v1.0.0-beta.27` engine in Docker on an
Apple-silicon Mac, with the layers already built. check, clippy and doc are the wall time
of one `dagger check --check=<lane>`, and the test build is its step's own time, without
volumes and then with them:

| One-line edit | check | clippy | doc | test build |
| --- | --- | --- | --- | --- |
| In a package that nothing depends on | 11.3 s → 2.7 s | 11.3 s → 1.8 s | 9.0 s → 3.7 s | 3m25s → 5 s |
| In a package that two others depend on | 10.2 s → 3.6 s | 10.8 s → 3.9 s | 9.0 s → 4.6 s | 2m44s → 54 s |

Over empty volumes a lane takes as long as it would without them: the engine starts the
target volume from the dependency layer without copying it. With nothing changed, every
lane is answered from the engine's cache in one or two seconds, with or without volumes.

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
`locked`, `buildProfile`, `target`, `features`, `allFeatures`, `noDefaultFeatures`,
`buildJobs` and `cargoEnvironment`.
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

### Whole-workspace tests

A package's run sees its own directory. That suits tests that stay in their package, and
caches each package's run apart. It fails a test that reads a file of another package or
of the repository around the workspace, or that builds something in the tree.

Under [`wholeWorkspace = true`](#large-workspaces) the workspace's tests are one check,
and two steps do its work after the dependency layer:

| Step | What runs | Runs again |
| --- | --- | --- |
| Run | `cargo nextest run --no-fail-fast --failure-output immediate-final --no-tests pass <settings' flags> <packages>` builds the test binaries and runs them | After any edit to the repository |
| Doctests | `cargo test --doc --no-fail-fast <settings' flags> <packages>`, in the run's container | When the run does |

`<packages>` is `--workspace`, or `-p <name>` for each package the `test` setting
selects when it leaves some out. With `testRunner = "cargo"` the run is `cargo test
--no-fail-fast`, which runs the doctests too, and there is no second step.

- **The run has the whole repository, and can write to it.** Both steps run in the test
  lane's [cache volumes](#cache-volumes), and the mirror is of the repository: every file
  at its path, without `.git`, the files that `.gitignore` ignores and the workspace's
  `target` directory. A test reads any of it and writes where it likes. What a test
  writes is gone before the next step, whose sync makes the mirror equal to the
  repository again. `dagger call rust workspace --path <root> repository` returns what is
  mirrored.
- **There is no `.git`.** A workspace that Dagger loads from Git has none, and a local one
  would key the run on every commit and fetch. `git` itself is there, as in the image, so
  a test that makes a repository of its own works.
- **A Cargo that a test starts builds where its own defaults say.** The lane gives its
  commands their target directory as an argument, and keeps it out of their environment,
  where a test's own `cargo build` would find it. That Cargo runs off the network, with
  the fetch layer's crates, and under `buildJobs`.
- **The workspace's `.config/nextest.toml` is in force**, as it is in a package's run.
  The lane's own arguments outrank it: every test runs whatever fails, and what a failed
  test printed is shown again at the end of the report, which is the part the check's
  message keeps.
- **The run is keyed on the repository.** Any edit runs every selected package's tests
  again, where a package's own run would stay cached. The build is still incremental in
  the lane's volumes.

A failed test or doctest fails the check with `rust: test failed in <root>:` and the end
of the runner's report.

## Development

The module is the `.dang` files at the repository root. [docs/lanes.md](docs/lanes.md)
describes its layout, the contract a lane follows, and the checks.

The dev module, `.dagger/modules/dev`, is written in Dang, like the module, so nothing
is compiled before a check starts. It is the workspace's main module, so its checks and
its generator run from the repository root:

```console
$ dagger check                       # every check
$ dagger check -l                    # the checks, each with what it covers
$ dagger check --check=test-lane     # one check
$ dagger generate                    # format the Dang source
```

They need the `v1.0.0-beta.27` CLI and an engine of that release, and network access to
Docker Hub, dl.dagger.io, static.rust-lang.org, crates.io, deb.debian.org, github.com
and proxy.golang.org.

Each check installs the module in fixture workspaces from `fixtures/`, and drives it with
the Dagger CLI as a repository that uses the module would. It fails with the first
assertion that does not hold.

| Check | Covers |
| --- | --- |
| `single-package` | A package at the repository root: its keys, the check addresses, and every setting. |
| `workspace-members` | A workspace of several members: its keys, the `clippy` and `test` settings, and the toolchain and fetch layers. |
| `two-workspaces` | Two workspaces in one repository: selection, the default volume prefixes, and separate inputs. |
| `nested-workspaces` | A workspace in a directory of another, and that `workspaceRoots = ["."]` keeps to the outer one. |
| `lint-lanes` | The `workspaceRoots` setting, and the fmt, deny and advisories lanes. |
| `build-lanes` | The recipe, the check flavour of the dependency layer, the check, clippy and doc lanes, and the settings `checkAllFeatures`, `checkNoDefaultFeatures`, `buildJobs` and `cargoEnvironment`. |
| `test-lane` | The test runner, the test flavour of the dependency layer, and the test lane. |
| `cache-volumes` | The compile lanes' volumes, and the helper that runs a lane's step. |
| `whole-workspace` | The `wholeWorkspace` setting: the check names, the tests over the whole repository, and the one clippy check and fmt generator. |
| `shared-layers` | That the seven lanes of a workspace share one recipe and one fetch. |
| `dang-format/stale` | That the Dang source is formatted: the module's, and the dev module's. |

After a change to a `.dang` file, the module's or the dev module's, run `dagger
generate` before `dagger check`: it formats the file, which `dang-format/stale` would
otherwise fail. The helper, in `helper/`, is a Rust crate with no dependencies: `cargo
test` there runs its tests, and so does `cache-volumes`.

`dagger generate` also formats the Dang source with `dang fmt`. The generator
`dang-format` builds `dang` from the commit of Dang that the `v1.0.0-beta.27` engine
embeds, so that it formats the language the engine reads.

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
