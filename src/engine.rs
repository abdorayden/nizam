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
use super::process::{self, CommandMultiChainedList, ProcInfo};
use super::task_manager::find_missing_dep;

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

        let commands = kahn::commands(&schedule, &model.tasks);
        let mut results: Vec<ProcInfo> = Vec::new();
        let mut stop = false;

        for (level_idx, level) in commands.iter().enumerate() {
            if stop {
                break;
            }

            note(format!(
                "level {}: running {} task(s)",
                level_idx + 1,
                level.len()
            ));

            let batch_size = limit.unwrap_or(level.len().max(1));

            for batch in level.chunks(batch_size) {
                if stop {
                    break;
                }

                let multi: CommandMultiChainedList =
                    batch.iter().map(|cmd| vec![cmd.clone()]).collect();
                let infos = process::run_multi_chained(multi, true);

                let mut failed = false;

                for chain in infos {
                    for info in chain {
                        if info.never_run || !info.is_ok() {
                            failed = true;
                            let detail = info.err.clone().unwrap_or_default();
                            if detail.is_empty() {
                                error(format!("task failed: {}", info.cmd));
                            } else {
                                error(format!("task failed: {}: {}", info.cmd, detail));
                            }
                        }
                        results.push(info);
                    }
                }

                if failed && exit_on_first_error {
                    note(format!(
                        "exit_on_first_error is set, stopping after level {}",
                        level_idx + 1
                    ));
                    stop = true;
                }
            }
        }

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
