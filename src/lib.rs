/// functionalities that used in python
use pyo3::prelude::*;

pub mod json;
pub mod engine;
pub mod kahn;
// pub mod logs;
pub mod process;
pub mod task_manager;

#[pyfunction]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[pymodule]
fn nizam(m: &Bound<'_ , PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(add, m)?)?;
    Ok(())
}
