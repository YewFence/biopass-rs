# AGENTS.md

## Project positioning

This repository is `biopass-rs`, a personal and unofficial Rust rewrite of upstream [`biopass`](https://github.com/TickLabVN/biopass).

Use `biopass` to refer to the upstream TickLabVN project, its packages, its configuration paths, and its PAM module.

Use `biopass-rs` to refer to this repository, its Rust crates, its helper binary, its package artifacts, its configuration paths, and its PAM module.

### Config schema

When config schema changes , update `crates/biopass-rs-auth/src/config/migration.rs` to support migration from old config to new config. The other code should always read **the newest** config schema, and the migration code should be the only place that reads the old config schema.

## Code check and lint

when modifying code in this repository, please run the following commands to check and lint your code:

```bash
mise run check
```

will run `cargo check` `cargo clippy` and `cargo fmt --check` for rust crates, and run `vp check` for the tauri frontend. confirm that all checks pass before committing your changes.

if there is lint error, you can run:

```bash
mise run fix
```

to automatically fix lint errors.
