# Codex integration

Codex has no skills directory. It reads instructions from `AGENTS.md` — at the
repository root for project scope, and from your Codex home directory for global
scope. So the wiring is a pointer rather than an install: keep this repository
checked out (or vendored as a submodule) and reference the skill from there.

## Install

```bash
cargo install --path skills/favro/favro-cli --force
which -a favro
favro --version    # must report 0.2.1 or newer
```

Then add the contents of [`AGENTS.snippet.md`](AGENTS.snippet.md) to your
`AGENTS.md`, adjusting the path to wherever this repository lives relative to
the project.

## Sandboxing and network access

The CLI talks to `https://favro.com/api/v1` over HTTPS. Whatever sandbox or
approval mode you run Codex under must permit outbound network access for
`favro`, or every command fails at the first request. Consult Codex's own
configuration documentation for the setting that applies to your version — this
repository deliberately does not pin a config key that may change.

Credentials come from `FAVRO_EMAIL` and `FAVRO_TOKEN` in the environment, or
from an env file the sandbox can read. Never place them in `.favro/project.toml`.

## Why the skill text is runtime-neutral

`SKILL.md` and its references describe operating rules and CLI invariants, not
Claude- or Codex-specific paths, so the same text works for both. The only
runtime-specific parts are installation and permissions, which is exactly what
lives in this directory.

One Claude-Code-flavored detail is worth carrying over anyway: pass multi-line
text as `--text "line1\nline2"` in plain double quotes. The CLI expands `\n` and
`\t` itself, which avoids shell quoting forms that some permission and approval
systems handle badly.
