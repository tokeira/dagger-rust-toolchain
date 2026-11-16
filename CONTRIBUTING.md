# Contributing

## Before you start

- [AGENTS.md](AGENTS.md) has the rules for a change. They apply to people and to agents.
- [docs/lanes.md](docs/lanes.md) is the contract between the core and the lanes. Read it
  before changing either.
- Bugs and ideas go to
  [the issues](https://github.com/tokeira/dagger-rust-toolchain/issues). A vulnerability
  does not: [SECURITY.md](SECURITY.md) says how to report one.

## Running the checks

The checks need the Dagger `v1.0.0-beta.27` CLI and an engine of that release. Run them
from the repository root:

```console
$ dagger check                       # every check
$ dagger check -l                    # the checks, each with what it covers
$ dagger check --check=test-lane     # one check on its own
$ dagger generate -y                 # format the Dang source
```

After a change to a `.dang` file, run `dagger generate -y` and commit what it writes.
The check `dang-format/stale` fails a file that is not formatted.

The README's [Development](README.md#development) section says what each check covers,
and what network access the checks need.

## Testing a new Dagger beta

The README lists a beta as [supported](README.md#supported-engines) once the module's
checks have passed on it. To run them on a new beta:

1. Install that beta's CLI, and start an engine of that release.
2. In `.dagger/modules/dev/harness.dang`, set `devDaggerVersion` to the beta, and the two
   checksums in `devDaggerArchive` to those of its CLI archives for `linux/amd64` and
   `linux/arm64`. Each check drives the module with the CLI pinned there, in new sessions
   of the engine that runs the check.
3. Leave `engineVersion` as it is, in `dagger-module.toml` and in
   `.dagger/modules/dev/dagger-module.toml`. It is raised only in a minor release.
4. Run `dagger check`.

When every check passes, open a pull request that adds the beta to the README's list,
and say in it what you ran. When one fails, open an issue with the check's message, or a
pull request with the fix.

`dang-format` builds `dang` at the commit that the `v1.0.0-beta.27` engine embeds,
whatever engine runs it. Its result says nothing about the new beta.

## Pull requests

- Keep one concern to a commit. Its message says what changes and why, in the style of
  `git log`.
- Credit each agent that took part with a commit trailer, as [AGENTS.md](AGENTS.md)
  sets out.
- A pull request merges once the module's checks have passed on it.
- The maintainer merges by fast-forward, so commits land as written. Rebase a branch on
  main when main has moved.

## Licence

Contributions are made under Apache-2.0, as section 5 of the [licence](LICENSE) says.
There is no contributor licence agreement, and no sign-off: do not add a `Signed-off-by`
trailer.
