use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct Data {
    widgets: Vec<Value>,
    columns: Vec<Value>,
    tags: Vec<Value>,
    cards: Vec<Value>,
    calls: Vec<(String, String, Option<Value>)>,
    lose_files: bool,
    lose_tag: bool,
}
#[derive(Default)]
struct Mock {
    data: RefCell<Data>,
}
impl Remote for Mock {
    fn entities(&self, path: &str, params: &[(&str, String)]) -> Result<Vec<Value>, String> {
        let mut d = self.data.borrow_mut();
        d.calls.push(("GET".into(), path.into(), None));
        let all = match path {
            "/widgets" => &d.widgets,
            "/columns" => &d.columns,
            "/cards" => &d.cards,
            "/tags" => &d.tags,
            _ => panic!("Unexpected GET {path}"),
        };
        Ok(all
            .iter()
            .filter(|v| {
                params.iter().all(|(k, want)| match *k {
                    "descriptionFormat" => true,
                    "cardSequentialId" => v["sequentialId"].as_u64() == want.parse::<u64>().ok(),
                    _ => v[*k].as_str() == Some(want),
                })
            })
            .cloned()
            .collect())
    }
    fn request_json(
        &self,
        method: &str,
        path: &str,
        params: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut d = self.data.borrow_mut();
        d.calls.push((method.into(), path.into(), body.clone()));
        let mut b = body.unwrap_or(json!({}));
        let n = d.calls.len();
        match (method, path) {
            ("POST", "/widgets") => {
                b["widgetCommonId"] = json!(format!("widget-{n}"));
                d.widgets.push(b.clone());
            }
            ("POST", "/columns") => {
                b["columnId"] = json!(format!("column-{n}"));
                b["position"] = json!(d.columns.len());
                d.columns.push(b.clone());
            }
            ("POST", "/tags") => {
                b["tagId"] = json!(format!("tag-{n}"));
                d.tags.push(b.clone());
            }
            ("POST", "/cards") => {
                assert_eq!(params, &crate::md_format());
                b["cardId"] = json!(format!("instance-{n}"));
                b["cardCommonId"] = json!(format!("common-{n}"));
                b["sequentialId"] = json!(n);
                b["archived"] = json!(false);
                b["tags"] = json!([]);
                b["position"] = json!(n);
                d.cards.push(b.clone());
            }
            ("GET", p) if p.starts_with("/cards/") => {
                return Ok(d
                    .cards
                    .iter()
                    .find(|c| c["cardId"].as_str() == Some(&p[7..]))
                    .unwrap()
                    .clone());
            }
            ("PUT", p) if p.starts_with("/cards/") => {
                assert_eq!(params, &crate::md_format());
                let index = d
                    .cards
                    .iter()
                    .position(|c| c["cardId"].as_str() == Some(&p[7..]))
                    .unwrap();
                for (op, add) in [("addTags", true), ("removeTags", false)] {
                    if let Some(tags) = b.get(op).and_then(Value::as_array) {
                        for name in tags {
                            let tag = match d.tags.iter().find(|t| t["name"] == *name) {
                                Some(t) => t["tagId"].clone(),
                                None => {
                                    let id = json!(format!("tag-{}", d.tags.len()));
                                    d.tags.push(json!({"tagId":id,"name":name}));
                                    id
                                }
                            };
                            let lose_tag = d.lose_tag;
                            let list = d.cards[index]["tags"].as_array_mut().unwrap();
                            list.retain(|v| *v != tag);
                            if add && !lose_tag {
                                list.push(tag);
                            }
                        }
                    }
                }
                // Behave like Favro plaintext: attachment nodes aren't in the body,
                // but a missing node in the write loses the corresponding upload.
                let text = b["detailedDescription"].as_str().unwrap();
                if let Some(files) = d.cards[index]["attachments"].as_array() {
                    for file in files {
                        assert!(
                            text.contains(file["fileURL"].as_str().unwrap()),
                            "attachment not reconstructed in Markdown"
                        );
                    }
                }
                let text = text
                    .lines()
                    .filter(|l| !l.starts_with("!["))
                    .collect::<Vec<_>>()
                    .join("\n");
                d.cards[index]["detailedDescription"] = json!(text);
                if d.lose_files {
                    d.cards[index]["attachments"] = json!([]);
                }
                return Ok(d.cards[index].clone());
            }
            ("PUT", p) if p.starts_with("/columns/") => {
                let index = d
                    .columns
                    .iter()
                    .position(|c| c["columnId"].as_str() == Some(&p[9..]))
                    .unwrap();
                let col = d.columns.remove(index);
                d.columns
                    .insert(b["position"].as_u64().unwrap() as usize, col);
                for (i, c) in d.columns.iter_mut().enumerate() {
                    c["position"] = json!(i);
                }
            }
            _ => panic!("Unexpected {method} {path}"),
        }
        Ok(b)
    }
}
fn run_mock(api: &Mock, command: Command) -> Result<Option<String>, String> {
    run(api, "collection", "Example", &Roadmap::default(), command)
}
fn ready() -> Mock {
    let api = Mock::default();
    run_mock(&api, Command::Init).unwrap();
    api.data.borrow_mut().calls.clear();
    api
}
fn seed(api: &Mock, id: &str, position: f64) {
    let mut d = api.data.borrow_mut();
    let widget = d.widgets[0]["widgetCommonId"].clone();
    let column = d.columns[0]["columnId"].clone();
    d.cards.push(json!({"cardCommonId":id,"cardId":id,"sequentialId":123,"widgetCommonId":widget,"columnId":column,"listPosition":position,"archived":false,"name":"INTERNAL NAME","detailedDescription":"INTERNAL BODY\n☑ Keep human tick","tags":[],"attachments":[{"name":"internal.pdf","fileURL":"https://private.example/secret"}],"assignments":[{"userId":"INTERNAL USER"}],"comments":["INTERNAL COMMENT"]}));
}
fn publish(api: &Mock, id: &str) {
    run_mock(
        api,
        Command::SetPublic {
            card: id.into(),
            title: "Offentlig tittel".into(),
            text: "Dette er offentlig tekst.".into(),
        },
    )
    .unwrap();
}
fn export_mock(api: &Mock) -> Result<String, String> {
    Ok(run_mock(
        api,
        Command::Export {
            public: true,
            format: "json".into(),
            out: None,
        },
    )?
    .unwrap())
}
#[test]
fn init_reuses_ensure_board_and_is_idempotent_including_order_and_tag() {
    let api = ready();
    {
        let mut d = api.data.borrow_mut();
        d.columns.reverse();
        for (i, c) in d.columns.iter_mut().enumerate() {
            c["position"] = json!(i);
        }
    }
    run_mock(&api, Command::Init).unwrap();
    {
        let mut d = api.data.borrow_mut();
        assert_eq!(d.widgets.len(), 1);
        assert_eq!(d.columns.len(), 4);
        assert_eq!(d.tags.len(), 1);
        assert_eq!(
            d.columns
                .iter()
                .map(|c| c["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            Roadmap::default().lanes
        );
        d.calls.clear();
    }
    run_mock(&api, Command::Init).unwrap();
    assert!(api
        .data
        .borrow()
        .calls
        .iter()
        .all(|(method, _, _)| method == "GET"));
}
#[test]
fn add_link_publish_and_unpublish_preserve_body_ticks_files_and_other_blocks() {
    let api = ready();
    run_mock(
        &api,
        Command::Add {
            title: "Internal feature".into(),
            lane: None,
            work: vec!["#9".into(), "8".into()],
            body: "Human text\n<!-- RESULT -->evidence<!-- /RESULT -->".into(),
        },
    )
    .unwrap();
    let id = {
        let mut d = api.data.borrow_mut();
        let c = &mut d.cards[0];
        c["attachments"] = json!([{"name":"evidence.pdf","fileURL":"https://example.test/file"}]);
        c["detailedDescription"] = json!(format!(
            "{}\n☑ Preserve tick",
            c["detailedDescription"].as_str().unwrap()
        ));
        c["cardCommonId"].as_str().unwrap().to_string()
    };
    run_mock(
        &api,
        Command::Link {
            card: id.clone(),
            work: vec!["#10".into(), "#9".into()],
            unlink: false,
        },
    )
    .unwrap();
    run_mock(
        &api,
        Command::Link {
            card: id.clone(),
            work: vec!["8".into()],
            unlink: true,
        },
    )
    .unwrap();
    publish(&api, &id);
    run_mock(
        &api,
        Command::Link {
            card: id.clone(),
            work: vec!["11".into()],
            unlink: false,
        },
    )
    .unwrap();
    let desc = description(&api.data.borrow().cards[0]).unwrap();
    assert!(desc.contains("Human text"));
    assert!(desc.contains("<!-- RESULT -->evidence<!-- /RESULT -->"));
    assert!(desc.contains("- [x] Preserve tick"));
    assert_eq!(work_links(&desc).unwrap(), BTreeSet::from([9, 10, 11]));
    assert!(public_copy(&desc, &Roadmap::default()).unwrap().is_some());
    run_mock(&api, Command::Unpublish { card: id.clone() }).unwrap();
    run_mock(&api, Command::Unpublish { card: id }).unwrap();
    let d = api.data.borrow();
    assert_eq!(d.cards[0]["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(
        public_copy(&description(&d.cards[0]).unwrap(), &Roadmap::default()).unwrap(),
        None
    );
    assert!(d.cards[0]["tags"].as_array().unwrap().is_empty());
}
#[test]
fn export_is_deterministic_ordered_read_only_and_excludes_every_internal_field() {
    let api = ready();
    seed(&api, "b", 2.5);
    seed(&api, "a", 1.25);
    seed(&api, "private", 0.0);
    publish(&api, "a");
    publish(&api, "b");
    {
        let mut d = api.data.borrow_mut();
        d.cards[2]["detailedDescription"] = d.cards[0]["detailedDescription"].clone();
        d.calls.clear();
    }
    let one = export_mock(&api).unwrap();
    api.data.borrow_mut().cards.reverse();
    let two = export_mock(&api).unwrap();
    assert_eq!(one, two);
    let v: Value = serde_json::from_str(&one).unwrap();
    assert_eq!(v["lanes"][0]["items"][0]["id"], short_id("a"));
    assert_eq!(v["lanes"][0]["items"].as_array().unwrap().len(), 2);
    for private in [
        "INTERNAL",
        "internal.pdf",
        "private.example",
        "123",
        "assignments",
        "cardCommonId",
        "tags",
        "work",
        "attachments",
        "comments",
    ] {
        assert!(!one.contains(private), "leaked {private}");
    }
    assert!(api.data.borrow().calls.iter().all(|(m, _, _)| m == "GET"));
    assert!(api
        .data
        .borrow()
        .calls
        .iter()
        .all(|(_, p, _)| p != "/comments"));
}
#[test]
fn public_tag_and_nonempty_block_are_both_required_archived_cards_never_export() {
    let api = ready();
    seed(&api, "published", 1.0);
    seed(&api, "private", 2.0);
    seed(&api, "archived", 3.0);
    publish(&api, "published");
    publish(&api, "archived");
    api.data.borrow_mut().cards[2]["archived"] = json!(true);
    let v: Value = serde_json::from_str(&export_mock(&api).unwrap()).unwrap();
    assert_eq!(v["lanes"][0]["items"].as_array().unwrap().len(), 1);
    let tag = api.data.borrow().tags[0]["tagId"].clone();
    api.data.borrow_mut().cards[1]["tags"] = json!([tag]);
    assert!(export_mock(&api).unwrap_err().contains("no public block"));
}
#[test]
fn export_fails_closed_for_every_invalid_public_block_and_leaves_output_file_untouched() {
    let api = ready();
    seed(&api, "test", 1.0);
    publish(&api, "test");
    let good = description(&api.data.borrow().cards[0]).unwrap();
    for bad in [
        "", "<!-- ROADMAP PUBLIC -->", "<!-- /ROADMAP PUBLIC -->",
        "<!-- ROADMAP PUBLIC --><!-- /ROADMAP PUBLIC -->",
        "<!-- ROADMAP PUBLIC -->Title: x\nText: x\nUpdated: 2026-11-23<!-- /ROADMAP PUBLIC --><!-- ROADMAP PUBLIC -->",
    ] { api.data.borrow_mut().cards[0]["detailedDescription"]=json!(bad);assert!(export_mock(&api).is_err(),"accepted {bad}"); }
    for bad in [
        "#3758",
        "# 3758",
        "FAVRO.COM/secret",
        "https://example.test",
        "www.example.test",
        "<b>unsafe</b>",
        "a\nb",
        "a\tb",
        "%23abc",
    ] {
        api.data.borrow_mut().cards[0]["detailedDescription"] =
            json!(good.replace("Dette er offentlig tekst.", bad));
        assert!(export_mock(&api).is_err(), "accepted {bad}");
    }
    for bad in [" ".into(), "æ".repeat(401)] {
        api.data.borrow_mut().cards[0]["detailedDescription"] =
            json!(good.replace("Dette er offentlig tekst.", &bad));
        assert!(export_mock(&api).is_err());
    }
    api.data.borrow_mut().cards[0]["detailedDescription"] =
        json!(good.replace("Offentlig tittel", &"ø".repeat(121)));
    assert!(export_mock(&api).is_err());
    api.data.borrow_mut().cards[0]["detailedDescription"] =
        json!(good.replace(&today(), "2026-02-30"));
    assert!(export_mock(&api).is_err());
    let path = std::env::temp_dir().join(format!("favro-roadmap-output-{}", std::process::id()));
    std::fs::write(&path, "previous reviewed export").unwrap();
    assert!(run_mock(
        &api,
        Command::Export {
            public: true,
            format: "json".into(),
            out: Some(path.clone())
        }
    )
    .is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "previous reviewed export"
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn invalid_public_input_is_rejected_before_any_card_write() {
    let api = ready();
    seed(&api, "test", 1.0);
    assert!(run_mock(
        &api,
        Command::SetPublic {
            card: "test".into(),
            title: "title".into(),
            text: "Look at #123".into()
        }
    )
    .is_err());
    assert!(api.data.borrow().calls.iter().all(|(m, _, _)| m == "GET"));
}
#[test]
fn attachment_loss_or_missing_tag_after_write_is_reported() {
    for files in [false, true] {
        let api = ready();
        seed(&api, "test", 1.0);
        if files {
            api.data.borrow_mut().lose_files = true;
        } else {
            api.data.borrow_mut().lose_tag = true;
        }
        let e = run_mock(
            &api,
            Command::SetPublic {
                card: "test".into(),
                title: "Title".into(),
                text: "Text".into(),
            },
        )
        .unwrap_err();
        assert!(
            e.contains(if files {
                "Attachment loss"
            } else {
                "tag verification"
            }),
            "{e}"
        );
    }
}
#[test]
fn inspect_never_creates_missing_boards_or_tags_and_public_unknown_lane_fails() {
    let api = Mock::default();
    assert!(export_mock(&api).is_err());
    assert!(api.data.borrow().calls.iter().all(|(m, _, _)| m == "GET"));
    let api = ready();
    seed(&api, "test", 1.0);
    publish(&api, "test");
    api.data.borrow_mut().cards[0]["columnId"] = json!("unknown");
    assert!(export_mock(&api).unwrap_err().contains("outside"));
}
#[test]
fn internal_listing_resolves_work_lanes_and_shows_missing_links() {
    let api = ready();
    seed(&api, "test", 1.0);
    run_mock(
        &api,
        Command::Link {
            card: "test".into(),
            work: vec!["123".into(), "999".into()],
            unlink: false,
        },
    )
    .unwrap();
    let list = run_mock(
        &api,
        Command::List {
            lane: None,
            public_only: false,
            json: true,
        },
    )
    .unwrap()
    .unwrap();
    let v: Value = serde_json::from_str(&list).unwrap();
    assert_eq!(v[0]["work"][0]["placements"][0]["lane"], "Nå");
    assert_eq!(v[0]["work"][1]["placements"], json!([]));
    assert_eq!(v[0]["public"], false);
}
#[test]
fn managed_block_errors_preserve_human_text_and_date_math_handles_leap_days() {
    let bad = "human text\n<!-- ROADMAP WORK -->broken";
    assert!(write_links(bad, &BTreeSet::from([1])).is_err());
    assert_eq!(date_from_days(0), "1970-01-01");
    assert_eq!(date_from_days(19782), "2024-02-29");
    assert!(validate_date("2024-02-29").is_ok());
    assert!(validate_date("2025-02-29").is_err());
    assert!(references(&["##1".into()]).is_err());
}
#[test]
fn custom_configuration_controls_structure_lanes_and_unicode_limits() {
    let api = Mock::default();
    let config = Roadmap {
        board: "Features".into(),
        lanes: vec!["Under arbeid".into(), "Ferdig".into()],
        lane_keys: vec!["building".into(), "shipped".into()],
        public_tag: "approved".into(),
        max_public_chars: 5,
    };
    run(&api, "collection", "Example", &config, Command::Init).unwrap();
    seed(&api, "test", 1.0);
    run(
        &api,
        "collection",
        "Example",
        &config,
        Command::SetPublic {
            card: "test".into(),
            title: "Test".into(),
            text: "æøåÆØ".into(),
        },
    )
    .unwrap();
    let out = run(
        &api,
        "collection",
        "Example",
        &config,
        Command::Export {
            public: true,
            format: "json".into(),
            out: None,
        },
    )
    .unwrap()
    .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["lanes"].as_array().unwrap().len(), 2);
    assert_eq!(v["lanes"][0]["key"], "building");
    assert_eq!(v["lanes"][0]["items"][0]["text"], "æøåÆØ");
    assert!(run(
        &api,
        "collection",
        "Example",
        &config,
        Command::SetPublic {
            card: "test".into(),
            title: "Test".into(),
            text: "æøåÆØÅ".into()
        }
    )
    .is_err());
}
#[test]
fn shared_ensure_board_keeps_standard_unscheduled_lane_and_refuses_duplicates() {
    let api = Mock::default();
    let lanes = vec!["Ready".into()];
    boards::resolve(&api, "collection", "Example", "Work", &lanes, true, true).unwrap();
    assert_eq!(api.data.borrow().columns.len(), 2);
    api.data.borrow_mut().calls.clear();
    boards::resolve(&api, "collection", "Example", "Work", &lanes, true, true).unwrap();
    assert!(api.data.borrow().calls.iter().all(|(m, _, _)| m == "GET"));
    let mut duplicate = api.data.borrow().widgets[0].clone();
    duplicate["widgetCommonId"] = json!("other");
    api.data.borrow_mut().widgets.push(duplicate);
    assert!(boards::resolve(&api, "collection", "Example", "Work", &lanes, false, true).is_err());
}
#[test]
fn duplicate_public_ids_missing_positions_and_missing_tags_fail_closed() {
    let api = ready();
    seed(&api, "test", 1.0);
    publish(&api, "test");
    let mut copy = api.data.borrow().cards[0].clone();
    copy["cardId"] = json!("duplicate-instance");
    api.data.borrow_mut().cards.push(copy);
    assert!(export_mock(&api)
        .unwrap_err()
        .contains("Duplicate public ID"));
    api.data.borrow_mut().cards.pop();
    api.data.borrow_mut().cards[0]
        .as_object_mut()
        .unwrap()
        .remove("listPosition");
    assert!(export_mock(&api).unwrap_err().contains("position"));
    api.data.borrow_mut().cards[0]["position"] = json!(1);
    api.data.borrow_mut().tags.clear();
    assert!(export_mock(&api).unwrap_err().contains("tag is missing"));
}
