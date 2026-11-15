// hada el fichier ykhoss les logs ta3 l'engine
//
// kol ligne fiha:
// - date w wa9t (UTC)
// - niveau: INFO / WARNING / ERROR
// - PID
// - thread id
// - la tâche (ila kayna)
// - message: start / end / error / code de retour ...
//
// les logs yrouhou l console. ila bghit yrouhou l fichier,
// dir variable d'environnement NIZAM_LOG_FILE=/path/to/file.log
//
// l'engine yest3mel des callbacks: fn(String).
// 3lach wrappers? khatarch l'engine yched ghi fonction, machi struct.

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::engine::Execution;

// les niveaux ta3 log
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

// nsemmiw niveau kima yban f log
fn level_name(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Info => "INFO",
        LogLevel::Warning => "WARNING",
        LogLevel::Error => "ERROR",
    }
}

// hadi t7awwel seconds men 1970 l date w wa9t (UTC)
fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;

    let hour = rest / 3_600;
    let minute = (rest % 3_600) / 60;
    let second = rest % 60;

    let (year, month, day) = date_from_days(days);

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

// hadi t7awwel 3adad les jours men 1970 l (year, month, day)
fn date_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    (year, month as u32, day as u32)
}

// nketbou ligne f console, w ila NIZAM_LOG_FILE mawjoud nzidouha f fichier
fn write_line(line: &str, level: LogLevel) {
    match level {
        LogLevel::Error | LogLevel::Warning => eprintln!("{line}"),
        LogLevel::Info => println!("{line}"),
    }

    if let Ok(path) = env::var("NIZAM_LOG_FILE") {
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = writeln!(file, "{line}") {
                    eprintln!("  [error] cannot write log file `{path}`: {error}");
                }
            }
            Err(error) => {
                eprintln!("  [error] cannot open log file `{path}`: {error}");
            }
        }
    }
}

// hada struct yemtel trace wa7da kamla
// n9edrou nrj3ouh men make_entry w n'affichiwouh b log_entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub pid: u32,
    pub thread_id: thread::ThreadId,
    pub task: Option<String>,
    pub message: String,
}

impl LogEntry {
    // nwajdo ligne kamla men les champs ta3 struct
    pub fn line(&self) -> String {
        format!(
            "{} [{:7}] pid={} thread={:?} task={} {}",
            self.timestamp,
            level_name(self.level),
            self.pid,
            self.thread_id,
            self.task.as_deref().unwrap_or("-"),
            self.message
        )
    }
}

// hadi traj3 struct LogEntry, ma t'affichich walou
pub fn make_entry(level: LogLevel, task: Option<&str>, message: &str) -> LogEntry {
    LogEntry {
        timestamp: timestamp(),
        level,
        pid: process::id(),
        thread_id: thread::current().id(),
        task: task.map(str::to_string),
        message: message.to_string(),
    }
}

// nketbou LogEntry f console/fichier
pub fn log_entry(entry: LogEntry) {
    write_line(&entry.line(), entry.level);
}

// hadi la fonction li tketb log kamel
pub fn log(level: LogLevel, task: Option<&str>, message: &str) {
    log_entry(make_entry(level, task, message));
}

// callbacks ta3 l'engine
pub fn log_error(message: String) {
    log(LogLevel::Error, None, &message);
}

pub fn log_warning(message: String) {
    log(LogLevel::Warning, None, &message);
}

pub fn log_note(message: String) {
    log(LogLevel::Info, None, &message);
}

// nloggiw début ta3 tâche m3a la commande
pub fn log_start(task: &str, cmd: &str) {
    log(LogLevel::Info, Some(task), &format!("start cmd={cmd}"));
}

// nloggiw fin ta3 tâche m3a code de retour
pub fn log_end(task: &str, code: Option<i32>) {
    let level = match code {
        Some(0) | None => LogLevel::Info,
        Some(_) => LogLevel::Error,
    };

    match code {
        Some(code) => log(level, Some(task), &format!("end code={code}")),
        None => log(level, Some(task), "end"),
    }
}

// error m3a la tâche
pub fn log_task_error(task: &str, message: &str) {
    log(LogLevel::Error, Some(task), message);
}

// timeout ta3 tâche
pub fn log_timeout(task: &str) {
    log(LogLevel::Error, Some(task), "timeout");
}

// l'utilisateur 7abes l'exécution
pub fn log_interrupt() {
    log(LogLevel::Warning, None, "interrupted by user");
}

// résumé ta3 l'exécution.
// les start/end/errors ta3 kol tâche ytketbou déjà f engine,
// donc hna nketbou ghi le résumé bach ma n3awdouch nafs les lignes.
pub fn log_report(report: &Execution, elapsed: Duration) {
    let failed = report
        .results
        .iter()
        .filter(|info| info.never_run || !info.is_ok())
        .count();
    let ok = report.results.len() - failed;
    let skipped = report.schedule.order.len() - report.results.len();

    let summary = format!(
        "{} level(s), {ok} ok, {failed} failed, {skipped} skipped, {:.2?}",
        report.schedule.levels.len(),
        elapsed
    );
    log(LogLevel::Info, None, &summary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_entry_returns_a_trace_struct() {
        let entry = make_entry(LogLevel::Info, Some("build"), "start");

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.pid, process::id());
        assert_eq!(entry.task.as_deref(), Some("build"));
        assert_eq!(entry.message, "start");
        assert!(!entry.timestamp.is_empty());
        assert!(entry.line().contains("task=build"));
    }
}
