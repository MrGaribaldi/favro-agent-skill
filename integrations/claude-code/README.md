# Claude Code integration

Claude Code discovers skills as directories under a `skills/` folder in its
configuration directory — `~/.claude/skills/<name>/SKILL.md` for a user-level
skill, or `<project>/.claude/skills/<name>/SKILL.md` for a project-level one.

## Install

```bash
./install.sh              # user level: ~/.claude/skills/favro
./install.sh --project .  # project level: ./.claude/skills/favro
```

The script symlinks `skills/favro` from this repository, so a `git pull` updates
the skill in place. Pass `--copy` if your setup cannot follow symlinks.

Then install the CLI and confirm the runtime resolves the right binary:

```bash
cargo install --path skills/favro/favro-cli --force
which -a favro
favro --version
```

## Permissions

Add the allow-rule from [`settings.snippet.json`](settings.snippet.json) to
`~/.claude/settings.json` (or the project's `.claude/settings.json`):

```json
"Bash(favro *)"
```

Two things about that rule are worth knowing, because both have cost real time:

**It is a prefix match.** Only commands starting with the literal `favro` match
it. `~/.local/bin/favro …`, an absolute path, or a `PATH=… favro …` prefix all
miss the rule and force an approval prompt. Always invoke the CLI bare.

**A background agent cannot answer a permission prompt.** It does not error or
time out usefully — it hangs until killed. So a command that fails to match the
allow-rule silently stalls automated work. This is also why the CLI expands
`\n` and `\t` itself: pass multi-line text as `--text "line1\nline2"` in plain
double quotes rather than shell `$'…'` ANSI-C quoting, whose embedded newlines
stop the command matching the rule.

If you grant narrower rules than `Bash(favro *)`, grant them per subcommand
(`Bash(favro list *)`) rather than per binary path — a path-based rule breaks
the moment the binary moves.

## Multi-agent setups

When several agents share one Favro project, give each a role in
`.favro/project.toml` and select it per agent with `FAVRO_ROLE` (or `--role`).
The role determines the comment prefix and, if configured, which credential
profile the CLI authenticates as. See
[configuration.md](../../skills/favro/references/configuration.md).
