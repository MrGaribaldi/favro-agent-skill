//! Shared ensure-board structure operations; tested through the same remote boundary as roadmaps.
use serde_json::{json, Value};

pub trait Remote {
    fn entities(&self, path: &str, params: &[(&str, String)]) -> Result<Vec<Value>, String>;
    fn request_json(
        &self,
        method: &str,
        path: &str,
        params: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, String>;
}

pub fn resolve(
    api: &impl Remote,
    collection_id: &str,
    collection: &str,
    board: &str,
    lanes: &[String],
    create: bool,
    unscheduled: bool,
) -> Result<(String, serde_json::Map<String, Value>), String> {
    let widgets = api.entities("/widgets", &[("collectionId", collection_id.to_string())])?;
    let matches: Vec<_> = widgets
        .iter()
        .filter(|w| {
            w["name"].as_str() == Some(board) && w["archived"] != true && w["type"] == "board"
        })
        .collect();
    if matches.len() > 1 {
        return Err(format!(
            "Multiple live boards named {board:?}; resolve duplicate names first"
        ));
    }
    let widget = match widgets
        .iter()
        .find(|widget| {
            widget.get("name").and_then(Value::as_str) == Some(board)
                && widget.get("archived").and_then(Value::as_bool) != Some(true)
                && widget.get("type").and_then(Value::as_str) == Some("board")
        })
        .cloned()
    {
        Some(value) => value,
        None if create => {
            api.request_json(
                "POST",
                "/widgets",
                &[],
                Some(json!({
                    "collectionId": collection_id,
                    "name": board,
                    "type": "board",
                    "color": "blue"
                })),
            )?
        }
        None => return Err(format!(
            "Board {board:?} does not exist in collection {collection:?}. Check the name or create it explicitly with `favro ensure-board --board {board:?}`."
        )),
    };
    let widget_id = widget
        .get("widgetCommonId")
        .and_then(|value| value.as_str())
        .filter(|id| !id.is_empty())
        .ok_or("Favro returned a board without widgetCommonId")?
        .to_string();

    let mut existing = serde_json::Map::new();
    for column in api.entities("/columns", &[("widgetCommonId", widget_id.clone())])? {
        if let (Some(name), Some(id)) = (
            column.get("name").and_then(|value| value.as_str()),
            column.get("columnId").and_then(|value| value.as_str()),
        ) {
            existing.insert(name.to_string(), json!(id));
        }
    }
    if create {
        for lane in lanes
            .iter()
            .map(String::as_str)
            .chain(unscheduled.then_some("unscheduled"))
        {
            if !existing.contains_key(lane) {
                let body = api.request_json(
                    "POST",
                    "/columns",
                    &[],
                    Some(json!({"widgetCommonId": widget_id, "name": lane})),
                )?;
                let id = body
                    .get("columnId")
                    .and_then(|value| value.as_str())
                    .filter(|id| !id.is_empty())
                    .ok_or("Favro created a column without columnId")?
                    .to_string();
                existing.insert(lane.to_string(), json!(id));
            }
        }
    }

    Ok((widget_id, existing))
}
