<!--
Add this section to your AGENTS.md. Adjust the path if this repository is
vendored somewhere other than ./vendor/favro-agent-skill.
-->

## Favro coordination

Track work, questions, decisions, risks and deliverables in Favro using the
`favro` CLI. Before any Favro command, run `favro --version` and require 0.2.1
or newer; a missing `--version` flag means a stale pre-0.2 binary — stop and
reinstall rather than continuing.

Read `vendor/favro-agent-skill/skills/favro/SKILL.md` for the operating rules,
and the references it links when you need them:

- `references/configuration.md` — project setup, roles, credentials, migration
- `references/workflow.md` — lane semantics, interviews, handoffs, completion
- `references/cli.md` — commands, Markdown round-trips, troubleshooting
- `references/development-profile.md` — code review and commit conventions
  (only when the project enables that profile)

Project behavior comes from `.favro/project.toml` in the current repository.
Run `favro check` before mutating anything. Credentials live in `FAVRO_EMAIL`
and `FAVRO_TOKEN`; never write them into a card, a comment, or committed
configuration.
