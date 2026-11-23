# Internal and public feature roadmaps

Roadmaps use one card per feature or theme, linked to implementation cards on their
existing boards. They do not replace task workflows. Keep internal context, work
links and public copy separate. Neither an existing internal roadmap nor a request
to build a roadmap authorizes publishing its contents; the human chooses what is public.

## Configuration and initialization

Requires CLI 0.3.0+. These defaults work without adding a section:

```toml
[roadmap]
board = "Veikart"
lanes = ["Nå", "Neste", "Senere", "Lansert"]
lane_keys = ["now", "next", "later", "released"]
public_tag = "roadmap:public"
max_public_chars = 400
```

Names and keys pair by index. Keep keys stable when renaming labels. Configure both
lists when changing the number of lanes. Public text accepts up to `max_public_chars`
Unicode characters (configurable 1–4000); public titles accept up to 120 characters.
Use Bokmål and one to three sentences for the public description. Language and sentence
count are editorial checks, not automatic language detection.

After agreeing the board's name and existing collection visibility:

```bash
favro check
favro roadmap init
```

`init` reuses the `ensure-board` implementation, creates missing lanes and the public
tag, and orders the configured lanes. Repeated runs reuse existing structure. Other
columns and their cards are retained. It does not broaden the collection's visibility.
It uses the same credentials as other commands. Inspection/export never creates
structure or tags and works with a read-only token once the roadmap is initialized.

## Maintain the internal roadmap

```bash
favro roadmap add --title "Dokumenthistorikk" --work '#3758' --body "Internal context and dependencies"
favro roadmap add --title "Bedre navigasjon" --lane "Neste"
favro roadmap link --card '#4000' --work '#3758' '#3759'
favro roadmap link --card '#4000' --work '#3759' --unlink
favro move --card '#4000' --lane "Lansert"
favro roadmap list
favro roadmap list --lane "Neste" --json
```

`add` defaults to the first configured lane. Work references are positive sequential
card numbers, with or without `#`; repeat `--work` or pass several arguments. Link
changes deduplicate and sort references in the maintained `ROADMAP WORK` block.
They preserve other description text, human checkbox ticks, the public block and
attachments. Do not concurrently edit the same description. Malformed or duplicate
managed blocks stop a write rather than guessing which text is safe to replace.

`list` groups cards by configured lane and shows public status plus each linked work
card's board, lane and archive status. Multiple board placements remain visible;
missing work cards show an empty placements list. This exposes drift for inspection,
but never moves roadmap cards automatically. `--public-only` filters this **internal**
listing; it does not make its output safe for publishing.

## Approve public copy and export

```bash
favro roadmap set-public --card '#4000' --title "Dokumenthistorikk" --text "Se tidligere versjoner av dokumentene dine."
favro roadmap export --public --format json --out roadmap-public.json
favro roadmap unpublish --card '#4000'
```

`set-public` writes the public title, text and UTC date into `ROADMAP PUBLIC` and adds
the configured public tag in the **same** Markdown PUT. `unpublish` clears that block
and removes the tag in the same write. Both preserve attachments and verify the exact
card afterwards. Verification failures report that the write may already have happened;
inspect before retrying. An orphan public block alone is private; the tag alone is an
export error. Generic `tag`/description commands can still create inconsistent state,
which export deliberately refuses.

Only `roadmap export --public --format json` produces a public artifact. It allowlists:

```json
{
  "generated": "2026-11-23",
  "lanes": [
    {
      "key": "now",
      "name": "Nå",
      "items": [
        {
          "id": "r04dc51112b4a7708",
          "title": "Dokumenthistorikk",
          "text": "Se tidligere versjoner av dokumentene dine.",
          "updated": "2026-11-23"
        }
      ]
    }
  ]
}
```

All configured lanes are present, even when empty. Lanes follow configuration order;
items follow card `listPosition` (falling back to `position`), with common ID as a stable
tie-breaker. The short item ID is deterministic FNV-1a over the card's common ID, stable
across lane moves and text edits; collisions/duplicate public IDs fail. It is an opaque
identifier, not a security token. `generated` is the UTC export day; `updated` is the
UTC public-copy date maintained by `set-public`, not an internal activity timestamp.
Repeated exports of the same data on the same UTC day are byte-identical.

Internal descriptions/titles, comments, assignments, tags, work references, other dates
and attachments are never serialized. Archived cards and lane-header cards are omitted.
A tagged card with missing/empty/malformed/duplicate public content, overlong text,
invalid update date, a missing position, or an unknown lane fails the whole export.
Public title/text must be plain single-line text without URLs, markup or `#` markers;
this deliberately rejects all URLs, including Favro links, and card references. The
export is built and validated before writing stdout or opening `--out`, so invalid
content leaves an existing reviewed export untouched. Errors identify the card without
quoting private text. This validation is not a replacement for human review of wording.

The site should render a committed, reviewed JSON export with ordinary text escaping.
Refreshing that file and publishing it are separate authorized actions. Never install
a Favro token in the production site. Do not substitute `get`, `list`, `overview` or
`roadmap list --json` for the public exporter.
