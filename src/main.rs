use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nizam::engine::{Engine, Execution};

fn log_error(msg: String) {
    eprintln!("  [error] {msg}");
}

fn log_warning(msg: String) {
    eprintln!("  [warning] {msg}");
}

fn log_note(msg: String) {
    println!("  [note] {msg}");
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn collect_paths(args: &[String]) -> Vec<PathBuf> {
    let targets: Vec<PathBuf> = if args.is_empty() {
        vec![PathBuf::from("templates")]
    } else {
        args.iter().map(PathBuf::from).collect()
    };

    let mut paths: Vec<PathBuf> = Vec::new();

    for target in targets {
        if target.is_dir() {
            paths.extend(json_files(&target));
        } else {
            paths.push(target);
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn print_report(report: &Execution, elapsed: Duration) {
    let failed = report
        .results
        .iter()
        .filter(|info| info.never_run || !info.is_ok())
        .count();
    let ok = report.results.len() - failed;

    for info in &report.results {
        let status = if info.never_run {
            "skipped"
        } else if info.is_ok() {
            "ok"
        } else {
            "failed"
        };
        println!("  [{status}] {}", info.cmd);
    }

    println!(
        "  {} level(s), {ok} ok, {failed} failed, {:.2?}",
        report.schedule.levels.len(),
        elapsed
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let paths = collect_paths(&args);

    if paths.is_empty() {
        eprintln!("no json templates found (looked in `templates/`)");
        std::process::exit(1);
    }

    let mut failed = 0usize;

    for path in &paths {
        println!("\n== {} ==", path.display());

        let engine = Engine::new(path.to_str().expect("template path must be valid utf-8"));
        let start = Instant::now();

        match engine.execute(log_error, log_warning, log_note) {
            Some(report) => {
                print_report(&report, start.elapsed());

                if report
                    .results
                    .iter()
                    .any(|info| info.never_run || !info.is_ok())
                {
                    failed += 1;
                }
            }
            None => {
                println!("  template could not be executed");
                failed += 1;
            }
        }
    }

    println!(
        "\n{}/{} template(s) completed successfully",
        paths.len() - failed,
        paths.len()
    );

    if failed > 0 {
        std::process::exit(1);
    }
}
