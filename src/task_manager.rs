/// tasks managment
/// this module depends on kahn's module
/// using the algorithm i can detect cycle and represent a good format for engine to execute

#[warn(unused_imports)]
use super::json::Tasks;


/// returns the first dependency (sorted) that does not point to an existing task
pub fn find_missing_dep(tasks: &Tasks) -> Option<(String, String)> {
    let mut missing: Vec<(String, String)> = Vec::new();

    for (id, task) in tasks {
        for dep in &task.deps {
            if !tasks.contains_key(dep) {
                missing.push((id.clone(), dep.clone()));
            }
        }
    }

    missing.sort();
    missing.into_iter().next()
}
