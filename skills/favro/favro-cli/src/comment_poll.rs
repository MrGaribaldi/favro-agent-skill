//! Count-based polling with an explicit full-read mode and caller-owned checkpoints.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Cache {
    pub schema_version: u32,
    pub organization_id: String,
    pub collection_id: String,
    pub comment_counts: HashMap<String, u64>,
}

impl Cache {
    pub fn empty(organization_id: String, collection_id: String) -> Self {
        Self {
            schema_version: 1,
            organization_id,
            collection_id,
            comment_counts: HashMap::new(),
        }
    }

    pub fn validate(&self, organization_id: &str, collection_id: &str) -> Result<(), String> {
        if self.schema_version != 1
            || self.organization_id != organization_id
            || self.collection_id != collection_id
        {
            return Err("Comment cache version or project does not match; use a separate cache for this project.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Thread {
    #[serde(rename = "cardCommonId")]
    pub card_common_id: String,
    pub comments: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct Poll {
    pub cards: Vec<Value>,
    pub threads: Vec<Thread>,
    pub skipped_unchanged: usize,
    pub next_cache: Cache,
}

pub fn poll(
    cards: Vec<Value>,
    previous: Cache,
    full_comments: bool,
    mut fetch: impl FnMut(&str) -> Result<Vec<Value>, String>,
) -> Result<Poll, String> {
    let mut result = Poll {
        cards: Vec::new(),
        threads: Vec::new(),
        skipped_unchanged: 0,
        next_cache: previous,
    };
    let mut seen = HashSet::new();
    for card in cards {
        let id = card
            .get("cardCommonId")
            .and_then(Value::as_str)
            .ok_or("Card listing omitted cardCommonId")?
            .to_owned();
        // A card on several boards still has only one shared comment thread.
        if !seen.insert(id.clone()) {
            continue;
        }
        let count = card.get("numComments").and_then(Value::as_u64);
        let unchanged =
            count.is_some_and(|count| result.next_cache.comment_counts.get(&id) == Some(&count));
        result.cards.push(card);
        if !full_comments && unchanged {
            result.skipped_unchanged += 1;
            continue;
        }
        let comments = if !full_comments && count == Some(0) {
            Vec::new()
        } else {
            fetch(&id)?
        };
        // Use the fetched count in case comments changed since card discovery.
        result
            .next_cache
            .comment_counts
            .insert(id.clone(), comments.len() as u64);
        result.threads.push(Thread {
            card_common_id: id,
            comments,
        });
    }
    // Forget removed/archived cards so reappearing cards get a fresh read.
    result
        .next_cache
        .comment_counts
        .retain(|id, _| seen.contains(id));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cache() -> Cache {
        Cache::empty("org".into(), "collection".into())
    }
    fn card(count: Value) -> Value {
        json!({"cardCommonId":"common", "numComments":count})
    }

    #[test]
    fn unchanged_counts_skip_fetch_but_full_mode_finds_same_count_edits() {
        let mut previous = cache();
        previous.comment_counts.insert("common".into(), 1);
        let skipped = poll(vec![card(json!(1))], previous.clone(), false, |_| {
            panic!("unnecessary request")
        })
        .unwrap();
        assert_eq!(skipped.skipped_unchanged, 1);
        assert!(skipped.threads.is_empty());
        let full = poll(vec![card(json!(1))], previous, true, |id| {
            assert_eq!(id, "common");
            Ok(vec![json!({"comment":"edited"})])
        })
        .unwrap();
        assert_eq!(full.threads[0].comments[0]["comment"], "edited");
    }

    #[test]
    fn increased_decreased_and_unknown_counts_fetch_without_resolving_cards() {
        for count in [json!(2), json!(0), Value::Null] {
            let mut previous = cache();
            previous.comment_counts.insert("common".into(), 1);
            let mut calls = 0;
            let result = poll(vec![card(count.clone())], previous, false, |id| {
                assert_eq!(id, "common");
                calls += 1;
                Ok(vec![json!({"comment":"new"})])
            })
            .unwrap();
            assert_eq!(calls, usize::from(count != json!(0)));
            assert_eq!(result.threads.len(), 1);
        }
    }

    #[test]
    fn first_empty_thread_is_cached_without_request_and_full_mode_reads_it() {
        let result = poll(vec![card(json!(0))], cache(), false, |_| {
            panic!("empty thread")
        })
        .unwrap();
        assert_eq!(result.next_cache.comment_counts["common"], 0);
        let mut calls = 0;
        poll(vec![card(json!(0))], result.next_cache, true, |_| {
            calls += 1;
            Ok(vec![])
        })
        .unwrap();
        assert_eq!(calls, 1);
    }

    #[test]
    fn duplicate_board_instances_fetch_once_and_record_fetched_count() {
        let mut calls = 0;
        let result = poll(vec![card(json!(1)), card(json!(1))], cache(), false, |_| {
            calls += 1;
            Ok(vec![json!({}), json!({})])
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(result.cards.len(), 1);
        assert_eq!(result.next_cache.comment_counts["common"], 2);
    }

    #[test]
    fn failed_fetch_returns_no_checkpoint_and_preserves_input_cache() {
        let previous = cache();
        assert!(poll(vec![card(json!(1))], previous.clone(), false, |_| Err(
            "rate limit".into()
        ))
        .is_err());
        assert!(previous.comment_counts.is_empty());
    }

    #[test]
    fn cache_is_project_scoped_and_removed_cards_are_forgotten() {
        let mut previous = cache();
        assert!(previous.validate("other", "collection").is_err());
        assert!(previous.validate("org", "other").is_err());
        previous.comment_counts.insert("gone".into(), 1);
        let result = poll(vec![], previous, false, |_| unreachable!()).unwrap();
        assert!(result.next_cache.comment_counts.is_empty());
    }
}
