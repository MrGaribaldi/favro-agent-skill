# Any other agent runtime

The skill is a plain directory of Markdown plus a single static binary. Nothing
about it is specific to one vendor, so wiring it into another runtime needs
three things.

## 1. Install the CLI and make the version checkable

```bash
cargo install --path skills/favro/favro-cli --force
which -a favro
favro --version    # must report 0.2.0 or newer
```

Do this in **every** environment the agent runs in. Containers and CI images
frequently do not share a home directory with your workstation, and a stale
binary is the single most confusing failure mode this project has: pre-0.2
builds reject `--version` outright, which is precisely why the skill's first
step is a version gate.

## 2. Put `SKILL.md` in front of the model

Whatever the runtime's mechanism is — a skills directory, a system prompt, an
instructions file, a retrieval index — it needs to reach
`skills/favro/SKILL.md`. That file is short by design and links the four
reference documents for the detail, so a runtime that can load files on demand
should load `SKILL.md` eagerly and the references lazily. A runtime that cannot
follow links should inline `SKILL.md` and let the agent read the references with
its own file tools.

## 3. Permit the binary and its network access

The agent must be able to execute `favro` and reach `https://favro.com/api/v1`
over HTTPS. If the runtime has an allow-list, prefer a rule scoped to the
command name (`favro …`) over one scoped to an absolute path — a path-based rule
breaks the moment the binary is reinstalled elsewhere.

Credentials come from `FAVRO_EMAIL` and `FAVRO_TOKEN` in the environment, or an
env file named by `FAVRO_ENV_FILE`. A read-only API token can inspect Favro but
cannot create or move cards.

## Contributing an integration

If you wire this into a runtime not covered here, a directory alongside
`claude-code/` and `codex/` with a README — and an install script if the runtime
has a skills directory worth linking into — is a welcome contribution.
