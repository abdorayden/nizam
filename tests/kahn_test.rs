use std::collections::HashMap;

use nizam::json::{Cmd, TaskOptions};
use nizam::kahn;

// nsan3ou tâches test bach nst3mlouhom fi les tests
fn make_tasks(items: &[(&str, &[&str])]) -> HashMap<String, TaskOptions> {
    let mut map = HashMap::new();

    for item in items {
        // nakhdou l'id w les dépendances
        let id = item.0;
        let deps = item.1;

        // n7awlou les dépendances men &str l String
        let mut dep_list: Vec<String> = Vec::new();
        for dep in deps {
            dep_list.push(dep.to_string());
        }

        // nsan3ou tâche
        let task = TaskOptions {
            cmd: Cmd::SimplCmd(format!("echo {id}")),
            deps: dep_list,
        };

        // w nzidouha fi map
        map.insert(id.to_string(), task);
    }

    map
}

// petite fonction terj3 liste ta3 String
fn strings(items: &[&str]) -> Vec<String> {
    let mut result = Vec::new();

    for item in items {
        result.push(item.to_string());
    }

    result
}

#[test]
fn sorts_a_chain() {
    // chaîne: 1 -> 2 -> 3
    let map = make_tasks(&[("1", &[]), ("2", &["1"]), ("3", &["2"])]);
    let result = kahn::sort(&map);

    assert_eq!(result.order, strings(&["1", "2", "3"]));
    assert_eq!(
        result.levels,
        vec![strings(&["1"]), strings(&["2"]), strings(&["3"])]
    );
    assert!(!result.has_cycle);
}

#[test]
fn independent_tasks_are_in_the_same_level() {
    // 1 w 2 ma 3andhomch dépendance, ya3ni rahom fi nafs niveau
    let map = make_tasks(&[("2", &[]), ("1", &[]), ("3", &["1"])]);
    let result = kahn::sort(&map);

    assert_eq!(result.levels[0], strings(&["1", "2"]));
    assert_eq!(result.levels[1], strings(&["3"]));
}

#[test]
fn detects_a_cycle() {
    // 1 yet3amed 3la 2, w 2 yet3amed 3la 1: hadi cycle
    let map = make_tasks(&[("1", &["2"]), ("2", &["1"])]);
    let result = kahn::sort(&map);

    assert!(result.has_cycle);
    assert!(result.order.is_empty());
}

#[test]
fn converts_levels_to_commands() {
    // kol niveau ywelli liste ta3 commandes
    let map = make_tasks(&[("1", &[]), ("2", &["1"])]);
    let result = kahn::sort(&map);

    assert_eq!(
        kahn::commands(&result, &map),
        vec![strings(&["echo 1"]), strings(&["echo 2"])]
    );
}
