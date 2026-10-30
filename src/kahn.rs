// akbar ST fl tarikh :(
// hada el fichier fih l'algorithme ta3 Kahn
//
// l'algorithme yekhdem hakda:
// 1. nhassbou chhal men dépendance 3and kol tâche
// 2. ndirou les tâches li ma 3andhomch dépendances fi liste esmha ready
// 3. nkharjou les tâches men ready, niveau b niveau
// 4. ki tekhlas tâche, nen9souha men les dépendances ta3 les tâches li tet3amed 3liha
// 5. ay tâche toussel l 0 twelli prête
// 6. ila bqaw des tâches ma wselouch l 0, ma3naha kayen cycle

use std::collections::HashMap;

use crate::json::Tasks;

// résultat ta3 l'algorithme
#[derive(Debug)]
pub struct KahnResult {
    // l'ordre: kol tâche tji mora les tâches li te3tamed 3liha
    pub order: Vec<String>,
    // les tâches m9assmin 3la niveaux, kol niveau ne9drou nchghlou m3a b3ad
    pub levels: Vec<Vec<String>>,
    // ila kanet true ma3naha kayen cycle, ya3ni ma 9edernach nertbou kolch
    pub has_cycle: bool,
}

// hna nebniw le graphe: l kol tâche nkhabyou les tâches li te3tamed 3liha,
// w nhassbou chhal men dépendance mazal ba9i.
//
// ila l9ina dépendance mahich fi tasks, ntannchouha hna.
// task_manager ye9der ytalla3 erreur wadha 9bel ma yebda le lancement.
fn build_graph(tasks: &Tasks) -> (HashMap<String, Vec<String>>, HashMap<String, usize>) {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut indegree: HashMap<String, usize> = HashMap::new();

    // nebdaou b liste fargha l kol tâche
    for id in tasks.keys() {
        graph.insert(id.clone(), Vec::new());
        indegree.insert(id.clone(), 0);
    }

    // ndourou 3la kol tâche, w nchoufou les dépendances ta3ha
    for (id, task) in tasks {
        for dep in &task.deps {
            // ki nel9aw la dépendance, nzidou la tâche fi liste li tet3amed 3liha
            if let Some(children) = graph.get_mut(dep) {
                children.push(id.clone());
            }

            // w nzidou wahed fi 3adad les dépendances ta3 la tâche
            if let Some(count) = indegree.get_mut(id) {
                *count += 1;
            }
        }
    }

    (graph, indegree)
}

// la fonction principale ta3 l'algorithme
pub fn sort(tasks: &Tasks) -> KahnResult {
    let (graph, mut indegree) = build_graph(tasks);

    // les tâches li ma 3andhomch dépendance rahom prêtes
    let mut ready: Vec<String> = Vec::new();
    for (id, count) in &indegree {
        if *count == 0 {
            ready.push(id.clone());
        }
    }
    ready.sort(); // nertbouhom bach résultat teb9a stable ki n'affichiwha

    let mut order: Vec<String> = Vec::new();
    let mut levels: Vec<Vec<String>> = Vec::new();

    // kol tour = niveau
    while !ready.is_empty() {
        // hada niveau raho kamel prêt
        let level = ready.clone();
        levels.push(level.clone());

        // nemshou ready bach n3emrouha b niveau li jay
        ready.clear();

        for id in &level {
            order.push(id.clone());

            if let Some(children) = graph.get(id) {
                for child in children {
                    if let Some(count) = indegree.get_mut(child) {
                        // nen9sou 3adad les dépendances
                        *count -= 1;

                        // hadi tâche ma b9atlhach dépendance, twelli prête
                        if *count == 0 {
                            ready.push(child.clone());
                        }
                    }
                }
            }
        }

        // nertbou niveau li jay
        ready.sort();
    }

    // les tâches li ma wselouch l 0 rahom bloqués fi cycle
    let has_cycle = order.len() != tasks.len();

    KahnResult {
        order,
        levels,
        has_cycle,
    }
}

// hna n7awlou kol niveau l liste ta3 commandes, bach ysehel nrebthouha m3a process::run_multi_chained
pub fn commands(result: &KahnResult, tasks: &Tasks) -> Vec<Vec<String>> {
    let mut all_commands: Vec<Vec<String>> = Vec::new();

    // ndourou 3la niveaux wahed b wahed
    for level in &result.levels {
        let mut level_commands: Vec<String> = Vec::new();

        // njam3ou les commandes ta3 les tâches li fi hada niveau
        for id in level {
            if let Some(task) = tasks.get(id) {
                level_commands.push(task.cmd.as_str().unwrap_or_default().to_string());
            }
        }

        // w nzidouhom fi liste kbira
        all_commands.push(level_commands);
    }

    all_commands
}
