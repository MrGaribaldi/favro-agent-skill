# favro CLI

Companion CLI for the agent-neutral Favro coordination skill. This independent project is not affiliated with or endorsed by Favro AB.

## Build and install

Requires Rust 1.82 or newer.

```bash
cargo install --path . --force
```

Cargo normally installs to `$CARGO_HOME/bin` (or `$HOME/.cargo/bin`). Ensure that directory is on `PATH`. If an existing user-local binary directory appears earlier on `PATH`, install there explicitly or remove the stale copy. For example:

```bash
cargo install --path . --root "$HOME/.local" --force
which -a favro
favro --version
```

`which -a favro` should show the intended installation first; `favro --version` must report 0.2.0 or newer.

When working from the repository root:

```bash
cargo install --path skills/favro/favro-cli --force
```

Authentication uses `FAVRO_EMAIL` and `FAVRO_TOKEN`; `FAVRO_ORG_ID` is required when the account belongs to multiple organizations. A read-only API token supports inspection but cannot create or move cards. Keep secrets in the environment or an ignored env file, never in `.favro/project.toml`.

Run `favro init`, configure the generated project file, then use `favro check`. Upgrade by rerunning the same `cargo install` command with `--force` and verify resolution again with `which -a favro`.

The crate-local `LICENSE` is copied from the repository-root `LICENSE` so Cargo packages include the text. Release review must confirm the two files remain byte-identical.
