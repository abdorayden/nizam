// hada el fichier ykhoss les logs ta3 l'engine
//
// 3andna 3 niveaux:
// - error: mouchkil, yrouh l stderr
// - warning: haja ghalta mais l'engine ykemmel, yrouh l stderr
// - note: message 3adi, yrouh l stdout
//
// l'engine yest3mel des callbacks: fn(String).
// 3lach wrappers? khatach l'engine yched ghi fonction, machi struct.
//
// ila bghit tzid fichier wla couleur, dirha hna f had les fonctions.

use std::time::Duration;

use crate::engine::Execution;

// nketbou les erreurs f stderr
pub fn log_error(message: String) {
    eprintln!("  [error] {message}");
}

// nketbou les warnings f stderr
pub fn log_warning(message: String) {
    eprintln!("  [warning] {message}");
}

// nketbou les notes f stdout
pub fn log_note(message: String) {
    println!("  [note] {message}");
}

// hna nketbou rizilta ta3 l'exécution:
// - [ok] ila la tâche kmellet mliha
// - [failed] ila la tâche fachlet
// - [skipped] ila la tâche ma tlansatch
// w f lekher nketbou chhal men niveau, ok, failed w l'wa9t.
pub fn log_report(report: &Execution, elapsed: Duration) {
    let mut ok = 0;
    let mut failed = 0;

    for info in &report.results {
        if info.never_run {
            println!("  [skipped] {}", info.cmd);
            failed += 1;
        } else if info.is_ok() {
            println!("  [ok] {}", info.cmd);
            ok += 1;
        } else {
            println!("  [failed] {}", info.cmd);
            failed += 1;
        }
    }

    println!(
        "  {} level(s), {ok} ok, {failed} failed, {:.2?}",
        report.schedule.levels.len(),
        elapsed
    );
}
