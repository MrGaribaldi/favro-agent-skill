#!/usr/bin/env bash
# Link (or copy) the Favro skill into a Claude Code skills directory.
#
#   ./install.sh                 -> ~/.claude/skills/favro
#   ./install.sh --project .     -> ./.claude/skills/favro
#   ./install.sh --copy          -> copy instead of symlink
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source_dir="$repo_root/skills/favro"
target_base="$HOME/.claude"
mode="link"

while [ $# -gt 0 ]; do
  case "$1" in
    --project)
      [ $# -ge 2 ] || { echo "--project needs a directory" >&2; exit 2; }
      target_base="$(cd "$2" && pwd)/.claude"
      shift 2
      ;;
    --copy) mode="copy"; shift ;;
    -h|--help) sed -n '2,7p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

[ -f "$source_dir/SKILL.md" ] || { echo "Not a skill source: $source_dir" >&2; exit 1; }

target_dir="$target_base/skills"
target="$target_dir/favro"
mkdir -p "$target_dir"

if [ -e "$target" ] || [ -L "$target" ]; then
  echo "Refusing to overwrite existing $target"
  echo "Remove it first if you intend to replace the installed skill."
  exit 1
fi

if [ "$mode" = "link" ]; then
  ln -s "$source_dir" "$target"
  echo "Linked $target -> $source_dir"
  echo "A git pull in the repository now updates the installed skill in place."
else
  cp -R "$source_dir" "$target"
  rm -rf "$target/favro-cli/target"
  echo "Copied $source_dir -> $target"
  echo "Re-run this script after updating the repository to refresh the copy."
fi

echo
echo "Next:"
echo "  1. cargo install --path $repo_root/skills/favro/favro-cli --force"
echo "  2. which -a favro && favro --version   # expect 0.2.0 or newer"
echo "  3. Add \"Bash(favro *)\" to permissions.allow in your settings.json"
echo "     (see $(dirname "${BASH_SOURCE[0]}")/settings.snippet.json)"
