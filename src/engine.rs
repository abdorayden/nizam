/// this file has engine functionalities
/// plugs with:
///  json.rs
///  kahn.rs
///  logs.rs
///  process.rs
///  task_manager.rs
///

use super::json::{EngineTemplate, Model};
use super::kahn::{self, KahnResult};
use super::process::{self, ProcInfo};
use super::task_manager::find_missing_dep;
use std::collections::{BTreeSet, HashMap};
use std::sync::mpsc;
use std::thread;

pub type LogingError = fn (msg: String);
pub type LogingWarning = fn (msg: String);
pub type LogingNote = fn (msg: String);
pub type Quit = fn ();

pub struct Engine {
    pub template: EngineTemplate,
}

/// everything a successful engine run produced
#[derive(Debug)]
pub struct Execution {
    /// the parsed template (bindings already substituted)
    /// saved it it might be used for reports
    pub model: Model,
    /// the schedule produced by kahn's algorithm
    /// kahn result too
    pub schedule: KahnResult,
    /// process results in the order the tasks were executed
    pub results: Vec<ProcInfo>,
}

fn report_task_result(info: &ProcInfo, error: &LogingError) -> bool {
    let failed = info.never_run || !info.is_ok();
    if failed {
        let detail = info.err.clone().unwrap_or_default();
        if detail.is_empty() {
            error(format!("task failed: {}", info.cmd));
        } else {
            error(format!("task failed: {}: {}", info.cmd, detail));
        }
    }
    failed
}

fn release_dependents(
    task_id: &str,
    dependents: &HashMap<String, Vec<String>>,
    remaining_deps: &mut HashMap<String, usize>,
    ready: &mut BTreeSet<String>,
) {
    if let Some(children) = dependents.get(task_id) {
        for child in children {
            if let Some(count) = remaining_deps.get_mut(child) {
                *count -= 1;
                if *count == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
}

impl Engine {
    pub fn new(template: &str) -> Self {
        Self {
            template : EngineTemplate::new(template),
        }
    }

    /// validates the template and computes the schedule without running anything.
    /// reports through `error` and returns `None` when the template is not runnable.
    fn prepare(&self, error: LogingError) -> Option<(Model, KahnResult)> {
        if let Some(err) = self.template.get_error() {
            error(err);
            return None;
        }

        let model = self.template.parse();

        if let Some((task, dep)) = find_missing_dep(&model.tasks) {
            error(format!("task `{task}` depends on unknown task `{dep}`"));
            return None;
        }

        let schedule = kahn::sort(&model.tasks);

        if schedule.has_cycle {
            let mut blocked: Vec<String> = model
                .tasks
                .keys()
                .filter(|id| !schedule.order.contains(id))
                .cloned()
                .collect();
            blocked.sort();
            error(format!(
                "dependency cycle detected between tasks: {}",
                blocked.join(", ")
            ));
            return None;
        }

        Some((model, schedule))
    }

    /// runs the template and returns the execution report.
    ///
    /// logging goes through the callbacks; `None` means the template could not
    /// be executed (load error, unknown dependency or cycle).
    pub fn execute(
        &self,
        error: LogingError,
        warning: LogingWarning,
        note: LogingNote,
    ) -> Option<Execution> {
        let (model, schedule) = self.prepare(error)?;

        let exit_on_first_error = model.exit_on_first_error.unwrap_or(false);

        let limit = match model.limite_tasks {
            Some(n) if n > 0 => Some(n as usize),
            Some(_) => {
                warning("limite_tasks must be a positive number, ignoring it".to_string());
                None
            }
            None => None,
        };

        let max_running = limit.unwrap_or_else(|| model.tasks.len().max(1));
        let mut remaining_deps: HashMap<String, usize> = model
            .tasks
            .iter()
            .map(|(id, task)| (id.clone(), task.deps.len()))
            .collect();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
        for (id, task) in &model.tasks {
            for dependency in &task.deps {
                dependents
                    .entry(dependency.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
        for children in dependents.values_mut() {
            children.sort();
        }

        // A sorted ready set keeps dispatch deterministic while allowing tasks
        // to start as soon as their own dependencies finish.
        let mut ready: BTreeSet<String> = remaining_deps
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
            .collect();
        let (completed_tx, completed_rx) = mpsc::channel::<(String, ProcInfo)>();
        let mut completed_results: HashMap<String, ProcInfo> = HashMap::new();
        let mut running = 0usize;
        let mut stop_scheduling = false;

        note(format!(
            "running {} task(s) with a concurrency limit of {}",
            model.tasks.len(), max_running
        ));

        while !ready.is_empty() || running > 0 {
            while !stop_scheduling && running < max_running {
                let Some(task_id) = ready.pop_first() else {
                    break;
                };
                let cmd = model.tasks[&task_id]
                    .cmd
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                match process::run_cmd_async(cmd.clone()) {
                    Ok(child) => {
                        running += 1;
                        let sender = completed_tx.clone();
                        thread::spawn(move || {
                            let info = process::wait_child(cmd, child);
                            let _ = sender.send((task_id, info));
                        });
                    }
                    Err(spawn_error) => {
                        let info = process::spawn_failure(cmd, spawn_error);
                        let failed = report_task_result(&info, &error);
                        completed_results.insert(task_id.clone(), info);

                        if failed && exit_on_first_error {
                            stop_scheduling = true;
                            note("exit_on_first_error is set, stopping new tasks".to_string());
                        } else {
                            release_dependents(&task_id, &dependents, &mut remaining_deps, &mut ready);
                        }
                    }
                }
            }

            if running == 0 {
                break;
            }

            let (task_id, info) = completed_rx
                .recv()
                .expect("task waiter thread should report its result");
            running -= 1;
            let failed = report_task_result(&info, &error);
            completed_results.insert(task_id.clone(), info);

            if failed && exit_on_first_error {
                stop_scheduling = true;
                note("exit_on_first_error is set, stopping new tasks".to_string());
            } else {
                release_dependents(&task_id, &dependents, &mut remaining_deps, &mut ready);
            }
        }

        // Preserve stable result ordering for callers even though tasks now
        // finish in completion order.
        let results = schedule
            .order
            .iter()
            .filter_map(|task_id| completed_results.remove(task_id))
            .collect();

        Some(Execution {
            model,
            schedule,
            results,
        })
    }

    pub fn run (&self, 
        error: LogingError,
        warning: LogingWarning,
        note: LogingNote,
        quit: Quit
        ) {
        match self.execute(error, warning, note) {
            Some(exec) => {
                if exec.schedule.has_cycle {
                    quit()
                }
            }
            None => {
                quit()
            }
        }
    }
}
