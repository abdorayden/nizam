
/// tests for load and unload functions from `json.rs` moduel

use nizam::json::{load, unload, Cmd, EngineTemplate, Model, TaskOptions};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// helpers

fn example_model() -> Model {
    let mut tasks = HashMap::new();
    tasks.insert(
        "1".to_string(),
        TaskOptions { cmd: Cmd::SimplCmd("sleep 5".to_string()), deps: vec![] },
    );
    tasks.insert(
        "2".to_string(),
        TaskOptions { cmd: Cmd::SimplCmd("sleep 10".to_string()), deps: vec!["1".to_string()] },
    );
    Model { limite_tasks: Some(2), tasks, ..Default::default() }
}

fn tmp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("pyrust_json_{}_{}.json", name, std::process::id()));
    p
}

fn tmp_dir(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("pyrust_json_{}_{}", name, std::process::id()));
    fs::create_dir_all(&p).unwrap();
    p
}

// tests

// `unload`
#[test]
fn unload_writes_json_file() {
    let path = tmp_path("unload");
    unload(path.to_str().unwrap(), &example_model()).unwrap();
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("sleep 5"));
    assert!(contents.contains("limite_tasks"));
    fs::remove_file(&path).ok();
}

// `load`
#[test]
fn load_parses_json_file() {
    let path = tmp_path("load");
    fs::write(&path, r#"{ "limite_tasks": 2, "tasks": { "1": { "cmd": "sleep 5" }, "2": { "cmd": "sleep 10", "deps": ["1"] } } } // comment test"#).unwrap();
    let model = load(path.to_str().unwrap()).unwrap();
    assert_eq!(model.limite_tasks, Some(2));
    assert_eq!(model.tasks.len(), 2);
    assert_eq!(model.tasks["1"].cmd.as_str(), Some("sleep 5"));
    assert!(model.tasks["1"].deps.is_empty());
    assert_eq!(model.tasks["2"].deps, vec!["1".to_string()]);
    fs::remove_file(&path).ok();
}

// tests both together
#[test]
fn round_trip_load_after_unload() {
    let path = tmp_path("roundtrip");
    let model = example_model();
    unload(path.to_str().unwrap(), &model).unwrap();
    let loaded = load(path.to_str().unwrap()).unwrap();
    assert_eq!(loaded, model); // works now that PartialEq is derived
    fs::remove_file(&path).ok();
}

// load error handling
#[test]
fn load_missing_file_errors() {
    assert!(load("/definitely/not/here.json").is_err());
}

#[test]
fn load_invalid_json_errors() {
    let path = tmp_path("invalid");
    fs::write(&path, "{ not valid json").unwrap();
    assert!(load(path.to_str().unwrap()).is_err());
    fs::remove_file(&path).ok();
}

// `EngineTemplate::parse`

#[test]
fn parse_substitutes_bindings() {
    let path = tmp_path("parse");
    fs::write(
        &path,
        r#"{
            "limite_tasks": 2,
            "exit_on_first_error": true,
            "bind": { "duration": "5", "name": "procm" },
            "tasks": {
                "1": { "cmd": "sleep ^duration^" },
                "2": { "cmd": "echo ^name^ after ^duration^s", "deps": ["1"] }
            }
        }"#,
    )
    .unwrap();

    let model = EngineTemplate::new(path.to_str().unwrap()).parse();

    assert_eq!(model.limite_tasks, Some(2));
    assert_eq!(model.exit_on_first_error, Some(true));
    assert_eq!(model.tasks["1"].cmd.as_str(), Some("sleep 5"));
    assert_eq!(model.tasks["2"].cmd.as_str(), Some("echo procm after 5s"));
    assert_eq!(model.tasks["2"].deps, vec!["1".to_string()]);
    assert_eq!(model.bind.unwrap().get("name").map(String::as_str), Some("procm"));
    fs::remove_file(&path).ok();
}

#[test]
fn parse_keeps_unknown_bindings_untouched() {
    let path = tmp_path("parse_unknown");
    fs::write(
        &path,
        r#"{ "bind": { "a": "1" }, "tasks": { "1": { "cmd": "run ^a^ ^b^ ^" } } }"#,
    )
    .unwrap();

    let model = EngineTemplate::new(path.to_str().unwrap()).parse();

    assert_eq!(model.tasks["1"].cmd.as_str(), Some("run 1 ^b^ ^"));
    fs::remove_file(&path).ok();
}

#[test]
fn parse_without_bind_keeps_cmd_untouched() {
    let path = tmp_path("parse_nobind");
    fs::write(&path, r#"{ "tasks": { "1": { "cmd": "sleep ^duration^" } } }"#).unwrap();

    let model = EngineTemplate::new(path.to_str().unwrap()).parse();

    assert_eq!(model.tasks["1"].cmd.as_str(), Some("sleep ^duration^"));
    assert!(model.bind.is_none());
    fs::remove_file(&path).ok();
}

#[test]
fn load_preserves_conditional_cmd() {
    let path = tmp_path("load_cond");
    fs::write(
        &path,
        r#"{
            "tasks": {
                "1": {
                    "cmd": [
                        { "when": { "os": "linux" }, "run": "echo linux" },
                        { "run": "echo fallback" }
                    ]
                }
            }
        }"#,
    )
    .unwrap();

    let model = load(path.to_str().unwrap()).unwrap();
    match &model.tasks["1"].cmd {
        Cmd::Cond(conditions) => {
            assert_eq!(conditions.len(), 2);
            assert_eq!(conditions[0].when.as_ref().unwrap().os, "linux");
            assert_eq!(conditions[1].when, None);
        }
        Cmd::SimplCmd(_) => panic!("expected a conditional command"),
    }
    fs::remove_file(&path).ok();
}

#[test]
fn parse_selects_matching_os_condition() {
    let path = tmp_path("parse_cond_match");
    let os = std::env::consts::OS;
    let body = format!(
        r#"{{
            "tasks": {{
                "1": {{
                    "cmd": [
                        {{ "when": {{ "os": "definitely-not-{os}" }}, "run": "echo wrong" }},
                        {{ "when": {{ "os": "{os}" }}, "run": "echo right" }},
                        {{ "run": "echo fallback" }}
                    ]
                }}
            }}
        }}"#
    );
    fs::write(&path, body).unwrap();

    let model = EngineTemplate::new(path.to_str().unwrap()).parse();

    assert_eq!(model.tasks["1"].cmd.as_str(), Some("echo right"));
    fs::remove_file(&path).ok();
}

#[test]
fn parse_uses_fallback_when_no_condition_matches() {
    let path = tmp_path("parse_cond_fallback");
    fs::write(
        &path,
        r#"{
            "tasks": {
                "1": {
                    "cmd": [
                        { "when": { "os": "definitely-not-a-real-os" }, "run": "echo wrong" },
                        { "run": "echo fallback" }
                    ]
                }
            }
        }"#,
    )
    .unwrap();

    let model = EngineTemplate::new(path.to_str().unwrap()).parse();

    assert_eq!(model.tasks["1"].cmd.as_str(), Some("echo fallback"));
    fs::remove_file(&path).ok();
}

#[test]
fn parse_resolves_conditions_before_bindings() {
    let path = tmp_path("parse_cond_bind");
    let os = std::env::consts::OS;
    let body = format!(
        r#"{{
            "bind": {{ "tool": "gcc" }},
            "tasks": {{
                "1": {{
                    "cmd": [
                        {{ "run": "echo not this" }},
                        {{ "when": {{ "os": "{os}" }}, "run": "^tool^ -c foo.c" }}
                    ]
                }}
            }}
        }}"#
    );
    fs::write(&path, body).unwrap();

    let model = EngineTemplate::new(path.to_str().unwrap()).parse();

    assert_eq!(model.tasks["1"].cmd.as_str(), Some("gcc -c foo.c"));
    fs::remove_file(&path).ok();
}

#[test]
fn parse_condition_without_match_or_fallback_is_empty() {
    let path = tmp_path("parse_cond_empty");
    fs::write(
        &path,
        r#"{ "tasks": { "1": { "cmd": [ { "when": { "os": "definitely-not-a-real-os" }, "run": "echo wrong" } ] } } }"#,
    )
    .unwrap();

    let model = EngineTemplate::new(path.to_str().unwrap()).parse();

    assert_eq!(model.tasks["1"].cmd.as_str(), Some(""));
    fs::remove_file(&path).ok();
}

// `uses`

#[test]
fn load_inherits_used_tasks_and_bindings() {
    let dir = tmp_dir("uses_basic");
    fs::write(
        dir.join("base.json"),
        r#"{
            "bind": { "tool": "echo" },
            "tasks": { "base": { "cmd": "^tool^ base" } }
        }"#,
    )
    .unwrap();
    fs::write(
        dir.join("main.json"),
        r#"{
            "uses": ["base.json"],
            "bind": { "name": "main" },
            "tasks": { "main": { "cmd": "^tool^ ^name^", "deps": ["base"] } }
        }"#,
    )
    .unwrap();

    let model = load(dir.join("main.json").to_str().unwrap()).unwrap();

    assert!(model.uses.is_none());
    assert_eq!(model.tasks.len(), 2);
    assert_eq!(model.tasks["base"].cmd.as_str(), Some("^tool^ base"));
    assert_eq!(model.tasks["main"].cmd.as_str(), Some("^tool^ ^name^"));

    let bind = model.bind.unwrap();
    assert_eq!(bind.get("tool").map(String::as_str), Some("echo"));
    assert_eq!(bind.get("name").map(String::as_str), Some("main"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn local_tasks_and_bindings_override_used_ones() {
    let dir = tmp_dir("uses_override");
    fs::write(
        dir.join("base.json"),
        r#"{
            "bind": { "tool": "old" },
            "tasks": { "same": { "cmd": "old task" } }
        }"#,
    )
    .unwrap();
    fs::write(
        dir.join("main.json"),
        r#"{
            "uses": ["base.json"],
            "bind": { "tool": "new" },
            "tasks": { "same": { "cmd": "^tool^ task" } }
        }"#,
    )
    .unwrap();

    let model = load(dir.join("main.json").to_str().unwrap()).unwrap();

    assert_eq!(model.tasks.len(), 1);
    assert_eq!(model.tasks["same"].cmd.as_str(), Some("^tool^ task"));
    assert_eq!(
        model.bind.unwrap().get("tool").map(String::as_str),
        Some("new")
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn uses_are_resolved_transitively() {
    let dir = tmp_dir("uses_transitive");
    fs::write(
        dir.join("c.json"),
        r#"{ "bind": { "c": "3" }, "tasks": { "c": { "cmd": "echo c" } } }"#,
    )
    .unwrap();
    fs::write(
        dir.join("b.json"),
        r#"{ "uses": ["c.json"], "tasks": { "b": { "cmd": "echo b", "deps": ["c"] } } }"#,
    )
    .unwrap();
    fs::write(
        dir.join("a.json"),
        r#"{ "uses": ["b.json"], "tasks": { "a": { "cmd": "echo a", "deps": ["b"] } } }"#,
    )
    .unwrap();

    let model = load(dir.join("a.json").to_str().unwrap()).unwrap();

    assert_eq!(model.tasks.len(), 3);
    assert!(model.tasks.contains_key("a"));
    assert!(model.tasks.contains_key("b"));
    assert!(model.tasks.contains_key("c"));
    assert_eq!(model.bind.unwrap().get("c").map(String::as_str), Some("3"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn uses_paths_are_relative_to_the_template() {
    let dir = tmp_dir("uses_relative");
    fs::create_dir_all(dir.join("libs")).unwrap();
    fs::write(
        dir.join("libs").join("base.json"),
        r#"{ "tasks": { "base": { "cmd": "echo base" } } }"#,
    )
    .unwrap();
    fs::write(
        dir.join("main.json"),
        r#"{ "uses": ["libs/base.json"], "tasks": { "main": { "cmd": "echo main" } } }"#,
    )
    .unwrap();

    let model = load(dir.join("main.json").to_str().unwrap()).unwrap();

    assert_eq!(model.tasks.len(), 2);
    assert!(model.tasks.contains_key("base"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_used_template_errors() {
    let dir = tmp_dir("uses_missing");
    fs::write(
        dir.join("main.json"),
        r#"{ "uses": ["does-not-exist.json"], "tasks": {} }"#,
    )
    .unwrap();

    assert!(load(dir.join("main.json").to_str().unwrap()).is_err());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn cyclic_uses_errors() {
    let dir = tmp_dir("uses_cycle");
    fs::write(
        dir.join("a.json"),
        r#"{ "uses": ["b.json"], "tasks": {} }"#,
    )
    .unwrap();
    fs::write(
        dir.join("b.json"),
        r#"{ "uses": ["a.json"], "tasks": {} }"#,
    )
    .unwrap();

    let err = load(dir.join("a.json").to_str().unwrap()).unwrap_err();

    assert!(
        err.to_string().contains("cyclic `uses`"),
        "unexpected error: {err}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn parse_substitutes_bindings_on_inherited_tasks() {
    let dir = tmp_dir("uses_parse_bind");
    fs::write(
        dir.join("base.json"),
        r#"{ "tasks": { "base": { "cmd": "^tool^ base.c" } } }"#,
    )
    .unwrap();
    fs::write(
        dir.join("main.json"),
        r#"{ "uses": ["base.json"], "bind": { "tool": "gcc" }, "tasks": {} }"#,
    )
    .unwrap();

    let model = EngineTemplate::new(dir.join("main.json").to_str().unwrap()).parse();

    assert_eq!(model.tasks["base"].cmd.as_str(), Some("gcc base.c"));

    fs::remove_dir_all(&dir).ok();
}
