# Favro agent coordination skill

An agent-neutral `SKILL.md` workflow and companion Rust CLI for coordinating product, research, business, compliance, operations, and development work in Favro. This independent project is not affiliated with or endorsed by Favro AB.

The skill treats Favro as the coordination surface for work, responsibility, progress and handoffs, while substantive artifacts stay in their own authoritative systems. Project-specific behavior — roles, lanes, credentials, completion gates — lives in a committed `.favro/project.toml`, so the same skill serves different teams without forking it.

## Install

Rust 1.82 or newer is required.

```bash
cargo install --path skills/favro/favro-cli
favro --version    # must report 0.2.1 or newer
```

Install and verify the CLI separately in **every** agent/container environment. Home directories and `PATH` binaries are often not shared even when the project workspace is, and a stale pre-0.2 binary fails in confusing ways — which is why the skill's first step is a version check. See [CLI installation details](skills/favro/favro-cli/README.md) for `PATH` shadowing, `--root`, authentication and upgrades.

Then wire the skill into your agent runtime — see [`integrations/`](integrations/):

| Runtime | Guide |
| --- | --- |
| Claude Code | [`integrations/claude-code`](integrations/claude-code/README.md) |
| Codex | [`integrations/codex`](integrations/codex/README.md) |
| Anything else | [`integrations/generic`](integrations/generic/README.md) |

## Use

From a product repository:

```bash
favro init            # asks for collection name, visibility, interview granularity
$EDITOR .favro/project.toml
favro check           # validates config, credentials, boards, users, shared fields
```

`project.toml` is safe to commit: it names environment variables, never tokens. Structure is created only by the explicit `ensure-project` and `ensure-board` commands — inspection commands never create collections, boards or lanes.

Read [the skill](skills/favro/SKILL.md) for operating rules, then the references it links for [configuration](skills/favro/references/configuration.md), [workflow](skills/favro/references/workflow.md), [CLI details](skills/favro/references/cli.md) and the optional [development profile](skills/favro/references/development-profile.md).

## Layout

```
skills/favro/          the portable skill — point your runtime here
├── SKILL.md           operating rules
├── references/        configuration, workflow, CLI, development profile
└── favro-cli/         the Rust CLI (crate: favro-cli, command: favro)
integrations/          per-runtime wiring: install steps, permission snippets
```

`skills/favro/` is deliberately shaped so the directory can be symlinked or copied
straight into a runtime's skill directory under the name `favro`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
