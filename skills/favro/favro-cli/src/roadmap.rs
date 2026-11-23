//! Private roadmap coordination and an allowlisted, fail-closed public export.
use crate::{
    attachments,
    boards::{self, Remote},
    config::Roadmap,
};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::PathBuf,
};

const PUBLIC_START: &str = "<!-- ROADMAP PUBLIC -->";
const PUBLIC_END: &str = "<!-- /ROADMAP PUBLIC -->";
const WORK_START: &str = "<!-- ROADMAP WORK -->";
const WORK_END: &str = "<!-- /ROADMAP WORK -->";

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ensure the configured roadmap board, ordered lanes and public tag. Idempotent.
    Init,
    /// Create an internal feature/theme card. Never published automatically.
    Add {
        #[arg(long)]
        title: String,
        #[arg(long)]
        lane: Option<String>,
        #[arg(long, num_args = 1..)]
        work: Vec<String>,
        #[arg(long, default_value = "")]
        body: String,
    },
    /// Maintain the internal list of linked work cards without changing other text.
    Link {
        #[arg(long)]
        card: String,
        #[arg(long, required = true, num_args = 1..)]
        work: Vec<String>,
        #[arg(long)]
        unlink: bool,
    },
    /// Replace the public block and add the configured public tag in the same write.
    SetPublic {
        #[arg(long)]
        card: String,
        #[arg(long)]
        title: String,
        /// One to three sentences in Bokmål, plain text without links or card references.
        #[arg(long)]
        text: String,
    },
    /// Clear the public block and remove the public tag in the same write.
    Unpublish {
        #[arg(long)]
        card: String,
    },
    /// INTERNAL listing: includes work-card lanes; not suitable for public distribution.
    List {
        #[arg(long)]
        lane: Option<String>,
        #[arg(long)]
        public_only: bool,
        #[arg(long)]
        json: bool,
    },
    /// Export only approved public copy; no internal fields. Read access only.
    Export {
        #[arg(long, required = true)]
        public: bool,
        #[arg(long, default_value = "json", value_parser = ["json"])]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// Dates are explicit public-copy metadata, not internal card/activity timestamps.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PublicCopy {
    title: String,
    text: String,
    updated: String,
}
#[derive(Serialize)]
struct PublicItem {
    id: String,
    #[serde(flatten)]
    copy: PublicCopy,
}
#[derive(Serialize)]
struct PublicLane {
    key: String,
    name: String,
    items: Vec<PublicItem>,
}
#[derive(Serialize)]
struct Export {
    generated: String,
    lanes: Vec<PublicLane>,
}

fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Favro response is missing {key}"))
}
fn description(v: &Value) -> Result<String, String> {
    v.get("detailedDescription")
        .and_then(Value::as_str)
        .map(crate::checkboxes_to_markdown)
        .ok_or_else(|| "Favro response is missing detailedDescription".into())
}

/// Unlike the older Notes helper, refuse ambiguous/truncated blocks before changing anything.
fn block<'a>(text: &'a str, start: &str, end: &str) -> Result<Option<&'a str>, String> {
    let starts: Vec<_> = text.match_indices(start).collect();
    let ends: Vec<_> = text.match_indices(end).collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([(a, _)], [(b, _)]) if a + start.len() <= *b => Ok(Some(text[a + start.len()..*b].trim())),
        _ => {
            Err("Malformed or duplicate roadmap managed block; repair it before continuing".into())
        }
    }
}
fn replace(text: &str, start: &str, end: &str, content: Option<&str>) -> Result<String, String> {
    block(text, start, end)?;
    match content {
        Some(content) => Ok(crate::place_managed_block(
            text,
            start,
            end,
            &format!("{start}\n{content}\n{end}"),
            false,
        )),
        None => Ok(crate::remove_managed_block(text, start, end)),
    }
}
fn references(values: &[String]) -> Result<BTreeSet<u64>, String> {
    values
        .iter()
        .map(|v| {
            v.strip_prefix('#')
                .unwrap_or(v)
                .parse::<u64>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| "Work references must be positive #card numbers".into())
        })
        .collect()
}
/// Decode presentation changes Favro makes to managed-block lines, not user escapes
/// such as `\n`. Strip one list marker, whitespace and Markdown hard breaks; only
/// ASCII punctuation may be backslash-escaped by Markdown. Consumers still validate
/// the decoded reference/public fields strictly.
fn managed_line(line: &str) -> String {
    let mut line = line.trim();
    // An odd terminal slash denotes a Markdown hard break. An escaped literal
    // backslash (even count) remains data and will fail field validation.
    if line.chars().rev().take_while(|ch| *ch == '\\').count() % 2 == 1 {
        line = line[..line.len() - 1].trim_end();
    }
    let mut chars = line.chars().peekable();
    let mut decoded = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek().is_some_and(char::is_ascii_punctuation) {
            decoded.push(chars.next().unwrap());
        } else {
            decoded.push(ch);
        }
    }
    if let Some(rest) = decoded.strip_prefix(['-', '*', '+', '•']) {
        if rest.starts_with(char::is_whitespace) {
            return rest.trim().to_string();
        }
    }
    decoded
}
fn work_links(text: &str) -> Result<BTreeSet<u64>, String> {
    let Some(body) = block(text, WORK_START, WORK_END)? else {
        return Ok(BTreeSet::new());
    };
    let values: Vec<String> = body
        .lines()
        .map(managed_line)
        .filter(|line| !line.is_empty())
        .collect();
    references(&values)
}
fn write_links(text: &str, links: &BTreeSet<u64>) -> Result<String, String> {
    let lines = links
        .iter()
        .map(|n| format!("- #{n}"))
        .collect::<Vec<_>>()
        .join("\n");
    replace(
        text,
        WORK_START,
        WORK_END,
        (!lines.is_empty()).then_some(lines.as_str()),
    )
}
fn validate_copy(copy: &PublicCopy, config: &Roadmap) -> Result<(), String> {
    for (name, text, limit) in [
        ("title", &copy.title, 120),
        ("text", &copy.text, config.max_public_chars),
    ] {
        if text.trim().is_empty() || text.chars().count() > limit {
            return Err(format!("Public {name} must contain 1–{limit} characters"));
        }
        let lower = text.to_lowercase();
        // Plain copy only. Ban URLs altogether, including scheme-less Favro links,
        // and all # markers (also catches spaced/escaped card references).
        if text.chars().any(char::is_control)
            || text.contains(['#', '<', '>', '*', '_', '`', '[', ']', '\\'])
            || [
                "favro.com",
                "://",
                "mailto:",
                "http:",
                "https:",
                "www.",
                "](",
                "&#",
                "\\u",
                "%23",
                "%2f",
            ]
            .iter()
            .any(|p| lower.contains(p))
        {
            return Err(format!(
                "Public {name} must be plain text without URLs, markup or #card references"
            ));
        }
    }
    validate_date(&copy.updated)?;
    Ok(())
}
fn public_copy(text: &str, config: &Roadmap) -> Result<Option<PublicCopy>, String> {
    let Some(body) = block(text, PUBLIC_START, PUBLIC_END)? else {
        return Ok(None);
    };
    let lines: Vec<_> = body
        .lines()
        .map(managed_line)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() != 3 {
        return Err("Public block must contain Title, Text and Updated lines".into());
    }
    let copy = PublicCopy {
        title: lines[0]
            .strip_prefix("Title: ")
            .ok_or("Public block has no Title line")?
            .into(),
        text: lines[1]
            .strip_prefix("Text: ")
            .ok_or("Public block has no Text line")?
            .into(),
        updated: lines[2]
            .strip_prefix("Updated: ")
            .ok_or("Public block has no Updated line")?
            .into(),
    };
    validate_copy(&copy, config)?;
    Ok(Some(copy))
}
fn leap(y: u64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
fn month_days(y: u64) -> [u64; 12] {
    [
        31,
        if leap(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ]
}
fn validate_date(s: &str) -> Result<(), String> {
    let parts: Vec<_> = s.split('-').collect();
    let valid = (|| {
        if parts.len() != 3
            || parts[0].len() != 4
            || parts[1].len() != 2
            || parts[2].len() != 2
            || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        let y = parts[0].parse::<u64>().ok()?;
        let m = parts[1].parse::<usize>().ok()?;
        let d = parts[2].parse::<u64>().ok()?;
        (y >= 1970 && (1..=12).contains(&m) && (1..=month_days(y)[m - 1]).contains(&d))
            .then_some(())
    })()
    .is_some();
    if valid {
        Ok(())
    } else {
        Err("Public Updated date must be a valid YYYY-MM-DD date".into())
    }
}
fn date_from_days(mut days: u64) -> String {
    let mut year = 1970;
    loop {
        let n = if leap(year) { 366 } else { 365 };
        if days < n {
            break;
        }
        days -= n;
        year += 1;
    }
    let mut month = 1;
    for n in month_days(year) {
        if days < n {
            break;
        }
        days -= n;
        month += 1;
    }
    format!("{year:04}-{month:02}-{:02}", days + 1)
}
fn today() -> String {
    date_from_days(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / 86400,
    )
}
fn short_id(common: &str) -> String {
    // FNV-1a is stable across CLI/compiler releases; not used as a security primitive.
    let mut hash = 0xcbf29ce484222325u64;
    for b in common.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("r{hash:016x}")
}
fn tag_id(api: &impl Remote, name: &str, create: bool) -> Result<Option<String>, String> {
    let matches: Vec<Value> = api
        .entities("/tags", &[])?
        .into_iter()
        .filter(|t| t["name"].as_str() == Some(name))
        .collect();
    match matches.as_slice() {
        [tag] => Ok(Some(field(tag, "tagId")?.into())),
        [] if create => Ok(Some(
            field(
                &api.request_json(
                    "POST",
                    "/tags",
                    &[],
                    Some(json!({"name":name,"color":"blue"})),
                )?,
                "tagId",
            )?
            .into(),
        )),
        [] => Ok(None),
        _ => Err("Multiple tags match roadmap.public_tag; resolve the ambiguity first".into()),
    }
}
fn tagged(card: &Value, tag: Option<&str>) -> Result<bool, String> {
    let tags = card
        .get("tags")
        .and_then(Value::as_array)
        .ok_or("Favro response is missing tags")?;
    if tags.iter().any(|v| !v.is_string()) {
        return Err("Favro returned invalid card tags".into());
    }
    Ok(tag.is_some_and(|id| tags.iter().any(|t| t.as_str() == Some(id))))
}
fn lane_name<'a>(
    config: &'a Roadmap,
    cols: &serde_json::Map<String, Value>,
    card: &Value,
) -> Option<&'a str> {
    config
        .lanes
        .iter()
        .find(|l| cols.get(*l) == card.get("columnId"))
        .map(String::as_str)
}
fn sorted_cards(mut cards: Vec<Value>) -> Result<Vec<Value>, String> {
    cards.retain(|c| c["archived"].as_bool() != Some(true) && c["isLane"].as_bool() != Some(true));
    for c in &cards {
        field(c, "cardCommonId")?;
        if !c
            .get("listPosition")
            .or_else(|| c.get("position"))
            .is_some_and(Value::is_number)
        {
            return Err("Roadmap card is missing its position".into());
        }
    }
    cards.sort_by(|a, b| {
        let pos = |c: &Value| {
            c.get("listPosition")
                .or_else(|| c.get("position"))
                .and_then(Value::as_f64)
                .unwrap()
        };
        pos(a)
            .total_cmp(&pos(b))
            .then_with(|| a["cardCommonId"].as_str().cmp(&b["cardCommonId"].as_str()))
    });
    Ok(cards)
}
fn select(api: &impl Remote, widget: &str, reference: &str) -> Result<Value, String> {
    let (param, value) = crate::card_reference_query(reference)?;
    let cards = api.entities("/cards", &[(param, value)])?;
    let selected =
        crate::select_card_instance(cards, &HashSet::from([widget.into()]), reference, "roadmap")?;
    if selected["archived"].as_bool() == Some(true) {
        return Err("Unarchive the roadmap card before editing it".into());
    }
    let id = field(&selected, "cardId")?;
    let fresh = api.request_json("GET", &format!("/cards/{id}"), &[], None)?;
    if fresh["cardId"] != selected["cardId"] || fresh["widgetCommonId"].as_str() != Some(widget) {
        return Err("Roadmap card changed board during lookup; retry after inspecting it".into());
    }
    Ok(fresh)
}
/// One Markdown PUT carries text and tag state; verify the exact instance afterwards.
fn write(
    api: &impl Remote,
    config: &Roadmap,
    before: &Value,
    text: &str,
    tag_change: Option<(&str, bool)>,
) -> Result<(), String> {
    let expected_work = work_links(text)?;
    let expected_public = public_copy(text, config)?;
    let text = attachments::preserve_in_markdown_except(
        before,
        &crate::checkboxes_to_markdown(text),
        &[],
    )?;
    let id = field(before, "cardId")?;
    let mut body = json!({"detailedDescription":text});
    if let Some((tag, add)) = tag_change {
        body[if add { "addTags" } else { "removeTags" }] = json!([tag]);
    }
    api.request_json(
        "PUT",
        &format!("/cards/{id}"),
        &crate::md_format(),
        Some(body),
    )?;
    let after = api
        .request_json("GET", &format!("/cards/{id}"), &[], None)
        .map_err(|e| format!("Roadmap card was updated but verification failed: {e}"))?;
    if after["cardId"] != before["cardId"] {
        return Err("Roadmap card was updated but verification returned the wrong instance".into());
    }
    attachments::verify(before, &after, &[])?;
    if let Some((tag, add)) = tag_change {
        // addTags/removeTags accept names, while the response carries IDs.
        let id = tag_id(api, tag, false)?;
        if tagged(&after, id.as_deref())? != add {
            return Err("Roadmap card was updated but public tag verification failed".into());
        }
    }
    // Verify managed content too: a successful HTTP response alone isn't a round trip.
    // Compare meaning, not Markdown spelling: Favro changes bullets, escapes,
    // indentation and blank lines between the write and this plaintext read.
    let verify = || -> Result<(), String> {
        let actual = description(&after)?;
        if work_links(&actual)? != expected_work || public_copy(&actual, config)? != expected_public
        {
            return Err("managed content differs from the requested values".into());
        }
        Ok(())
    };
    verify().map_err(|error| {
        format!("Roadmap card was updated but managed block verification failed: {error}")
    })?;
    Ok(())
}
fn export(
    config: &Roadmap,
    cols: &serde_json::Map<String, Value>,
    cards: &[Value],
    tag: Option<&str>,
    generated: &str,
) -> Result<String, String> {
    validate_date(generated)?;
    let mut lanes: Vec<PublicLane> = config
        .lanes
        .iter()
        .zip(&config.lane_keys)
        .map(|(name, key)| PublicLane {
            key: key.clone(),
            name: name.clone(),
            items: vec![],
        })
        .collect();
    let mut seen = HashSet::new();
    for card in cards {
        if !tagged(card, tag)? {
            continue;
        }
        let common = field(card, "cardCommonId")?;
        let item = (|| {
            let copy = public_copy(&description(card)?, config)?
                .ok_or("Tagged public card has no public block")?;
            let name = lane_name(config, cols, card)
                .ok_or("Public card is outside the configured roadmap lanes")?;
            let id = short_id(common);
            if !seen.insert(id.clone()) {
                return Err("Duplicate public ID; inspect duplicate cards or ID collision".into());
            }
            lanes
                .iter_mut()
                .find(|l| l.name == name)
                .unwrap()
                .items
                .push(PublicItem { id, copy });
            Ok::<_, String>(())
        })();
        item.map_err(|e| format!("Cannot export roadmap card {common}: {e}"))?;
    }
    serde_json::to_string_pretty(&Export {
        generated: generated.into(),
        lanes,
    })
    .map_err(|e| e.to_string())
}

pub fn run(
    api: &impl Remote,
    collection_id: &str,
    collection: &str,
    config: &Roadmap,
    command: Command,
) -> Result<Option<String>, String> {
    config.validate()?;
    // All inspection/export paths request create=false; no structure/tag writes.
    let init = matches!(command, Command::Init);
    let (widget, cols) = boards::resolve(
        api,
        collection_id,
        collection,
        &config.board,
        &config.lanes,
        init,
        false,
    )?;
    for lane in &config.lanes {
        if !cols.contains_key(lane) {
            return Err(format!(
                "Roadmap lane {lane:?} is missing; run roadmap init"
            ));
        }
    }
    match command {
        Command::Init => {
            // Inserting/reordering a column shifts its neighbours. Refresh after each move.
            for (position, name) in config.lanes.iter().enumerate() {
                let columns = api.entities("/columns", &[("widgetCommonId", widget.clone())])?;
                let column = columns
                    .iter()
                    .find(|c| c["columnId"] == cols[name])
                    .ok_or("Roadmap column disappeared")?;
                if column["position"].as_u64() != Some(position as u64) {
                    api.request_json(
                        "PUT",
                        &format!("/columns/{}", field(column, "columnId")?),
                        &[],
                        Some(json!({"position":position})),
                    )?;
                }
            }
            tag_id(api, &config.public_tag, true)?;
            Ok(Some(format!(
                "Ensured roadmap {:?} ({widget})",
                config.board
            )))
        }
        Command::Add {
            title,
            lane,
            work,
            body,
        } => {
            if title.trim().is_empty() {
                return Err("Roadmap title must not be empty".into());
            }
            let lane = lane.unwrap_or_else(|| config.lanes[0].clone());
            let column = cols.get(&lane).ok_or("Unknown roadmap lane")?;
            let body = crate::unescape(&body);
            // User text cannot pre-populate publishable metadata accidentally.
            if body.contains("<!-- ROADMAP") || body.contains("<!-- /ROADMAP") {
                return Err("Use roadmap link/set-public for managed blocks".into());
            }
            let body = write_links(&body, &references(&work)?)?;
            let result = api.request_json("POST","/cards",&crate::md_format(),Some(json!({"widgetCommonId":widget,"columnId":column,"name":title,"detailedDescription":body})))?;
            let card = result
                .get("entities")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .unwrap_or(&result);
            Ok(Some(format!(
                "Created roadmap #{} [{}]",
                card["sequentialId"],
                field(card, "cardCommonId")?
            )))
        }
        Command::Link { card, work, unlink } => {
            let requested = references(&work)?;
            let before = select(api, &widget, &card)?;
            let text = description(&before)?;
            let mut links = work_links(&text)?;
            for n in requested {
                if unlink {
                    links.remove(&n);
                } else {
                    links.insert(n);
                }
            }
            write(api, config, &before, &write_links(&text, &links)?, None)?;
            Ok(Some(format!("Updated work links on {card}")))
        }
        Command::SetPublic { card, title, text } => {
            let copy = PublicCopy {
                title: crate::unescape(&title).trim().into(),
                text: crate::unescape(&text).trim().into(),
                updated: today(),
            };
            validate_copy(&copy, config)?;
            let before = select(api, &widget, &card)?;
            let content = format!(
                "Title: {}\n\nText: {}\n\nUpdated: {}",
                copy.title, copy.text, copy.updated
            );
            let text = replace(
                &description(&before)?,
                PUBLIC_START,
                PUBLIC_END,
                Some(&content),
            )?;
            write(
                api,
                config,
                &before,
                &text,
                Some((&config.public_tag, true)),
            )?;
            Ok(Some(format!("Updated public copy on {card}")))
        }
        Command::Unpublish { card } => {
            let before = select(api, &widget, &card)?;
            let text = replace(&description(&before)?, PUBLIC_START, PUBLIC_END, None)?;
            write(
                api,
                config,
                &before,
                &text,
                Some((&config.public_tag, false)),
            )?;
            Ok(Some(format!("Unpublished {card}")))
        }
        Command::Export { out, .. } => {
            let cards = sorted_cards(api.entities(
                "/cards",
                &[
                    ("widgetCommonId", widget),
                    ("descriptionFormat", "plaintext".into()),
                ],
            )?)?;
            let tag = tag_id(api, &config.public_tag, false)?;
            let tag =
                tag.ok_or("Roadmap public tag is missing; run roadmap init before exporting")?;
            let output = export(config, &cols, &cards, Some(&tag), &today())?;
            // Construct and validate everything before opening/truncating an output file.
            if let Some(path) = out {
                std::fs::write(&path, format!("{output}\n"))
                    .map_err(|e| format!("Cannot write export {}: {e}", path.display()))?;
                Ok(None)
            } else {
                Ok(Some(output))
            }
        }
        Command::List {
            lane,
            public_only,
            json: as_json,
        } => {
            if lane.as_ref().is_some_and(|l| !config.lanes.contains(l)) {
                return Err("Unknown roadmap lane".into());
            }
            let cards = sorted_cards(api.entities(
                "/cards",
                &[
                    ("widgetCommonId", widget),
                    ("descriptionFormat", "plaintext".into()),
                ],
            )?)?;
            let tag = tag_id(api, &config.public_tag, false)?;
            let widgets = api.entities("/widgets", &[("collectionId", collection_id.into())])?;
            let mut work_cards = HashMap::new();
            let mut work_columns = HashMap::new();
            let mut output = Vec::new();
            for name in &config.lanes {
                if lane.as_ref().is_some_and(|l| l != name) {
                    continue;
                }
                for card in cards
                    .iter()
                    .filter(|c| lane_name(config, &cols, c) == Some(name))
                {
                    let desc = description(card)?;
                    let published =
                        tagged(card, tag.as_deref())? && public_copy(&desc, config)?.is_some();
                    if public_only && !published {
                        continue;
                    }
                    let mut work = Vec::new();
                    for n in work_links(&desc)? {
                        let instances = if let Some(v) = work_cards.get(&n) {
                            v
                        } else {
                            let fetched =
                                api.entities("/cards", &[("cardSequentialId", n.to_string())])?;
                            work_cards.insert(n, fetched);
                            &work_cards[&n]
                        };
                        let mut placements = Vec::new();
                        for c in instances {
                            let Some(w) = widgets.iter().find(|w| {
                                w["widgetCommonId"] == c["widgetCommonId"]
                                    && w["archived"] != true
                                    && w["type"] == "board"
                            }) else {
                                continue;
                            };
                            let wid = field(w, "widgetCommonId")?;
                            if !work_columns.contains_key(wid) {
                                work_columns.insert(
                                    wid.to_string(),
                                    api.entities("/columns", &[("widgetCommonId", wid.into())])?,
                                );
                            }
                            let lane = work_columns[wid]
                                .iter()
                                .find(|l| l["columnId"] == c["columnId"])
                                .and_then(|l| l["name"].as_str());
                            placements.push(json!({"board":w["name"],"lane":lane,"archived":c["archived"].as_bool().unwrap_or(false)}));
                        }
                        work.push(json!({"number":n,"placements":placements}));
                    }
                    output.push(json!({"lane":name,"number":card["sequentialId"],"id":card["cardCommonId"],"title":card["name"],"public":published,"work":work}));
                }
            }
            if as_json {
                Ok(Some(serde_json::to_string_pretty(&output).unwrap()))
            } else {
                Ok(Some(
                    output
                        .iter()
                        .map(|c| {
                            format!(
                                "[{}] #{} {} public={} work={}",
                                c["lane"].as_str().unwrap(),
                                c["number"],
                                c["title"].as_str().unwrap_or("?"),
                                c["public"],
                                c["work"]
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests;
