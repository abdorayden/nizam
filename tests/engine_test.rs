/// tests for the engine orchestration (json -> kahn -> process)

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use nizam::engine::Engine;

// helpers

fn tmp_dir(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "pyrust_engine_{}_{}",
        name,
        std::process::id()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn write_template(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path
}

fn json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn noop_error(_: String) {}
fn noop_warning(_: String) {}
fn noop_note(_: String) {}

#[cfg(target_os = "linux")]
fn mkdir_cmd(path: &Path) -> String {
    format!("mkdir -p {}", json_path(path))
}

#[cfg(target_os = "windows")]
fn mkdir_cmd(path: &Path) -> String {
    format!("cmd /C mkdir {}", json_path(path))
}

#[cfg(target_os = "linux")]
fn touch_cmd(path: &Path) -> String {
    format!("touch {}", json_path(path))
}

#[cfg(target_os = "windows")]
fn touch_cmd(path: &Path) -> String {
    format!(r#"cmd /C type nul > "{}""#, json_path(path))
}

#[cfg(target_os = "linux")]
fn fail_cmd() -> String {
    "false".to_string()
}

#[cfg(target_os = "windows")]
fn fail_cmd() -> String {
    "cmd /C exit 1".to_string()
}

// execution

#[test]
fn runs_dependency_levels_in_order() {
    let dir = tmp_dir("deps");
    let out = dir.join("out");
    let done = out.join("done");

    let body = format!(
        r#"{{
            "tasks": {{
                "create": {{ "cmd": "{}" }},
                "use": {{ "cmd": "{}", "deps": ["create"] }}
            }}
        }}"#,
        mkdir_cmd(&out),
        touch_cmd(&done),
    );
    let path = write_template(&dir, "template.json", &body);

    let report = Engine::new(path.to_str().unwrap())
        .execute(noop_error, noop_warning, noop_note)
        .expect("engine should run");

    assert_eq!(report.results.len(), 2);
    assert!(report.results.iter().all(|info| info.is_ok()));
    assert!(done.exists());
    assert!(!report.schedule.has_cycle);
    assert_eq!(report.schedule.levels.len(), 2);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn substitutes_bindings_before_running() {
    let dir = tmp_dir("bindings");
    let body = r#"{
        "bind": { "word": "hello" },
        "tasks": { "1": { "cmd": "echo ^word^" } }
    }"#;
    let path = write_template(&dir, "template.json", body);

    let report = Engine::new(path.to_str().unwrap())
        .execute(noop_error, noop_warning, noop_note)
        .expect("engine should run");

    assert_eq!(report.model.tasks["1"].cmd, "echo hello");
    assert_eq!(report.results.len(), 1);
    assert!(report.results[0].is_ok());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn empty_template_runs_nothing() {
    let dir = tmp_dir("empty");
    let path = write_template(&dir, "template.json", r#"{ "tasks": {} }"#);

    let report = Engine::new(path.to_str().unwrap())
        .execute(noop_error, noop_warning, noop_note)
        .expect("engine should run");

    assert!(report.results.is_empty());
    assert!(report.schedule.levels.is_empty());
    assert!(!report.schedule.has_cycle);

    fs::remove_dir_all(&dir).ok();
}

// failures

#[test]
fn records_failed_tasks() {
    let dir = tmp_dir("failure");
    let body = format!(
        r#"{{ "tasks": {{ "bad": {{ "cmd": "{}" }} }} }}"#,
        fail_cmd()
    );
    let path = write_template(&dir, "template.json", &body);

    let report = Engine::new(path.to_str().unwrap())
        .execute(noop_error, noop_warning, noop_note)
        .expect("engine should run");

    assert_eq!(report.results.len(), 1);
    assert!(!report.results[0].is_ok());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn exit_on_first_error_stops_later_levels() {
    let dir = tmp_dir("exit_on_error");
    let done = dir.join("should_not_exist");

    let body = format!(
        r#"{{
            "exit_on_first_error": true,
            "tasks": {{
                "bad": {{ "cmd": "{}" }},
                "after": {{ "cmd": "{}", "deps": ["bad"] }}
            }}
        }}"#,
        fail_cmd(),
        touch_cmd(&done),
    );
    let path = write_template(&dir, "template.json", &body);

    let report = Engine::new(path.to_str().unwrap())
        .execute(noop_error, noop_warning, noop_note)
        .expect("engine should run");

    assert_eq!(report.results.len(), 1);
    assert!(!report.results[0].is_ok());
    assert!(!done.exists());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn continues_after_failure_when_not_exiting() {
    let dir = tmp_dir("continue_on_error");
    let done = dir.join("done");

    let body = format!(
        r#"{{
            "exit_on_first_error": false,
            "tasks": {{
                "bad": {{ "cmd": "{}" }},
                "after": {{ "cmd": "{}", "deps": ["bad"] }}
            }}
        }}"#,
        fail_cmd(),
        touch_cmd(&done),
    );
    let path = write_template(&dir, "template.json", &body);

    let report = Engine::new(path.to_str().unwrap())
        .execute(noop_error, noop_warning, noop_note)
        .expect("engine should run");

    assert_eq!(report.results.len(), 2);
    assert!(!report.results[0].is_ok());
    assert!(report.results[1].is_ok());
    assert!(done.exists());

    fs::remove_dir_all(&dir).ok();
}

// validation errors

static CYCLE_ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn cycle_error(msg: String) {
    CYCLE_ERRORS.lock().unwrap().push(msg);
}

#[test]
fn detects_cycle_before_running() {
    CYCLE_ERRORS.lock().unwrap().clear();

    let dir = tmp_dir("cycle");
    let body = r#"{
        "tasks": {
            "a": { "cmd": "echo a", "deps": ["b"] },
            "b": { "cmd": "echo b", "deps": ["a"] }
        }
    }"#;
    let path = write_template(&dir, "template.json", body);

    let report = Engine::new(path.to_str().unwrap())
        .execute(cycle_error, noop_warning, noop_note);

    assert!(report.is_none());

    let errors = CYCLE_ERRORS.lock().unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("cycle"));
    assert!(errors[0].contains("a"));
    assert!(errors[0].contains("b"));

    drop(errors);
    fs::remove_dir_all(&dir).ok();
}

static MISSING_ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn missing_error(msg: String) {
    MISSING_ERRORS.lock().unwrap().push(msg);
}

#[test]
fn reports_unknown_dependency() {
    MISSING_ERRORS.lock().unwrap().clear();

    let dir = tmp_dir("missing_dep");
    let body = r#"{
        "tasks": {
            "a": { "cmd": "echo a", "deps": ["does_not_exist"] }
        }
    }"#;
    let path = write_template(&dir, "template.json", body);

    let report = Engine::new(path.to_str().unwrap())
        .execute(missing_error, noop_warning, noop_note);

    assert!(report.is_none());

    let errors = MISSING_ERRORS.lock().unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("does_not_exist"));
    assert!(errors[0].contains("a"));

    drop(errors);
    fs::remove_dir_all(&dir).ok();
}

// callbacks / Spec::run

static INVALID_ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static INVALID_QUITS: AtomicUsize = AtomicUsize::new(0);

fn invalid_error(msg: String) {
    INVALID_ERRORS.lock().unwrap().push(msg);
}

fn invalid_quit() {
    INVALID_QUITS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn invalid_template_reports_and_quits() {
    INVALID_ERRORS.lock().unwrap().clear();
    INVALID_QUITS.store(0, Ordering::SeqCst);

    let dir = tmp_dir("invalid");
    let path = dir.join("does_not_exist.json");
    let engine = Engine::new(path.to_str().unwrap());

    engine.run(invalid_error, noop_warning, noop_note, invalid_quit);

    assert_eq!(INVALID_QUITS.load(Ordering::SeqCst), 1);
    assert_eq!(INVALID_ERRORS.lock().unwrap().len(), 1);

    fs::remove_dir_all(&dir).ok();
}

static SUCCESS_QUITS: AtomicUsize = AtomicUsize::new(0);

fn success_quit() {
    SUCCESS_QUITS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn successful_run_does_not_quit() {
    SUCCESS_QUITS.store(0, Ordering::SeqCst);

    let dir = tmp_dir("success_quit");
    let path = write_template(&dir, "template.json", r#"{ "tasks": { "1": { "cmd": "echo ok" } } }"#);
    let engine = Engine::new(path.to_str().unwrap());

    engine.run(noop_error, noop_warning, noop_note, success_quit);

    assert_eq!(SUCCESS_QUITS.load(Ordering::SeqCst), 0);

    fs::remove_dir_all(&dir).ok();
}

static LIMIT_WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn limit_warning(msg: String) {
    LIMIT_WARNINGS.lock().unwrap().push(msg);
}

#[test]
fn invalid_task_limit_warns_and_still_runs() {
    LIMIT_WARNINGS.lock().unwrap().clear();

    let dir = tmp_dir("limit");
    let path = write_template(
        &dir,
        "template.json",
        r#"{
            "limite_tasks": 0,
            "tasks": { "1": { "cmd": "echo ok" } }
        }"#,
    );

    let report = Engine::new(path.to_str().unwrap())
        .execute(noop_error, limit_warning, noop_note)
        .expect("engine should still run");

    assert_eq!(report.results.len(), 1);
    assert!(report.results[0].is_ok());

    let warnings = LIMIT_WARNINGS.lock().unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("limite_tasks"));

    drop(warnings);
    fs::remove_dir_all(&dir).ok();
}
