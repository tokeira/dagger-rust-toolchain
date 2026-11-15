# Source layout and the lane contract

A lane is one kind of work the module does for a Cargo workspace: fmt, clippy, test,
check, deny or doc. The module's core gives each lane its name, its signature, its check
address and its inputs. This page is what someone who changes a lane, or adds one, needs:
the layout, what a lane receives, the rules every lane follows, the layers the lanes
share, what each lane does, and the module's known limits.

## Source layout

A directory of Dang files is one module. Types and top-level `let` functions are visible
from every file, with no import. A type cannot be reopened in a second file, so the
functions that Dagger calls live on the core's types and forward to one top-level function
per lane.

| File | Holds |
| --- | --- |
| `dagger-module.toml` | The module's name, `engineVersion` and runtime. |
| `rust.dang` | `Rust`: the settings (the constructor's arguments), `cargoFlags`, `workspaces` and `workspace`. |
| `discovery.dang` | Finding workspace roots and member packages: the skeleton, `cargo metadata`, the `workspaceRoots` setting, the mount path and the workspace name. |
| `workspaces.dang` | `RustWorkspaces` (collection), `RustWorkspace`, `RustInputs`, the layer accessors, and the forwarders of the workspace checks. |
| `packages.dang` | `RustPackages` (collection), `RustPackage`, `RustMember`, and the forwarders of the package checks and of the generator. |
| `layers.dang` | The image, the toolchain layer and the fetch layer. |
| `dependencies.dang` | The recipe, and the dependency layer's check flavour: the compiled dependencies, and the container of a check, clippy or doc lane. |
| `compile.dang` | The compile lanes' cache volumes, the helper's build, and the one step that syncs a lane's mirror and runs its Cargo command. |
| `helper/` | The helper, `rust-lane-helper`: a Rust crate with no dependency, which a compile lane's step runs. |
| `results.dang` | `RustLaneResults`: how a package's check reads its share of a shared step. |
| `support.dang` | Paths, patterns and sorting. |
| `lane-fmt.dang`, `lane-clippy.dang`, `lane-test.dang`, `lane-check.dang`, `lane-deny.dang`, `lane-doc.dang` | One lane each. |
| `lane-test-runner.dang`, `lane-test-dependencies.dang` | The test lane's runner, cargo-nextest, and the test flavour of the dependency layer. |
| `fixtures/` | Cargo layouts the acceptance runs over. |
| `.dagger/modules/dev/` | The dev module, in Dang, and the workspace's main module: the acceptance, as nine checks, and the generator `dang-format`. |

Every top-level name starts with `rust` or `Rust`, because all files share one namespace.
A lane's own helpers and types start with the lane's name too: `rustClippyStep`,
`RustTestNextestBinary`. Add a lane's helpers to its own file, and split a lane over
several files named `lane-<name>-<part>.dang` if it grows.

check, clippy, doc and the test build are the compile lanes: each compiles the
workspace's members with one Cargo command, in cache volumes of its own.

## Entry points

The core calls each lane through one function. Its name and signature are fixed.

| Lane | Function, in `lane-<lane>.dang` | Called by | Address |
| --- | --- | --- | --- |
| fmt | `rustFmtLane(package: RustPackage!): Changeset!` | `RustPackage.fmt`, a generator | `rust/workspaces/packages/fmt`, and the check `rust/workspaces/packages/fmt/stale` |
| clippy | `rustClippyLane(package: RustPackage!, selection: RustPackages!): Void` | `RustPackage.clippy(selection)` | `rust/workspaces/packages/clippy` |
| test | `rustTestLane(package: RustPackage!, selection: RustPackages!): Void` | `RustPackage.test(selection)` | `rust/workspaces/packages/test` |
| check | `rustCheckLane(workspace: RustWorkspace!, inputs: RustInputs!): Void` | `RustWorkspace.check(ws)` | `rust/workspaces/check` |
| deny | `rustDenyLane(workspace: RustWorkspace!, inputs: RustInputs!): Void` | `RustWorkspace.deny(ws)` | `rust/workspaces/deny` |
| doc | `rustDocLane(workspace: RustWorkspace!, inputs: RustInputs!): Void` | `RustWorkspace.doc(ws)` | `rust/workspaces/doc` |

The engine makes the `stale` check from the generator: it fails when `fmt` returns a
changeset that is not empty. `dagger check` runs it, and `dagger generate` applies the
changeset.

A package's check and the generator take no `Workspace`. The engine passes a package check
the selected `RustPackages`, and a generator nothing. What they read from the repository
was prepared when the collection was made, in `RustInputs`. A workspace check takes the
`Workspace` and prepares the same inputs itself.

Do not change a forwarder's name, arguments or docstring's first line without changing the
acceptance: they are the module's public surface.

## What a lane receives

### `RustInputs`

Views of the repository, each rooted at the repository root, so a file's path in a view is
its path in the repository. None runs a container.

| Member | Holds | A step over it is keyed on |
| --- | --- | --- |
| `source` | The workspace's files and the `includeExtraFiles` setting's. Workspaces rooted below this one, `.git`, `dagger.toml`, `dagger.lock`, the root's `target` and ignored files are left out. | Every file of the workspace. |
| `manifests` | `Cargo.toml` files, `Cargo.lock`, `.cargo/config.toml` and the toolchain files. | Those files. |
| `targets` | Paths of the files that make Cargo targets: `src/lib.rs`, `src/main.rs`, `src/bin/*.rs`, `tests/*.rs`, `examples/*.rs`, `benches/*.rs`, `build.rs`. | The set of paths, not the files' contents. |
| `toolchainFiles` | `rust-toolchain` and `rust-toolchain.toml` of the workspace root and of the directories above it. | Those files. |
| `formatFiles` | `rustfmt.toml` and `.rustfmt.toml` of the directories above the workspace root. The workspace's own are in `source`. | Those files. |

### `RustWorkspace`

| Member | Gives |
| --- | --- |
| `path` | The root, relative to the repository root. `"."` is the repository root. |
| `mountPath` | Where to mount a view: the `mountPath` setting, or `/src/<workspace name>`. |
| `workdir` | `mountPath` joined with `path`: where Cargo commands run. |
| `volumePrefix` | Prefix for a lane's cache volumes: the `volumePrefix` setting or the workspace name, then the root when it is not `"."`. The compile lanes' volumes are `<volumePrefix>-target-<lane>` and `<volumePrefix>-source-<lane>`. |
| `tool` | The `Rust` object: every setting, and `cargoFlags`. |
| `skeletonOf(inputs)` | The skeleton: the manifests as the repository has them, with an empty file for each target path. Cargo can load and resolve the workspace from it. |
| `recipeOf(inputs)` | The recipe: the skeleton as cargo-chef restores it, with the members' versions masked and a stub that compiles for each target. The fetch layer and both flavours of the dependency layer run over it. |
| `toolchainOf(inputs, components)` | The toolchain layer with the rustup components named, for example `["clippy"]`. Its working directory is `workdir`. |
| `fmtToolchainOf(inputs)` | The fmt toolchain layer: the `fmtToolchain` setting's toolchain, or the workspace's, with rustfmt. |
| `fetchedOf(inputs)` | The fetch layer: `toolchainOf(inputs, [])` after `cargo fetch`, with the recipe mounted at `mountPath`. A workspace has one, and every lane takes its crates from it. |
| `withCrates(inputs, ctr)` | `ctr` with the fetch layer's registry and git checkouts mounted in Cargo's home, and `CARGO_NET_OFFLINE=true`. |

### `RustPackage` and `RustPackages`

A `RustPackage` has `path` (its key), `name` (what `cargo -p` takes), `workspace` and
`inputs`.

A `RustPackages` is the selection a package check receives.

| Member | Gives |
| --- | --- |
| `paths` | The selected keys. The engine narrows this list. |
| `members` | Every member of the workspace as `RustMember(path, name)`, selected or not. |
| `narrowed` | Whether the selection left members out. |
| `selected(patterns)` | The selected members that `patterns` select too, in key order. Pass `workspace.tool.clippy` or `workspace.tool.test`. |
| `packageFlags(patterns)` | `["--workspace"]` when nothing is left out, otherwise `-p <name>` for each member of `selected(patterns)`. It raises when nothing is selected: Cargo, given no package, would run on every member. |
| `workspace`, `inputs` | As above. |

### `Rust`, the settings

`workspace.tool` reads every setting: `locked`, `features`, `clippyArgs`, `testRunner` and
the rest. `workspace.tool.cargoFlags` is the list every Cargo build command gets:
`--locked`, `--profile`, `--target` and the feature flags. Add the package selection
yourself.

### Helpers in `support.dang`

`rustPathIn(root, dir)`, `rustParent(file)`, `rustUnder(at, root)`, `rustScoped(root,
patterns)`, `rustLineage(root)`, `rustLines(items)`, `rustSorted(items)` and
`rustSelects(patterns, at)`.

## Rules

### Start from the layers

Build a lane's container from `toolchainOf` or `fmtToolchainOf`, give it crates with
`withCrates`, then mount a view at `workspace.mountPath`:

```dang
let audit = workspace.withCrates(inputs, workspace.toolchainOf(inputs, []))
  .withMountedDirectory(workspace.mountPath, inputs.source)
```

The toolchain layer keeps the toolchain files at their paths under `mountPath`, and its
working directory is `workdir`. rustup therefore selects the workspace's toolchain there,
with or without a view mounted. Under the `toolchain` or `fmtToolchain` setting the layer
sets `RUSTUP_TOOLCHAIN` instead, which outranks the files.

`withCrates` keeps Cargo off the network. A step that needs a crate the fetch layer did
not download fails and says so. Do not turn the network back on, and do not fetch again:
add what is missing to the fetch layer. A workspace has one, and a second one downloads
every crate twice on a cold engine.

With `locked` off, `cargo fetch` may write a `Cargo.lock`. It is at `workdir` in
`fetchedOf(inputs)`, whose mount is the recipe. A lane that resolves again must use that
file. The members' versions in it are the recipe's, masked: Cargo corrects them when it
resolves over the source.

A compile lane does not mount a view at `mountPath`: it mounts its source volume there,
and its target volume over the dependency layer's target directory. It starts from
`rustDependenciesLane`, or from `rustCompileLane` in `compile.dang`: see "The dependency
layer" and "The compile lanes' volumes" below.

### Key a step on what it reads

A step is cached by its inputs. Give it the narrowest view that holds what it reads. Six
facts, each measured on a `v1.0.0-beta.27` engine, decide what that is:

- A step over a workspace view (`ws.directory(...)` with `include`), or over a `filter` of
  one, is keyed on the content of the files the view holds. An edit elsewhere leaves it
  cached. This is how the toolchain layer survives every source edit.
- A step over a `filter` or a `directory(path)` of another step's output is not: it runs
  again whenever that step runs again, even if the files come out the same. Take a narrow
  input from `RustInputs`, not from a step's output.
- Text is content. When Dang reads a step's output as a string (`File.contents`,
  `stdout`) and gives that string to a later step (`withNewFile`, an argument), the
  later step is keyed on the string. It stays cached when the first step runs again and
  writes the same text. This is how the recipe, and every layer over it, survives a
  release bump.
- A file read with `file(path)` from a directory that has been evaluated (`sync`) is keyed
  on its content, and so is a directory put together from such files with `withFile`.
  Without the `sync` the file is keyed on the step. This is how a package's test run
  stays cached while its binaries come out the same.
- A step that mounts a cache volume is cached like any other: with the same inputs it
  does not run again. The volume's contents are no part of its key, and do not travel to
  another engine. Only the compile lanes mount volumes, and only in the one step that
  "One step for all that touches a volume" describes.
- A cache volume is named by its key, its sharing mode and the directory it starts from
  (`withMountedCache(path, cacheVolume(key), source: directory, sharing: LOCKED)`), within
  the module that mounts it. A new volume holds that directory's files, with their
  modification times. With another directory under the same key the engine mounts
  another volume, started from that directory, and with the first directory again it
  mounts the first volume, as its last step left it. This is how a lane's target volume
  follows its dependency layer.

### One shared step, read per package

clippy, the test build and the doctests cover many packages in one Cargo command. Every
package's check must build exactly the same step, so that the engine runs it once.

- Make the step a function of the inputs, the settings and
  `selection.selected(patterns)` only. Never give it the package that is asking.
- Name the packages with `selection.packageFlags(patterns)`.
- Give each covered package one text, in a directory, in a file named by the package's
  position in `selection.selected(patterns)`: `0`, `1`, and so on. A package that passed
  has an empty file. One that failed has the text to show the user.
- Wrap that directory in `RustLaneResults`, and end the lane with `assertPassed`:

```dang
let rustClippyLane(package: RustPackage!, selection: RustPackages!): Void {
  let covered = selection.selected(package.workspace.tool.clippy)
  RustLaneResults(lane: "clippy", covered: covered, results: rustClippyStep(selection))
    .assertPassed(package.path)
}
```

`assertPassed` raises `rust: clippy failed in <path>:` and the file's text, or
`rust: package <path> is outside the clippy selection` when the step did not cover the
package.

The forwarders have already applied the `clippy` and `test` settings to the package that
is asking: a package the setting leaves out passes without reaching its lane. The shared
step must still leave such packages out of its command, which `selected` and
`packageFlags` do.

A shared step must fail closed. When its command fails and its output does not say which
package is at fault, every covered package fails. A package passes a failed step only
when the output accounts for the failure in other packages: clippy's "not checked" and
the doctest step's hints, under "The lanes" below, are the two cases.

### One step for all that touches a volume

The engine keys a step on its inputs and never on what a volume holds. Two steps over
one volume are therefore unsafe whenever the second relies on what the first left there:
the engine can answer the first from its cache, with the volume holding another run's
files, and then run the second and keep its result under the right key. A sync step
followed by a Cargo step would build the wrong tree that way.

So everything that reads or writes a lane's volumes happens in one step, which the
helper runs: it makes the mirror equal to the lane's source view, runs the Cargo command,
and copies what later steps read out of the volume into the step's own filesystem. A
lane runs its command with `rustCompileCargo(workspace, lane, command, output, options)`:

```dang
let ran = rustCompileCargo(
  workspace,
  rustDependenciesLane(workspace, inputs, "doc", [], rustDependenciesLayer(workspace, inputs, [])),
  ["cargo", "doc", "--quiet", "--workspace", "--no-deps"] + workspace.tool.cargoFlags,
  "/tmp/rust-doc-output",
  [],
)
```

- A lane that runs a second Cargo command in its volumes runs it the same way, as a
  step that syncs for itself: the check lane's second feature set, and the doctests. Each
  is then right whatever ran before it.
- A step that reads the build's results mounts no volume. It reads the step's own
  filesystem: the exit code, `stderr`, the file `output`, and what the helper kept
  under `rustCompileKept`.
- A step outside the lane must not read the mirror either. The test lane's `cargo
  metadata` runs over `inputs.source` in a container without volumes.

### Report failures by raising

A check returns `null` when it passes. It fails by `raise` with a message that starts
`rust: ` and says what the user should look at. A generator returns its `Changeset`; an
empty one means nothing to change.

To report Cargo's own message, a lane runs its command with `expect: ANY` and reads the
exit code and `stderr`: a failed step's error is only `exit code: N`. A step run with
`expect: ANY` succeeds whatever Cargo did, so the engine keeps its result: the same
inputs give the same failure without running again. That is right when the source is at
fault, and wrong for a failure that is not, such as a compiler killed for lack of memory
or a download that failed.

- `rustDependenciesRan(lane, arguments, output, command)` runs a step with `expect: ANY`,
  and runs it once more without `expect` when Cargo says a signal stopped a process, or
  when the helper failed for a reason of its own, such as a full disk, which it reports
  with exit code 125: a failed step without `expect` is not kept, so the next run tries
  again. `rustCompileCargo` runs a compile lane's command through it, under the helper,
  and `rustDependenciesCargo(lane, command, output)` the command of the dependency
  layer's test flavour, which mounts no volume.
- A step whose failure may not be the source's runs without `expect`, and its error
  is only `exit code: N`. The fetch layer does, because it reaches the network, and so
  do the check flavour's `cargo check` steps, whose error names the workspace and points
  to the failed step's log.

### Keep to the module's conventions

- No shell scripts. Pass each command to `withExec` as a list of arguments, and use its
  `redirectStdout`, `redirectStderr` and `expect: ANY` where a shell would redirect or
  test an exit code. The module runs no shell: it uses `xargs` where it needs one command
  per line of a file, and parses Cargo's JSON and TOML in Dang, with `JSON.decode` and
  `TOML.decode`. What one step must do in sequence, the helper does: it is a program
  built from `helper/` in a content-keyed step, and has no dependency, so building it
  needs no crate and no network. Extend it, with a test, when a lane needs more of that.
- Pin every image by the digest of its multi-platform index, and every downloaded tool by
  checksum for `linux/amd64` and `linux/arm64`. The module must run on both.
- Use no acronyms in the names of types, functions and arguments. From `v1.0.0-beta.26`
  the engine spells names that contain acronyms differently, and a name without one is
  the same under both rules.
- The first line of a docstring is what `dagger check -l` and `dagger call --help` show.
  Make it a whole sentence.
- Format the source with `dagger generate`. The dev module's generator `dang-format`
  runs `dang fmt -w` over the `.dang` files at the repository root and over the dev
  module's, with `dang` built from the commit of Dang that the `v1.0.0-beta.27` engine
  embeds, and its `stale` check, `dang-format/stale`, fails a file that is not
  formatted. `dang fmt` formats the files of one directory, not those below it, so the
  generator names both directories.
- Stay within the Dang that `go.dang` of dagger.io/go uses. The module declares
  `engineVersion = "v1.0.0-beta.27"`, the one release it is tested on, and the engine
  checks its calls against that release's API.
- The module has no `dang.toml`, as dagger.io/go has none, so the engine reads its
  source in the current language version of the Dang it embeds. The `v1.0.0-beta.27`
  engine embeds commit `00dc3801b409` of
  [github.com/vito/dang](https://github.com/vito/dang), a pre-release of v2.3.1, whose
  language version is 2.1. `dang-format` builds `dang` from the same commit.

## Discovery and the `workspaceRoots` setting

`rustDiscover` in `discovery.dang` keeps the candidates that `workspaceRoots` selects,
with `rustSelects`, before it asks Cargo about any of them. `cargo metadata` therefore
never runs in a root that the setting leaves out.

The setting chooses roots, not packages and not files. A workspace still has every member
Cargo gives it, and its `RustInputs` still hold every file below its root that is not in
another workspace: the files of a root that the setting leaves out belong to the workspace
that encloses it, if there is one.

## The layers

Each workspace has these, and no step of them reads a source file or mounts a cache
volume:

| Layer | Function | What runs | Keyed on |
| --- | --- | --- | --- |
| Toolchain | `toolchainOf`, `fmtToolchainOf` | The image, `systemPackages`, `rustup toolchain install`, the `target`, the components | The image, `systemPackages`, the toolchain files or settings, `target`, the components |
| Recipe | `recipeOf` | `cargo chef prepare` over the skeleton, then `cargo-chef chef cook --no-build` from the document it wrote | What the document says |
| Fetch | `fetchedOf` | `cargo fetch` over the recipe | The toolchain layer, the recipe, `locked` |
| Dependencies, check flavour | `rustDependenciesLayer` | `cargo check` over the recipe in each shape a lane runs, then `cargo clean -p` | The fetch layer, and the settings that change compilation |
| Dependencies, test flavour | `rustTestDependencies` | `cargo build --tests` over the recipe, then `cargo clean -p` | The toolchain layer, the fetch layer, the recipe, and the same settings |

### The recipe

`dependencies.dang` writes the recipe with
[cargo-chef](https://github.com/LukeMathWalker/cargo-chef) 0.1.78, pinned by checksum.

| Function | Gives |
| --- | --- |
| `rustDependenciesRecipe(workspace, inputs)` | The recipe, as a directory rooted at the repository root that Cargo can load, resolve and compile: the members' manifests with their versions masked and their lint tables dropped, `Cargo.lock` with the same versions masked, the Cargo configuration, and a stub that compiles in place of each target's source. Other manifests and configuration of `inputs.manifests` are there as they are. `workspace.recipeOf(inputs)` is the same. |
| `rustDependenciesLocal(lock)` | The packages a lockfile takes from a path, each as `name@version`: what both flavours of the dependency layer clean. It reads the lockfile with `TOML.decode`. |

What to know before changing it:

- **The recipe is the workspace's one stub tree.** The fetch layer and both flavours of
  the dependency layer run over it. The skeleton is not a compile input: discovery, the
  fmt and deny lanes and a package's test run read it, because it has every manifest as
  the repository has it.
- **The recipe is keyed on what it says.** cargo-chef writes the recipe as one JSON
  document, in a step over the skeleton that runs again whenever a manifest changes.
  Dang reads the document as text, and a second step restores the directory from that
  text. So the recipe, the fetch layer and both flavours of the dependency layer stay
  cached when the document comes out the same: after a source edit, after a release that
  changes only members' versions, and after an edit to a manifest's comments. They run
  again when a dependency, a feature, a target or the lockfile changes.
- **cargo-chef masks the members' versions.** In the recipe every member has version
  `0.0.1`, in its manifest and in `Cargo.lock` (seen with cargo-chef 0.1.78). A package
  that is not a member keeps its own version. `rustDependenciesLocal` therefore reads the
  lockfile the layer compiled with, never the source's: `cargo clean -p <name>@<version>`
  finds no package under the version the source gives a member.
- **cargo-chef knows the members only.** A package that is not a member, such as one that
  `includeExtraFiles` brings in, is in the recipe with its own manifest and an empty file
  for each conventional target path, as in the skeleton. A build script gets an empty
  `main`. A target at a path the manifest names is not there.
- **A `base` container needs `tar` and `xz`**, to unpack cargo-chef.

### The dependency layer

The dependency layer is the workspace's dependencies, compiled, with nothing of its
members. It has two flavours, because `cargo check` and `cargo test` compile a dependency
differently, each in a target directory of its own in the layer's filesystem:

| Flavour | Function | Target directory | Serves |
| --- | --- | --- | --- |
| Check | `rustDependenciesLayer(workspace, inputs, sets)` | `rustDependenciesTarget`, `/var/cache/rust/target-check` | check, clippy and doc |
| Test | `rustTestDependencies(workspace, inputs)` | `rustTestTargetDir`, `/var/cache/rust/target-test` | test |

`RustWorkspace.checkDependencies` and `RustWorkspace.testDependencies` return them to a
caller, as `recipe` and `fetched` return theirs.

**The check flavour** runs, on the fetch layer, `cargo check --workspace --all-targets`
and `cargo check --workspace` under the settings' flags, then `cargo check --workspace`
under each feature set of `sets`.

- **One `cargo check` for each shape of command.** Cargo compiles a dependency once for
  each set of features it is used with, and the set depends on the command: with
  `--all-targets` the dev-dependencies count, and each feature flag changes it. A lane
  that runs Cargo in a shape the layer did not run compiles the difference itself, on
  every run. Add a shape to the layer when you add one to a lane. `sets` is how the
  check lane adds its own without the clippy and doc lanes waiting for them, or failing
  with them: the layers share their first steps.
- A check, clippy or doc lane starts from `rustDependenciesLane(workspace, inputs, lane,
  components, dependencies, marks)`: `toolchainOf(inputs, components)`, the fetch layer's
  crates in Cargo's home with Cargo kept off the network, and the lane's two volumes,
  the target volume started from the layer's target directory. `lane` names the
  volumes: `"check"`, `"clippy"` or `"doc"`. `marks` is `""`, or what the lane's command
  covers that Cargo cannot tell: see "A target volume is for one set of marks" below.
  `rustDependenciesFlags(tool)` is `--locked`, `--profile` and `--target` from the
  settings, without the feature flags.

**The test flavour** runs `cargo build --tests --workspace --profile test` over the
recipe, with the settings' flags and the fetch layer's crates.

- `cargo build --tests --profile test` compiles the same dependencies as `cargo test`,
  with the same profile, and `buildProfile` replaces `test`.
- It builds with `CARGO_INCREMENTAL=0`, like every Cargo command of the test lane, so
  that unchanged inputs give the same binaries, byte for byte.
- The test build runs in the toolchain layer with the same environment, and its target
  volume starts from the layer's target directory.

Both flavours:

- **Tolerate no failure.** cargo-chef's stubs compile, so a build that fails over the
  recipe is the dependencies' failure or the settings': a dependency that does not build,
  a feature no package has, a `Cargo.lock` that `--locked` refuses. The layer raises, and
  the lane does not run its own command. The acceptance runs both flavours over a stub of
  every kind of target (`build-kinds`) and over a package outside the workspace root
  (`build-outside`).
- **Keep nothing of the stubs.** Cargo decides by modification time whether a package it
  takes from a path is up to date, and a mounted source file keeps the time it had on
  the user's disk. A stub's artifact is newer than a source file that was last edited
  before the layer was built, and Cargo would take it for the member's. `cargo clean -p
  <name>@<version>` removes it, for each package of `rustDependenciesLocal`. The clean
  is not optional: without it, a test build over sources older than the layer took the
  stubs' outputs for the packages' and compiled nothing of them (measured). The fixture
  `build-stale` is a package Cargo cannot tell from its stub; it does not compile, and
  its checks must say so.
- **Are what a lane's target volume starts from.** The lane compiles the members into
  its own volume, on top of the layer's compiled dependencies. For Cargo to find those
  fresh, the lane must run as the layer did: the same mount path, Cargo home and target
  directory, and the same profile, features, target and environment. Change both
  together.

## The compile lanes' volumes

`compile.dang` gives each compile lane two cache volumes, both `LOCKED`: one step holds
a volume at a time, and since each lane has its own, no lane waits for another.

| Volume | Mounted at | Holds |
| --- | --- | --- |
| `<volumePrefix>-target-check`, `-target-clippy`, `-target-doc` | `rustDependenciesTarget`, `/var/cache/rust/target-check` | The lane's target directory: the check flavour's compiled dependencies, and what the lane compiled of the members, with Cargo's incremental state. |
| `<volumePrefix>-target-test` | `rustTestTargetDir`, `/var/cache/rust/target-test` | The same for the test build, on the test flavour's dependencies. |
| `<volumePrefix>-source-<lane>`, for the same four lanes | `mountPath` | The lane's mirror of its source view, where Cargo runs. |

| Function | Gives |
| --- | --- |
| `rustCompileLane(workspace, inputs, lane, toolchain, target, dependencies)` | A lane's container: `toolchain` with the helper, the source view, and the lane's two volumes. `dependencies` is the directory the target volume starts from. |
| `rustCompileCargo(workspace, lane, command, output, options)` | The step: the helper syncs the mirror and runs `command` with its stdout in the file `output`. `options` is the helper's other arguments: what to keep for later steps, `--renew all`, and `--marks`. |
| `rustCompileView(workspace, inputs)` | The lane's source view: `inputs.source`, and with `locked` off the `Cargo.lock` that the fetch resolved in place of the source's, as text. |
| `rustCompileVolume(workspace, holds, lane)` | A volume's name. |
| `rustCompileHelper(platform)` | The helper's binary. |
| `rustCompileStep(workspace, selection, lane)` | A lane's step over every package its setting selects: what `RustWorkspace.compiled` returns. |

**The target volume starts from the dependency layer, and follows it.** The engine
starts a new volume from the directory it is given, here the target directory of the
lane's dependency layer. A volume is named by that directory as well as by its key (the
sixth fact under "Key a step on what it reads"). So:

- an empty volume, on a new engine or after the engine collected it, starts from the
  dependency layer again;
- when the dependency layer changes, as after a change to `Cargo.lock`, the toolchain or
  a feature setting, the lane mounts a new volume that starts from the new layer, and
  compiles every member once;
- when it changes back, as on another branch, the lane mounts the earlier volume again,
  with what it had compiled.

The engine does the seeding. No marker in the volume names the dependency layer it was
started from, and the helper copies no dependency. As the engine's source has it at `v1.0.0-beta.22`,
`v1.0.0-beta.25` and `v1.0.0-beta.27` (`core/cache.go`), a volume is a writable
snapshot on top of its source directory's, so starting one copies nothing; an engine
configured with `cacheVolumes.dir` keeps volumes as plain directories, and copies the
source into a new one. Only the first was run here.

What a volume holds changes no verdict in any case: Cargo compiles whatever its target
directory lacks. The layers have removed what they compiled of the stubs, so a volume
never starts with a stub's artifact.

**A target volume is for one set of marks.** Some of what decides a command's result
is not in Cargo's view, and Cargo then takes what one command left in a target
directory for what another would make. Two cases were measured with Cargo 1.100, both
in the clippy lane:

- clippy lints the packages its command names, and only compiles a member that is
  there because a named package needs it. After `cargo clippy -p <a package that needs
  it>`, a member with a lint error passed `cargo clippy --workspace` in the same target
  directory, unlinted.
- clippy reads `clippy.toml`. After an edit to it, every member that had passed stayed
  as it was.

So a lane can have marks: a text that says what its command covers. `rustClippyMarks`
is the paths of the packages the command names, `clippyArgs`, and the `clippy.toml` and
`.clippy.toml` files of the source, with their contents. clippy 1.100 records its
arguments with what it compiles, so that Cargo compiles the members again when they
change, but a workspace's toolchain may be older. `rustDependenciesLane` writes the marks into
the directory the target volume starts from, as the file `rustCompileMarks`. A volume is
named by that directory, so a command with other marks gets another volume under the
same key, and finds nothing that another command left. The helper checks it
(`--marks`): if the target volume holds other marks than the lane's, or none, as it
would on an engine that named volumes otherwise, the helper empties it, and Cargo
compiles everything again. The check, doc and test lanes have no marks: each compiles
what it compiles whatever the selection.

**The mirror keeps modification times.** Cargo decides by modification time whether a
member is up to date. The helper makes the mirror equal to the lane's source view,
which is mounted at `rustCompileSource`, by comparing content:

- a file whose bytes are the same is left alone, with the time it has;
- a file that differs, or is new, is written, and has the time of the write;
- a file or directory that the view no longer has is removed, as is anything a build
  left in the mirror.

After an edit Cargo therefore sees new times on the edited files alone, and compiles
the packages that hold them and the packages that need those. A file put back as it was
is written again too, since it differs from the mirror: Cargo compiles its package
again.

**A manifest can get a new time with the same content.** Cargo tracks what rustc read
file by file, and notices an edit or a deletion of any of those files, wherever they
are. It tracks rustdoc, and a build script that names no file with
`cargo::rerun-if-changed`, by the newest time among the files of the package. That
misses two things, both measured with Cargo 1.100 and `cargo doc`, where a warm lane
passed and a lane over empty volumes failed:

- A file that is gone leaves the newest time as it was. So when the sync removes
  something, it gives a new time to the nearest `Cargo.toml` above it: the manifest of
  the package that lost the file. Every lane does this.
- A file outside the package that its documentation includes, with `include_str!` in a
  `doc` attribute, is none of the package's files. So the doc lane passes `--renew all`:
  whenever the sync wrote or removed anything, every `Cargo.toml` in the mirror gets a
  new time, and rustdoc runs again for every member. What the doc lane keeps from run
  to run is what rustc compiled, the members' metadata.

A package whose build script names no file is compiled again in the same cases, as it
is on any change to its own files. That is Cargo's documented rule for such a script,
and was not measured here.

**The helper** is `helper/`, a Rust crate that uses the standard library alone.
`rustCompileHelper` builds it with the Cargo of the module's pinned image, whatever the
`image` and `base` settings say, from the crate's files and nothing else, so every
workspace on an engine shares one build. It is built for musl, so it is linked
statically and runs in any `base`. The toolchain layer of that image gets the musl
target from rustup first, which needs the network once. In a lane's step it:

1. syncs the mirror (`--source`, `--mirror`, and `--renew all` for the doc lane);
2. checks the target volume's marks, for a lane that has any (`--marks`);
3. runs the command in the step's working directory, with its stdout in a file of the
   step (`--stdout`), and exits with the command's exit code;
4. when the command succeeded, copies the files that the build's own listing names
   from the target volume to `rustCompileKept`, at the paths they have in the target
   directory (`--keep`, with `--listing nextest` or `--listing cargo`);
5. adds to the report at `rustCompileReport`: the command (`run`), what the sync did
   (`wrote`, `removed`, `renewed`), the packages Cargo compiled and those it documented
   (`compiled`, `documented`), and the exit code.

```console
$ dagger call rust workspace --path . compiled --lane doc file --path /var/cache/rust/report contents
run cargo doc --quiet --workspace --no-deps --locked
mirror wrote 1 removed 0 kept 12
wrote crates/mid/src/lib.rs
renewed Cargo.toml
renewed crates/base/Cargo.toml
...
compiled suite-mid
documented suite-base
documented suite-lone
documented suite-mid
documented suite-tool
exit 0
```

The report's lists of packages come from the target directory: Cargo renews a unit's
`invoked.timestamp` when it runs rustc, rustdoc or a build script for it. The helper
reads the two layouts it was tried on: that of Cargo 1.91 and 1.99, and that of Cargo
1.100. The report is for whoever inspects a lane, and the acceptance reads it. No
verdict depends on it.

The helper's tests are in the crate: `cargo test` in `helper/`. The acceptance runs
them.

**There is no registry volume.** The crates are in the fetch layer, a step keyed on its
inputs that any engine cache can serve and that travels to another engine. A registry
volume would only help where it is warm, and would let a lane's result depend on what
it holds.

## The lanes

### fmt

`rustFmtLane` runs one step per package, `cargo fmt -p <name>`, in `fmtToolchainOf`. The
step's mount is the skeleton with the package's files laid over it:

- the skeleton, because `cargo fmt` asks `cargo metadata --no-deps` for the package's
  targets, which loads every member's manifest and wants its target files to exist;
- the package's `.rs` files, from a `filter` of `inputs.source`, without the directories
  below it that hold a `Cargo.toml`;
- the rustfmt configuration files of the package and of each directory above it, from
  `inputs.source` and `inputs.formatFiles`.

It needs no crate and no lockfile, so it does not call `withCrates`. The changeset is the
package's `.rs` files after the step against the same files before it, both at their
repository paths.

Measured on `v1.0.0-beta.27`:

- An edit to a package's Rust file runs that package's step and leaves the others `CACHED`.
- An edit to a manifest runs every package's step, because the skeleton runs again.
- A file that is not Rust source and not a rustfmt configuration runs nothing.
- A second `dagger generate` over unchanged files names no `cargo fmt` step at all. Its
  trace has `RustPackage.fmt CACHED`: the engine answers the generator's function from
  its cache, without evaluating it.

### clippy

`rustClippyRun` is the shared step: one `cargo clippy --quiet <packages> --all-targets
--keep-going --message-format=json <settings' flags> -- --no-deps <clippyArgs>`, in the
lane's volumes, on the check flavour of the dependency layer, with Cargo's JSON messages
kept in a file of the step. `rustClippyStep` sorts them into the per-package texts in
Dang, only when the command failed. A run that passes costs each check one exit code.

A warm volume changes no verdict. Cargo names a member it finds fresh among its
artifacts, as it names one it compiled, and a member with an error is never fresh: Cargo
compiles it again and reports the error again. The packages the command names,
`clippyArgs` and `clippy.toml` are the lane's marks: a command that differs in any of
them has a target volume of its own.

When the command fails, `rustClippyReports` gives each covered package:

- its own errors, when clippy or the compiler reports any in it;
- `not checked: blocked by <member>`, when it needs a member that has an error in its
  library or build script and whose library Cargo did not get compiled. "Needs" comes
  from `cargo metadata` over the recipe, by path, through every kind of dependency. An
  error in a member's tests alone blocks nobody;
- `not checked: cargo clippy failed, and no compiler error in a member explains it`,
  when the failure is none of those: an error in a dependency, a build script that
  failed, or Cargo stopping before it compiled anything;
- nothing, and the package passes, when every error is in a member it does not need.

### check and doc

`rustCheckLane` runs `cargo check --quiet --workspace` under `--no-default-features` and
under `--all-features`, the two ends of the feature space that clippy does not reach. It
leaves an end out when the settings already put clippy there, and passes those sets to
the dependency layer as `sets`. Each set is a step of its own in the lane's volumes
(`rustCheckRun`), and each syncs the mirror for itself. Cargo keeps what it compiled
under one set apart from the other, so both stay warm in the one target volume.

`rustDocLane` runs `cargo doc --quiet --workspace --no-deps` with `RUSTDOCFLAGS="-D
warnings"`, in the lane's volumes (`rustDocRun`). rustdoc runs again for every member
whenever the tree changed, and rustc compiles again only the metadata that the edit
reaches: "A manifest can get a new time with the same content" has the reason.

Both raise on any exit code but 0, with Cargo's `stderr`.

### deny

`rustDenyLane` returns `null` unless `inputs.source` has a `deny.toml` in the workspace
root. Otherwise it runs one step, `cargo deny [flags] check --disable-fetch`, in
`withCrates(inputs, toolchainOf(inputs, []))`:

- **cargo-deny** comes from `http` with a checksum and `File.extract`, with no container
  command, and is copied to `bin` in Cargo's home, where Cargo finds subcommands.
- **The advisory database** is `git(...).head(noLock: true).tree`, mounted where
  cargo-deny keeps it: `advisory-dbs/advisory-db-3157b0e258782691` in Cargo's home. The
  directory's name is cargo-deny's, a hash of the address. `noLock` makes the engine
  resolve the branch on every run, and write nothing to the repository's `dagger.lock`.
  The tree keeps its `.git` directory, which cargo-deny opens.
- **The mount** is the skeleton with `inputs.source` laid over it, without its `.rs` files.
  With `locked` off, the `Cargo.lock` is the fetch layer's.

Measured on `v1.0.0-beta.27`:

- Every run resolves the database's branch with `git ls-remote`. The tree of a commit the
  engine has fetched is `CACHED`.
- A second run, and a run after an edit to a Rust file, leave `cargo deny` `CACHED`.
- An edit to `deny.toml`, or a new file that is not Rust source, runs it again and leaves
  `cargo fetch` `CACHED`.

### test

| Step | Function | Shared | Keyed on |
| --- | --- | --- | --- |
| The runner | `rustTestWithNextest` | By every workspace | The release archive of cargo-nextest, which the engine fetches by checksum and unpacks. Nothing executes. |
| The dependency layer, test flavour | `rustTestDependencies(workspace, inputs)` | By the workspace | See "The dependency layer". |
| The test build | `rustTestCompiled(selection)` | By every package's check | The dependency layer, `inputs.source`, the settings, and `selection.packageFlags(tool.test)`. |
| The workspace's metadata, for nextest | `rustTestMetadata(selection)` | By every package's check | `inputs.source` and the toolchain layer. It mounts no volume. |
| A package's run | `rustTestNextestRun`, or `rustTestCargoRun` under `testRunner = "cargo"` | No: one per package | The content of the package's test binaries and of the binaries its tests run, the skeleton, the package's own files, and the toolchain and fetch layers. |
| The doctests | `rustTestDoctests` | By every package's check | The test build: it runs in the build's container. |

The test build and the doctests run in the lane's volumes, `<volumePrefix>-target-test`
and `<volumePrefix>-source-test`, under the helper. After a source edit the build
compiles the packages the edit reaches and no other. No other step of the lane mounts a
volume.

**The build keeps what the runs read.** The target directory is a volume, which a later
step must not read. The helper therefore copies the files that the build's listing
names from the volume to `rustCompileKept`, in the step's own filesystem, at the paths
they have in the target directory: the test binaries, the binaries the tests run, the
members' build scripts' output directories and the directories of libraries the
binaries load. The listing is there too: `binaries-metadata.json` under nextest, and
`test-binaries.jsonl` under cargo.

**A run is keyed on content.** `rustTestOutputs` evaluates what the build kept, and
`rustTestKept` copies the files a run needs out of it one at a time, with `file` and
`withFile`: the fourth fact under "Key a step on what it reads". A run is therefore
cached for as long as its binaries are the same, byte for byte, however often the build
runs again. The build compiles with `CARGO_INCREMENTAL=0`, at the dependency layer's
paths and under its environment, so a package whose inputs did not change has the same
binaries whether Cargo found them in a warm volume or compiled them in a new one.

**What a run sees:**

- At `mountPath`: the skeleton, and over it the package's own directory without the
  directories of members below it, the workspace's `.config/nextest.toml`, and the files
  of `includeExtraFiles`. Other packages' source files are empty there. A test that reads
  a file of another package finds it only if `includeExtraFiles` names it. It is the
  skeleton and not the recipe because this is what a test reads when it runs: the
  skeleton has every manifest as the repository has it, and the recipe masks the
  members' versions.
- At `rustTestTargetDir`, as a directory and not the volume, at the paths the build left
  them: the package's test binaries, the binaries its tests run
  (`CARGO_BIN_EXE_<name>`), its build script's output directory, and the libraries the
  build's binaries load. Under nextest the list comes from `cargo nextest list
  --list-type binaries-only`, and under cargo from the JSON messages of `cargo test
  --no-run`.
- The toolchain layer and the crates, as every lane has them.

nextest runs the package's binaries with `--binaries-metadata` and `--cargo-metadata`,
its documented way to run binaries it did not build in that container, and with
`-E 'package(=<name>)'`. The list still names the other packages' binaries, which are not
there. nextest 0.9.151 skips a binary that the filter leaves out without opening it.

**Results.** The lane's text for a package comes from three steps, so no step writes the
result files. `rustTestLane` puts the package's text in a directory of its own, at the
package's position, and ends with `RustLaneResults.assertPassed` like clippy.

- When the test build fails, every covered package fails: the package Cargo names in
  `could not compile` with Cargo's output, the others with `not tested: the test build
  failed.` before it. cargo-nextest and `cargo test` have no `--keep-going`, so the
  packages that a failure does not block are not built either.
- After each package's doctests, the shared doctest step prints a hint if they failed:
  ``error: doctest failed, to rerun pass `<arguments>` ``. `rustTestDoctestOwner` reads
  whose doctests a hint is about:
  - `-p <name> --doc` names the package. Cargo 1.100.0 writes this form when the command
    ran with `-p`, or with `--workspace` over more than one member.
  - `--doc` names none. Cargo writes it when the command ran with `--workspace` over a
    workspace with one member, and the hint is then about that member.
  - `-p <name>@<version> --doc` names the package too. Cargo 1.100.0 was not seen to
    write it, even for a command that ran with `-p <name>@<version>`.
- The doctest step fails closed. A package passes a failed step only when every hint is
  about another covered package. When a hint is about no covered package, or the step
  failed with no hint, every covered package fails with the step's output. The exception
  is Cargo's `no library targets found`: no package it was given has a library, there is
  no doctest, and every package passes.
- A package's share of the step's output is what Cargo and rustdoc printed between
  `Doc-tests <crate>` and the package's hint. Cargo writes those two lines to stderr,
  and rustdoc writes the doctests' report to stdout, so the engine can record the
  report after the hint: it did once, on an engine busy with every check of the
  acceptance. A share without the report's `test result:` line is therefore not taken
  for the whole: the package gets the step's whole output instead.

## Known limits

- **clippy under a `target` setting can pass a package unchecked.** A member that Cargo
  compiles both for the host, as a build-dependency, and for the target has one package
  id in Cargo's `compiler-artifact` messages. `rustClippyBuilt` in `lane-clippy.dang`
  reads those messages to tell whether a member's library was compiled, and the host
  artifact can hide a target library that failed. A package that needs that library is
  then not "blocked", and can pass although clippy never checked it.
- **The fetch layer says little when it fails.** With `locked` on and no `Cargo.lock`,
  `cargo fetch --locked` fails and the check's error is a bare `exit code: 101`: Cargo's
  message is only in the failed step's log. The step is not run with `expect: ANY`, which
  would let a lane report the message, because the engine keeps the result of such a
  step, and would then keep a failure that the network caused.
- **The shared doctest step runs again after every source edit.** rustdoc compiles
  doctests when they run, against the libraries in the test lane's target volume, so
  the step is keyed on the test build and not on any package's files. After an edit to
  one package, every selected package's doctests run again, while the test runs of the
  packages that do not link it stay cached.
- **A lane is incremental only where its volumes are warm.** The volumes stay on the
  engine that wrote them. On an engine without them, or after the engine collected
  them, a lane compiles every member once, on top of the dependency layer. A change to
  the dependency layer starts a new target volume, with the same cost, and the volumes
  of earlier layers stay until the engine collects them.
- **Over a warm volume, Cargo decides what to compile again.** It does so by the
  modification times in the mirror and by what each compilation said it read. The
  module covers what Cargo was seen to miss: for rustdoc, for a deleted file, and for
  the packages and the `clippy.toml` of a clippy command. It does not cover
  an input that Cargo is never told of: a file that a build script reads without
  naming it, when the script names others with `cargo::rerun-if-changed`, and a file
  that a procedural macro reads. A warm lane then keeps what it built from the old
  file, where a lane over empty volumes builds from the new one. A new `volumePrefix` gives
  a lane empty volumes. These two cases are Cargo's documented rules, and were not
  measured here.
- **The lanes trust the engine's clock.** A file that the sync writes has the time of
  the write. If the clock steps back between two runs, by more than the time between
  them, Cargo can take an edited file for one that is older than its last build.
- **A build that writes into its source tree loses those files.** The sync removes from
  the mirror whatever the source view does not hold, before the next build.
- **With `locked` off, Cargo writes `Cargo.lock` in the mirror on every run.** The sync
  puts back the lockfile that the fetch resolved, whose member versions are the
  recipe's, and Cargo corrects them again. A root package whose build script names no
  `rerun-if-changed` is then compiled again on every run. Not measured.
- **Workspaces with one name share volumes.** The default `volumePrefix` is the
  repository's name, and volumes belong to an engine. Two repositories of that name on
  one engine, or two checkouts of one repository, use the same volumes in turn. That
  changes no verdict, since each step syncs for itself and holds the volumes while it
  runs, but each run compiles what differs from the one before. Set `volumePrefix` to keep
  them apart.
- **The report of what Cargo compiled reads Cargo's own layout.** It knows the layout of
  target directories of Cargo 1.91 and 1.99, and that of Cargo 1.100, which differ.
  Under another, the report names no package, and the acceptance's assertions that read
  it fail. No verdict depends on it.
- **A compile error in the test build fails every selected package's test check.**
  cargo-nextest and `cargo test` have no `--keep-going`.
- **Packages that are not members get conventional stubs only.** A package that
  `includeExtraFiles` brings in has an empty file for each conventional target path in
  the skeleton and the recipe. A target at a path its manifest names is not there.
- **The deny acceptance reads the live RustSec database.** The engine fetches
  `https://github.com/RustSec/advisory-db` at its default branch on every run, so the
  acceptance can change between runs with no change to the module: the assertion that
  the `flagged` fixture fails names RUSTSEC-2020-0095 and fails if RustSec withdraws that
  advisory, and an assertion that two runs share a cached `cargo deny` fails if RustSec
  publishes between them.

## Acceptance

The acceptance is the dev module's checks. The dev module is the workspace's main
module (`main = true` in `dagger.toml`), so `dagger check` at the repository root runs
them all, side by side, with the format check. Each drives the module over fixtures,
each copied into a repository of its own, and fails with the message of the first
assertion that does not hold.

| Check | Function, in `.dagger/modules/dev` | Asserts | Fixtures |
| --- | --- | --- | --- |
| `single-package` | `devCoreSingle`, in `core.dang` | Discovery of a single package, the check addresses, and every setting. | `single` |
| `workspace-members` | `devCoreMembers`, in `core.dang` | The collections of a workspace of several members, the `clippy` and `test` settings, and the toolchain and fetch layers. | `members` |
| `two-workspaces` | `devCorePair`, in `core.dang` | Two workspaces in one repository: selection, default volume prefixes, and separate inputs. | `pair` |
| `nested-workspaces` | `devCoreNested`, in `core.dang` | A workspace in a directory of another. | `nested` |
| `lint-lanes` | `devLint`, in `lanes-lint.dang` | The `workspaceRoots` setting, and the fmt and deny lanes. | `single`, and those whose names start `lint-` |
| `build-lanes` | `devBuild`, in `lanes-build.dang` | The recipe, the check flavour of the dependency layer, and the check, clippy and doc lanes. | `members`, `single`, `pair`, `nested`, and those whose names start `build-` |
| `test-lane` | `devTest`, in `lanes-test.dang` | The runner's pins, the test flavour of the dependency layer, and the test lane. | `single`, `test-suite`, `test-doctest`, `build-kinds`, `build-outside` |
| `cache-volumes` | `devCache`, in `lanes-cache.dang` | The helper's own tests, and the compile lanes' volumes: their names, what a lane compiles over empty volumes, after a one-file edit and after a change to the dependency layer, that a deleted file is gone, and that verdicts over warm volumes and over empty ones agree. | `test-suite`, `pair`, `build-lints`, `build-stale` |
| `shared-layers` | `devCoreShared`, in `core.dang` | That the six lanes of a workspace share one recipe and one fetch. | `lint-deny` |

`dev.dang` holds the checks themselves, on `Dev`, and the generator `dang-format`.
`harness.dang` holds what every file uses.

```console
$ dagger check                       # every check
$ dagger check --check=build-lanes   # one check
$ dagger check -l                    # the checks, each with what it covers
```

A check prints each assertion as it holds, in its function's log. The engine does not
always deliver those lines, and nothing reads them: a check's verdict is its result.

### How a check is written

The dev module is written in Dang, like the module. The engine interprets it, so
nothing is compiled before a check starts, and the module loads on any engine at or
above its `engineVersion`, `v1.0.0-beta.27`. Its files share one namespace: every
top-level name starts with `dev` or `Dev`, and one that belongs to a single file with
that file's name too.

- **A check is a function of `Dev`** in `dev.dang`, with `@check` and
  `@cache(policy: FunctionCachePolicy.NEVER)`. Its name is the check's address:
  `singlePackage` is `single-package`. The first line of its docstring is what `dagger
  check -l` shows.
- **It takes the `Workspace`.** `devBegin(ws, "<check>")` reads the repository through
  one view of it, `ws.directory("/", include: [...], gitignore: true)`: the module's
  `.dang` files and manifest, `helper/` and `fixtures/`. A view is keyed on the files it
  holds. Of a local workspace those are the checkout's, without what `.gitignore`
  leaves out, and of a workspace loaded from Git the commit's. Only a local workspace
  was run. `.gitignore` leaves out every directory called `target`, so no fixture can
  hold one. `devBegin` returns the check's `DevRun`: the view, and a container with
  git, the pinned CLI and the module's source at `/rust`.
- **A group of assertions is a function that returns the messages of those that held.**
  It makes a fixture's repository with `devConsumer`, runs `dagger` commands in it, and
  ends each assertion with `devHeld("<fixture>: <what holds>")`. `devPassed` reports
  how many held.
- **An assertion fails by `raise`.** A helper that checks something, such as
  `devPrints`, `devSameLines` or `devStateOf`, raises `devCheckFailed(what, detail)`:
  what was checked, then what was found in place of what was expected. The check stops
  there, and that text is its error.
- **An assertion that a trace lacks a step holds on a trace with no step at all.** So
  each sits beside an assertion that needs a step of the same trace, through
  `devTraced` or `devStateOf`, which fail on a trace they cannot read.
- **A message is the assertion's own.** Dang puts words before the message of an error
  that passes through certain places: `each block: ` for the block of `.each`,
  `evaluating left side: ` for an operand, `evaluating receiver: ` for the value a
  method is called on. So a loop whose body asserts is `devEach(items) { ... }`, which
  adds nothing, and the result of a call that can raise is bound with `let` before an
  operator or a method uses it.
- **A command that must succeed** runs through `devStdout` or `devStderr`. When it
  fails, the engine's error says only `exit code: N`, so `rescue` replaces it with the
  command, its exit code and the end of its output, which the error carries.
- **A command that must fail**, as a fixture's deliberate failure makes it, runs
  through `devFailing` or `devRefused`: with `expect: FAILURE`, so the step succeeds
  when the command fails, and the assertion reads the whole of what it printed with
  `combinedOutput`. When the command succeeds, the engine raises, and `rescue` turns
  that into `<command>: succeeded, but must fail`. An error rescued from a failed step
  would carry only the last 100 KiB of each output, and an assertion that an output
  does not say something needs all of it.
- **The value of a run is `run.next`:** the check's name and `Random.string`, 26
  characters that are new on every read. `devDagger` gives one to every `dagger`
  command, as an environment variable, and an assertion puts one in an edit, a file or
  a setting wherever a step must run.

### The checks are independent

Each check must pass alone, on an engine that has run nothing, and beside the others,
in any order. `dagger check` runs them at the same time. Three rules keep it so:

- **A check's volumes are its own.** `devConsumer` and `devConfigured` give the module
  a `volumePrefix` in every repository they make: `<check>-<fixture>`, unless the
  settings name one. Two checks that run the same lane over the same fixture therefore
  mount different volumes, and neither finds the other's build or waits for its lock.
  `devInstalled` writes the settings as they are, for an assertion that reads the
  default prefix. A command from it must mount no volume, unless the repository has a
  name no other check uses, which `devRenamed` gives it: `cache-volumes` asserts the
  default names of the volumes in a repository called `cache-volumes-pair`.
- **A step that must run reads a value of the run.** `run.next` gives a value that no
  other command of the run, no other run and no other check has: it starts with the
  check's name, and the rest is random. An assertion that a step is `DONE` first puts
  such a value among the step's inputs, as an edit, a file or a setting.
- **A step that must be cached was run by the same check.** An assertion that a step
  is `CACHED` follows a command of the same check that ran it.

No check is cached: each is declared `@cache(policy: FunctionCachePolicy.NEVER)`. Its
verdict depends on the engine in hand and, for `lint-lanes` and `shared-layers`, on
RustSec's database of the moment, and neither is among the inputs the engine would key
a cached verdict on.

No check asks which engine runs it. Each drives the module with the `v1.0.0-beta.27`
CLI, pinned by checksum for `linux/amd64` and `linux/arm64`, on the engine that runs
the check, and an assertion about what an engine does names that behaviour: that a
step is `CACHED` or `DONE`, that a volume is `LOCKED` and starts from a directory, that
a failed step is not kept. An engine that behaves otherwise fails that assertion, with
its message, and no other. The checks were run on `v1.0.0-beta.27` alone.

On a ten-CPU machine, a run of every check took between 2 and 3 minutes on an empty
engine, and between 1¼ and 3 minutes on a warm one. Together the checks keep every CPU
busy. On a smaller machine, `dagger check -j <n>` runs at most `n` of them at once: an
overloaded engine can close a session it has not heard from for a few seconds, and the
check that started it fails with `engine closed the session`. A run leaves about 2 GB on
the engine, in cached steps and in the volumes of `cache-volumes`.

When you change a lane or add one:

1. Assert what the lane does over a fixture, in the lane's file. Add the fixtures it
   needs under `fixtures/`, each small and named for the lane's file: for example a
   package that rustfmt would change, or one with a clippy warning.
2. Assert the lane's cache behaviour with `devTraced`, which returns each container
   command of a `dagger` command with `DONE` when it ran and `CACHED` when it did not:
   which steps a source edit runs again, and which a manifest edit does. A compile
   lane's command runs under the helper: `devTraced` gives the Cargo command, and says
   that the helper ran it.
3. Run `dagger generate` after any change to the dev module's sources, which formats
   them, then run the check. The engine reads the source as it is: there is nothing to
   compile, and no generated file.

What to know about reading a `dagger` command from the acceptance, on a `v1.0.0-beta.27`
engine:

- A package check that passed is not run again over the same files and the whole
  selection: the engine answers `dagger check --check=clippy` without calling the module,
  and the trace names no container command. A check that failed, a narrowed selection
  (`--rust-package`) and a workspace check are evaluated every time. An assertion that
  reads a passing package check's commands must change a file first:
  `lanes-test.dang` writes a file with a value of the run into each package.
- `devTraced` fails on a trace with no container command. A second `dagger generate`
  has such a trace, so `lanes-lint.dang` reads it with its own `devLintTrace`.
- What a compile lane's step did is in the helper's report, a file of the step. Read it
  with `dagger call rust workspace --path <root> compiled --lane <lane> file --path
  /var/cache/rust/report contents`: the files the sync wrote and removed, and the
  packages Cargo compiled and documented. `compiled` runs the lane over every package its setting
  selects, and names them with `-p`, so its step is not the one that `dagger check` runs
  with `--workspace`, but it runs in the same volumes.
- `cache-volumes` gives the module a new `volumePrefix` on every run. Its volumes then
  start empty whatever the engine holds, and stay on the engine until it collects them.
- A step's output reaches a trace as log lines, and one acceptance run in five lost the
  lines of the last step of a nested `dagger check`, whose step itself was there and
  `DONE`. `lanes-test.dang` therefore asserts on three things only: whether a `dagger`
  command succeeds, the message of a check that fails, and the state of each step in a
  trace. So that a passing check still shows what ran, a fixture is broken on purpose: a
  failing test's message carries the runner's report, which lists the tests that passed
  beside it.

A fixture that is a Cargo workspace is discovered by the module when it is installed at
the repository root. The repository does not install the module on itself for that
reason.
