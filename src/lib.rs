/// Python bindings for the Nizam task runner.
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

pub mod engine;
pub mod json;
pub mod kahn;
pub mod process;
pub mod task_manager;
// pub mod logs;

use crate::engine::{Engine, Execution};
use crate::process::ProcInfo;

#[pyclass(name = "Engine")]
struct EngineWrapper {
    engine: Engine,
}

#[pymethods]
impl EngineWrapper {
    #[new]
    fn new(template: String) -> Self {
        Self {
            engine: Engine::new(&template),
        }
    }

    /// Load, validate, and execute the template, returning its report.
    fn run(&self, py: Python<'_>) -> PyResult<Py<ExecutionWrapper>> {
        match self.engine.execute(log_error, log_warning, log_note) {
            Some(execution) => Py::new(py, ExecutionWrapper { execution }),
            None => Err(PyRuntimeError::new_err(
                "template could not be loaded or scheduled; see stderr for details",
            )),
        }
    }
}

// replace all those functions with prepared callbacks from logs module
fn log_error(message: String) {
    eprintln!("nizam error: {message}");
}

fn log_warning(message: String) {
    eprintln!("nizam warning: {message}");
}

fn log_note(message: String) {
    eprintln!("nizam: {message}");
}

#[pyclass(name = "Execution")]
struct ExecutionWrapper {
    execution: Execution,
}

#[pymethods]
impl ExecutionWrapper {
    /// Task ids in topological execution order.
    #[getter]
    fn order(&self) -> Vec<String> {
        self.execution.schedule.order.clone()
    }

    /// Task ids grouped by dependency level.
    #[getter]
    fn levels(&self) -> Vec<Vec<String>> {
        self.execution.schedule.levels.clone()
    }

    /// Results in the order tasks were executed.
    #[getter]
    fn results(&self) -> Vec<ProcInfoWrapper> {
        self.execution
            .results
            .iter()
            .map(|info| ProcInfoWrapper::from(info))
            .collect()
    }

    #[getter]
    fn succeeded(&self) -> bool {
        self.execution.results.iter().all(ProcInfo::is_ok)
    }
}

#[pyclass(name = "TaskResult")]
struct ProcInfoWrapper {
    cmd: String,
    status: Option<i32>,
    err: Option<String>,
    out: Option<String>,
    never_run: bool,
    succeeded: bool,
}

impl From<&ProcInfo> for ProcInfoWrapper {
    fn from(info: &ProcInfo) -> Self {
        Self {
            cmd: info.cmd.clone(),
            status: info.status.code(),
            err: info.err.clone(),
            out: info
                .out
                .as_ref()
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned()),
            never_run: info.never_run,
            succeeded: info.is_ok(),
        }
    }
}

#[pymethods]
impl ProcInfoWrapper {
    #[getter]
    fn cmd(&self) -> &str {
        &self.cmd
    }

    #[getter]
    fn status(&self) -> Option<i32> {
        self.status
    }

    #[getter]
    fn err(&self) -> Option<&str> {
        self.err.as_deref()
    }

    #[getter]
    fn out(&self) -> Option<&str> {
        self.out.as_deref()
    }

    #[getter]
    fn never_run(&self) -> bool {
        self.never_run
    }

    #[getter]
    fn succeeded(&self) -> bool {
        self.succeeded
    }
}

#[pyfunction]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[pymodule]
fn nizam(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EngineWrapper>()?;
    m.add_class::<ExecutionWrapper>()?;
    m.add_class::<ProcInfoWrapper>()?;
    m.add_function(wrap_pyfunction!(add, m)?)?;
    Ok(())
}
