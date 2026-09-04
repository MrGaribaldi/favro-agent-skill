# Contributing

## Building and checking

```bash
cargo test   --manifest-path skills/favro/favro-cli/Cargo.toml
cargo clippy --all-targets --manifest-path skills/favro/favro-cli/Cargo.toml -- -D warnings
cargo build  --release --manifest-path skills/favro/favro-cli/Cargo.toml
```

Clippy is expected to pass with warnings denied. Rust 1.82 or newer is required
(`rust-version` in `Cargo.toml`); `Option::is_none_or` is the current floor.

## Testing against Favro

Prefer read-only verification. `favro check` and `favro list-collections` make
no mutations, and `favro --config <path> check` will exercise configuration
parsing without touching a live collection at all.

If you must test writes, use a scratch collection you own — never a collection
with real work in it. The CLI has no delete command; `archive` is the closest
thing and is reversible with `--undo`.

## Design rules worth preserving

These are invariants the code has been shaped around. Changing one is fine, but
do it deliberately.

- **Inspection never mutates.** `list`, `overview`, `review-queue`, `check` and
  card lookup must not create collections, boards or lanes. Only `ensure-project`
  and `ensure-board` create structure, and only after the operator has confirmed
  names and visibility.
- **Secrets stay out of `project.toml`.** The file names environment variables.
  It is meant to be committed.
- **Human-authored comments are untouchable.** `comment-edit` and
  `comment-delete` act only on comments carrying the selected role's prefix or
  the legacy `🤖` marker.
- **Report the mutation before the optional extras.** If a card is created,
  print its ID before applying tags or shared fields, so a later failure cannot
  produce an unreported card.
- **Truncate on character boundaries.** Error paths quote user and API text;
  `truncate_utf8` exists because byte slicing panicked on Norwegian input.
- **Round-trip Favro's Markdown.** Favro stores `- [ ]` as `☐` and blank lines as
  a leading `↵`. Every whole-description write must go through
  `checkboxes_to_markdown` first, or checkboxes and human ticks are destroyed.

## Documentation

`SKILL.md` is the agent-facing contract and should stay short; detail belongs in
`references/`. Keep both runtime-neutral — anything specific to one agent
runtime belongs in `integrations/`.

## Licensing

MIT. `skills/favro/favro-cli/LICENSE` is a copy of the repository-root `LICENSE`
so that `cargo package` includes the license text; release review should confirm
the two remain byte-identical.
