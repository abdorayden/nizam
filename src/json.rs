// managing the (de)serialization of the json template file.
//
// this module export two functions for loading and unloading json file
// and types
//
// template example
//
// ```json
// {
//   "limite_tasks": 2,
//   "exit_on_first_error": true,
//   "bind" : {
//      "duration" : "5"
//   },
//   "uses": [
//      "bindings.json"
//   ],
//   "tasks": {
//     "1": {
//          "cmd": "sleep ^duration^"
//      },
//     "2": {
//          "cmd": "sleep 10", "deps": ["1"]
//      },
//      "3": {
//          "cmd": [
//              {
//                  "when": {
//                      "os": "linux"
//                  },
//                  "run": "echo hey"
//              },
//              {
//                  "run": "echo otherwise"
//              }
//          ],
//          "deps": ["2"]
//      }
//   }
// }
// ```

// Copyright (c) 2026 Computer Science Security Team . All Rights Reserved.

use serde::Deserialize;
use serde::Serialize;

// standerd rust libs
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Result;
use std::path::{Path, PathBuf};

/// built-in variables a conditional `when` can check.
/// `os` is matched against the current platform (`std::env::consts::OS`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BuiltInVarsCondition {
    pub os: String,
    /// reserved for future conditions; currently not evaluated
    #[serde(default)]
    pub condition: Option<String>,
}

impl BuiltInVarsCondition {
    fn matches(&self) -> bool {
        self.os.eq_ignore_ascii_case(std::env::consts::OS)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Conditional {
    /// when omitted this entry is the fallback command
    pub when: Option<BuiltInVarsCondition>,
    pub run: String, // command that runs when the condition is true
}

/// a task command: either a plain string or a list of conditional commands
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Cmd {
    Cond(Vec<Conditional>),
    SimplCmd(String),
}

impl Cmd {
    /// returns the plain command string once the template has been parsed
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Cmd::SimplCmd(cmd) => Some(cmd),
            Cmd::Cond(_) => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaskOptions {
    pub cmd: Cmd,
    #[serde(default)]
    pub deps: Vec<String>,
}

pub type Tasks = HashMap<String, TaskOptions>;
pub type Bindings = HashMap<String, String>;
pub type Uses = Vec<String>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Model {
    #[serde(default)]
    pub limite_tasks: Option<i32>,
    #[serde(default)]
    pub exit_on_first_error: Option<bool>, // this field makes the engine exit on first error
    pub bind: Option<Bindings>,
    #[serde(default)]
    pub uses: Option<Uses>,
    pub tasks: Tasks,
}

fn remove_comments(content: &mut Vec<u8>) {
    let cleaned: Vec<u8> = content
        .split(|&b| b == b'\n')
        .map(|line| {
            let end = line
                .windows(2)
                .position(|w| w == b"//")
                .unwrap_or(line.len());
            &line[..end]
        })
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .flat_map(|line| line.iter().copied().chain(std::iter::once(b'\n')))
        .collect();

    *content = cleaned;
}

fn read_model(file_path: &Path) -> Result<Model> {
    let file = File::open(file_path)?;
    let mut content = Vec::new();
    BufReader::new(file).read_to_end(&mut content)?;
    remove_comments(&mut content);
    let content = std::str::from_utf8(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    serde_json::from_str(content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e).into())
}

/// loads a template and recursively merges the tasks and bindings of every
/// template listed in `uses`. local tasks/bindings override inherited ones.
pub fn load(file_path: &str) -> Result<Model> {
    let mut stack: Vec<PathBuf> = Vec::new();
    load_with_uses(Path::new(file_path), &mut stack)
}

fn load_with_uses(file_path: &Path, stack: &mut Vec<PathBuf>) -> Result<Model> {
    let canonical = std::fs::canonicalize(file_path)?;

    if let Some(start) = stack.iter().position(|path| path == &canonical) {
        let chain: Vec<String> = stack[start..]
            .iter()
            .chain(std::iter::once(&canonical))
            .map(|path| path.display().to_string())
            .collect();
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("cyclic `uses` between templates: {}", chain.join(" -> ")),
        ));
    }

    let mut model = read_model(file_path)?;
    let uses = model.uses.take().unwrap_or_default();

    stack.push(canonical);
    let result = inherit_uses(&mut model, file_path, &uses, stack);
    stack.pop();

    result?;
    Ok(model)
}

/// merges the tasks and bindings of `uses` into `model`. bindings and tasks
/// defined locally win over inherited ones; later entries in `uses` win over
/// earlier ones.
fn inherit_uses(
    model: &mut Model,
    file_path: &Path,
    uses: &[String],
    stack: &mut Vec<PathBuf>,
) -> Result<()> {
    let base = file_path.parent().unwrap_or_else(|| Path::new("."));
    let mut inherited_tasks = Tasks::new();
    let mut inherited_bind = Bindings::new();

    for used in uses {
        let used_model = load_with_uses(&base.join(used), stack)?;
        inherited_tasks.extend(used_model.tasks);
        if let Some(bind) = used_model.bind {
            inherited_bind.extend(bind);
        }
    }

    inherited_tasks.extend(std::mem::take(&mut model.tasks));
    model.tasks = inherited_tasks;

    if let Some(bind) = model.bind.take() {
        inherited_bind.extend(bind);
    }
    model.bind = if inherited_bind.is_empty() {
        None
    } else {
        Some(inherited_bind)
    };

    Ok(())
}

/// for serialize
pub fn unload(file_path: &str, model: &Model) -> Result<()> {
    let stream = File::create(file_path)?;
    let writer = BufWriter::new(stream);

    serde_json::to_writer_pretty(writer, model)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// EngineTemplate: the real template that used by the engine after parsing bindings and expressions
/// in json file
pub struct EngineTemplate {
    pub model: Result<Model>,
}

/// replaces every `^var^` occurrence in `cmd` with the value of `var`
/// in `bind`. occurrences whose variable is not defined in `bind` are
/// left untouched.
fn substitute_bindings(cmd: &str, bind: &Bindings) -> String {
    let mut out = String::with_capacity(cmd.len());
    let mut rest = cmd;

    while let Some(start) = rest.find('^') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];

        match after.find('^') {
            Some(end) => {
                let var = &after[..end];
                match bind.get(var) {
                    Some(value) => out.push_str(value),
                    None => {
                        out.push('^');
                        out.push_str(var);
                        out.push('^');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                return out;
            }
        }
    }

    out.push_str(rest);
    out
}

/// picks the first conditional whose `when` matches the current platform.
/// falls back to the entry without a `when`. bindings are substituted after
/// the command has been selected.
fn resolve_conditions(conditions: Vec<Conditional>, bind: &Bindings) -> String {
    let mut fallback: Option<String> = None;

    for conditional in conditions {
        match conditional.when {
            Some(when) => {
                if when.matches() {
                    return substitute_bindings(&conditional.run, bind);
                }
            }
            None => {
                if fallback.is_none() {
                    fallback = Some(conditional.run);
                }
            }
        }
    }

    fallback
        .map(|run| substitute_bindings(&run, bind))
        .unwrap_or_default()
}

impl EngineTemplate {
    pub fn new(file_path: &str) -> Self {
        let model = load(file_path);
        Self { model: model }
    }

    pub fn get_error(&self) -> Option<String> {
        match self.model.as_ref() {
            Err(e) => Some(e.to_string()),
            Ok(_) => None,
        }
    }

    pub fn parse(&self) -> Model {
        let og_model = self
            .model
            .as_ref()
            .expect("EngineTemplate must be valid")
            .clone();

        let mut bind = og_model.bind.clone().unwrap_or_default();

        bind.insert(
            // add builtin bindings
            "HOME".to_string(),
            if let Some(path) = env::home_dir() {
                path.to_str().unwrap_or(&String::new()).to_string()
            } else {
                String::new()
            },
        );

        bind.insert(
            // add builtin bindings
            "PWD".to_string(),
            env::current_dir()
                .unwrap_or((&String::new()).into())
                .to_str()
                .unwrap_or(&String::new())
                .to_string(),
        );

        let mut model = Model {
            limite_tasks: og_model.limite_tasks,
            exit_on_first_error: og_model.exit_on_first_error,
            bind: og_model.bind,
            uses: None,
            tasks: Tasks::new(),
        };

        for (task_id, option) in og_model.tasks {
            let cmd = match option.cmd {
                Cmd::SimplCmd(command) => Cmd::SimplCmd(substitute_bindings(&command, &bind)),
                Cmd::Cond(conditions) => Cmd::SimplCmd(resolve_conditions(conditions, &bind)),
            };

            model.tasks.insert(
                task_id,
                TaskOptions {
                    cmd,
                    deps: option.deps,
                },
            );
        }
        model
    }
}
