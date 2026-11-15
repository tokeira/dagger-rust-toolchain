# Rust toolchain module contributor guidance

## Mission and scope

`rust` is a Dagger module, written in Dang, that runs the Rust toolchain's checks over
every Cargo workspace in a repository: rustfmt, clippy and tests for each package, and
cargo check, cargo-deny and rustdoc for each workspace. Its steps are keyed on what they
read, so that an engine that has run a step can serve it again. These instructions apply
to the whole repository.

Read [README.md](README.md) for what the module does. Read [docs/lanes.md](docs/lanes.md)
before changing the core or a lane. It is the contract between them: what a lane receives,
how a step is keyed, how a package reads its share of a shared step, and the module's
known limits.

## Layout

- The `*.dang` files at the root are the module. They share one namespace, so every
  top-level name starts with `rust` or `Rust`. Each lane has a file of its own,
  `lane-<name>.dang`.
- `helper/` is the program a compile lane runs to sync its source mirror, run Cargo and
  keep what later steps read, all in one step. It is written in Rust, with the standard
  library alone.
- `.dagger/modules/dev` is the development module, written in Dang, and the workspace's
  main module. Its checks are the module's acceptance, and its generator `dang-format`
  formats the Dang source: the module's and its own. Its top-level names start with
  `dev` or `Dev`. docs/lanes.md says how a check is written.
- `fixtures/` holds the Cargo workspaces the checks run against. Several fail on purpose,
  and the checks assert those failures. Repairing one removes coverage; it does not fix a
  bug.

## Sources of truth

- Cargo decides what a workspace and its members are, and what a command does. The module
  asks Cargo (`cargo metadata`) rather than reading manifests itself, and behaves as Cargo
  would.
- [dagger.io/go](https://github.com/dagger/go) sets the shape of a Dang module:
  collections, a check for each package, and settings with selection patterns. Follow it
  unless Cargo works differently, and record the difference in docs/lanes.md.
- The engine's API at the module's `engineVersion` defines what Dang can call.

## Rules

- Logic lives in Dang or Rust. No shell scripts, and no `sh -c`.
- Pin every image by its multi-platform digest. Pin every downloaded tool by version and
  SHA-256 checksum, for linux/amd64 and linux/arm64.
- Give no type, function or argument a name that contains an acronym. From
  `v1.0.0-beta.26` the engine spells such names differently.
- Make the first line of a docstring a whole sentence. It is what `dagger check -l` and
  `dagger call --help` show.
- Never edit a generated file by hand.
- Format Rust with rustfmt. Format Dang with `dagger generate -y`, and commit what it
  writes: the generator `dang-format` runs `dang fmt` at the commit of Dang that the
  engine embeds, over the module's files and the dev module's.

## Keying

The engine caches a step by its inputs, so give each step the narrowest view that holds
what it reads. These facts decide what that view is. Each was measured on a
`v1.0.0-beta.27` engine:

- A step over a workspace view (`ws.directory(...)` with `include`), or over a `filter`
  of one, is keyed on the content of the files in the view. An edit elsewhere leaves it
  cached.
- A step over a `filter` or a `directory(path)` of another step's output runs again
  whenever that step runs again, even when the files come out the same. Take a narrow
  input from `RustInputs`, not from a step's output.
- Text is content. When a step's output is read as a string (`File.contents`, `stdout`)
  and given to a later step, the later step is keyed on the string. It stays cached when
  the first step runs again and writes the same text.
- A file read with `file(path)` from a directory that has been evaluated (`sync`) is keyed
  on its content, and so is a directory built from such files with `withFile`. Without
  the `sync`, the file is keyed on the step that made it.
- A cache volume's contents are no part of a step's key, and do not travel to another
  engine. Whatever fills a volume and whatever reads it must therefore run in one exec.
  A compile lane syncs its source mirror and runs Cargo in one run of the helper. The
  engine seeds a new target volume from the lane's dependency layer, and a new layer
  gets a new volume.

Three rules follow:

- Keep Cargo off the network outside the fetch layer. A step takes its crates with
  `withCrates`. When a crate is missing, add it to the fetch layer. Never fetch again in
  a lane.
- Build a shared step from the inputs, the settings and the selection alone, never from
  the package that is asking. Every package's check then builds the same step, and the
  engine runs it once.
- A step run with `expect: ANY` is kept whatever Cargo did. When Cargo reports that a
  signal stopped a process, run the command again as a step that must succeed, so that
  the failure of a killed compiler is not kept.

## Engine

The module and the dev module declare `engineVersion = "v1.0.0-beta.27"`, and are tested
on that release alone. The checks drive the module with the `v1.0.0-beta.27` CLI, pinned
by checksum, on the engine that runs them. No check asks which engine that is: an
assertion about what an engine does names that behaviour, and fails with its own message
on an engine that behaves otherwise.

## Verification

Run this from the repository root before pushing:

```console
dagger check
```

It runs every check: the acceptance, as one check for each group of fixtures, and the
format check. `dagger check -l` lists them, and `dagger check --check=<name>` runs one.

- Assert on results, not log lines: what a command returns, the message of a check that
  fails, the state of a step. Nested trace logs can lose lines.
- Keep the checks independent. Each must pass alone and beside the others, in any order:
  a check's cache volumes are its own, and a step it expects to run reads a value of its
  run. docs/lanes.md sets out the rules.
- When a change alters what an assertion checks, change the assertion. Say which one, and
  why, in the commit message.

## Provenance

- Use code or text from another project only when its licence allows it. When a file
  adapts code or text from an Apache-2.0 project, such as dagger.io/go, say which parts at
  the head of the file, and record the project in [NOTICE](NOTICE).
- [grouville/dagger-rust](https://github.com/grouville/dagger-rust) publishes no licence.
  Borrow nothing from it: no code, no text and no names.

## Worktree and Git safety

- Treat pre-existing changes as user work. Do not discard, overwrite, or include them
  without explicit authorization.
- Never use destructive restoration commands such as `git reset --hard`, `git clean
  -f`, or broad `git checkout`/`git restore` operations without approval of the exact
  command and target.
- Keep commits coherent. Do not mix dependency movement, generated churn, formatting,
  or unrelated cleanup into feature work.

## Commit attribution — recognise agent work (required)

Claude, working through Claude Code, does most of the work in this repository. That
contribution is recognised in the history, never flattened into a lone human author. The
human operator stays the Git `author`, and agents are credited with required commit
trailers:

- **`Co-authored-by: <Agent> <email>`**: one line for every agent that authored part of
  the change, including code, documentation, tests, or specifications.
- **`Assisted-by: <Agent> <email>`**: one line for every agent that assisted without
  primary authorship, including review, verification, research, or pairing.

Both trailers are required whenever their role applies. Credit every participating
agent. A commit with genuine agent involvement and no corresponding attribution trailer
is incomplete.

Canonical identity: `Claude <noreply@anthropic.com>`.

Trailers go at the end of the message, after a blank line. This repository does not
require a DCO sign-off, and an agent must never add a `Signed-off-by` trailer.

```text
Write the doc lane

<why the change is right>

Co-authored-by: Claude <noreply@anthropic.com>
```

Pull-request descriptions must name the participating agents, and distinguish
authorship from assistance. Do not include model names or task transcripts unless they
materially help review.

## Definition of done

A change is complete when:

- the module behaves as Cargo does;
- every step is keyed on what it reads;
- the checks pass and the generated files are current;
- docs/lanes.md and the README describe the change;
- agent attribution is recorded.
