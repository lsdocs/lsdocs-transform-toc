# AGENTS.md

Guidance for AI agents (and humans) working in lsdocs-transform-toc, one repository of
the lsdocs organization (https://github.com/lsdocs). It was split from the
lsdocs monorepo at commit b95f771; its history before that is the
monorepo's, filtered to these files. README.md says what it is and lists
its known limitations and TODO items.

## Working across repositories

Other lsdocs repositories are git dependencies
(`{ git = "https://github.com/lsdocs/<repo>", branch = "main" }`);
`Cargo.lock` pins their revisions (`nur bump [crate]` moves them).

- `nur link ../<repo>` builds against a local checkout (a `[patch]` in the
  untracked `.cargo/config.toml`); `nur unlink` undoes it.
- With `LSDOCS_ORG_DIR` set (e.g. `C:/dev`), Cargo fetches the
  organization's repositories from the local clones there (their
  committed `main`) instead of GitHub.
- `nur check-dupes` fails if a crate of the organization is built twice;
  `nur check-git-deps` fails on a dependency that is not a path inside
  this repository or a git dependency on the repository holding it
  (`scripts/org.nuon`), and on a committed `[patch]` table. `nur msrv`
  checks each crate on its own `rust-version`; `nur deny` runs
  cargo-deny (both are CI jobs of their own).

## Dev tasks

Dev tasks are nushell scripts run with `nur` (`nurfile`); `nur --list`
shows them. `nur ci` runs what CI runs: run it before committing. Tools:
`mise install` (see `mise.toml`); Rust from rustup.

## Conventions

- Commits: Conventional Commits, `type(scope): summary`.
- Line endings: LF everywhere; scripts that write files write LF.
- Comments and docs: ASCII only.
- MSRV is per crate (`rust-version`); the workspace's is the base.
- Dependencies are declared in `[workspace.dependencies]` and used with
  `workspace = true`.
- Lints: the workspace lints apply to every crate; `missing_docs` is on;
  `unsafe_code` is denied.
- No panics outside tests: clippy denies `unwrap_used`, `expect_used` and
  `panic` in library, binary and example code. Tests opt in per module
  (`#![allow(clippy::unwrap_used)]`).
- Licensing: `Apache-2.0 OR MIT`. Nothing is published.
- Record what is left open in README.md's TODO section, and what the code
  does not do (or does differently) under Known limitations.

## This repository

README.md says what it holds and lists its known limitations and TODO;
keep them current.
