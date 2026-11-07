// Copyright (c) 2026 Computer Science Security Team . All Rights Reserved.

// NOTE: for windows processes you might be add cmd /C prefix

/// process managment

use std::process::{
    Command,
    Child,
    ExitStatus,
    Output,
    Stdio
};

use std::io::Result;

///
/// ProcInfo is struct that holds the informations of the process that is already ran and finished
///
#[derive(Debug, PartialEq, Default)]
pub struct ProcInfo {
    pub cmd : String,
    pub status : ExitStatus,
    pub err : Option<String>,
    pub out : Option<Output>,
    pub never_run: bool,
}

impl ProcInfo {
    fn new(cmd: String, status: ExitStatus, err: Option<String>, out: Option<Output>, never_run: bool) -> Self {
        Self {
            cmd,
            status,
            err,
            out,
            never_run,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status.success()
    }
}

/// Waits for a spawned command and captures its output as a `ProcInfo`.
/// This can be called from a waiter thread so the engine can react as soon as
/// any task completes instead of waiting for a whole batch.
pub fn wait_child(cmd: String, child: Child) -> ProcInfo {
    match child.wait_with_output() {
        Ok(output) => {
            let status = output.status;
            let err = if output.stderr.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            };
            ProcInfo::new(cmd, status, err, Some(output), false)
        }
        Err(e) => ProcInfo::new(
            cmd,
            ExitStatus::default(),
            Some(e.to_string()),
            None,
            true,
        ),
    }
}

/// Creates the result for a command that could not be spawned.
pub fn spawn_failure(cmd: String, error: std::io::Error) -> ProcInfo {
    ProcInfo::new(
        cmd,
        ExitStatus::default(),
        Some(error.to_string()),
        None,
        true,
    )
}

/// CommandMultiChainedList is type that represent list of chained deps list of commands
/// {
///     {
///         "gcc foo.c -c -o foo.o",
///         "gcc main.c -o main foo.o"
///     },
///     {
///         "gcc bar.c -c -o bar.o",
///         "gcc another_main.c -o another_main bar.o"
///     },
/// }
///
pub type CommandChainedList = Vec<String>;
pub type CommandMultiChainedList = Vec<CommandChainedList>;
pub type ProcInfoChainedList = Vec<Vec<ProcInfo>>;

pub fn run_cmd(cmd: String) -> Result<Command> {
    let splitted: Vec<&str> = cmd.split(' ').collect();
    let mut command = Command::new(splitted[0]);
    for i in 1..splitted.len() {
        command.arg(splitted[i]);
    }
    Ok(command)
}

pub fn run_cmd_async(cmd: String) -> Result<Child>{
    let splitted: Vec<&str> = cmd.split(' ').collect();
    let mut command = Command::new(splitted[0]);
    for i in 1..splitted.len() {
        command.arg(splitted[i]);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

///run_chained must be a single proc because it's chained and each command has a dep
pub fn run_chained(chained_commands: CommandChainedList) -> Vec<ProcInfo> {
    let mut list_of_chained_commands: Vec<ProcInfo> = Vec::new();

    for cmd_str in &chained_commands {
        let cmd_str_owned = cmd_str.clone();
        match run_cmd(cmd_str_owned.clone()) {
            Ok(mut command) => {
                match command.output() {
                    Ok(output) => {
                        let status = output.status;
                        let err = if output.stderr.is_empty() {
                            None
                        } else {
                            Some(String::from_utf8_lossy(&output.stderr).to_string())
                        };
                        let out = Some(output);
                        let info = ProcInfo::new(cmd_str_owned, status, err, out, false);
                        list_of_chained_commands.push(info);
                    }
                    Err(e) => {
                        let info = ProcInfo::new(
                            cmd_str_owned,
                            ExitStatus::default(),
                            Some(e.to_string()),
                            None,
                            true,
                        );
                        list_of_chained_commands.push(info);
                    }
                }
            }
            Err(e) => {
                let info = ProcInfo::new(
                    cmd_str_owned,
                    ExitStatus::default(),
                    Some(e.to_string()),
                    None,
                    true,
                );
                list_of_chained_commands.push(info);
            }
        }
    }
    list_of_chained_commands
}

// NOTE: steps (round based, one child process per chain command)
// round 0: spawn "gcc foo.c -c -o foo.o" and "gcc bar.c -c -o bar.o" (each in its own child), wait for them all
// round 1: spawn "gcc main.c -o main foo.o" and "gcc another_main.c -o another_main bar.o", wait for them all
// repeat until every chain has no commands left
// helper function
fn run_chained_child(chained_commands: &CommandChainedList, index: usize) -> Result<(Option<Child>, bool)> {
    if index >= chained_commands.len() {
        return Ok((None, true));
    }
    let child_proc = run_cmd_async(chained_commands[index].clone())?;
    Ok((Some(child_proc), index + 1 == chained_commands.len()))
}

/// run_multi_chained runs every inner chain (deps are sequential inside a chain).
/// `separate_proc: false` -> chains run one after another in this proc, otherwise every
/// command of a chain runs in its own child process and all chains advance round by round
pub fn run_multi_chained(
    multi_chained: CommandMultiChainedList,
    separate_proc: bool,
) -> ProcInfoChainedList {
    if !separate_proc {
        let mut ret: ProcInfoChainedList = Vec::new();
        for chain in &multi_chained {
            let info_chain = run_chained(chain.clone());
            ret.push(info_chain);
        }
        return ret;
    }

    let mut handles: ProcInfoChainedList = multi_chained
        .iter()
        .map(|chain| Vec::with_capacity(chain.len()))
        .collect();
    let mut indices: Vec<usize> = vec![0; multi_chained.len()];

    loop {
        let mut pending: Vec<(usize, String, Child)> = Vec::new();
        let mut any_active = false;

        for (chain_idx, chain) in multi_chained.iter().enumerate() {
            let index = indices[chain_idx];
            if index >= chain.len() {
                continue;
            }
            any_active = true;
            let cmd = chain[index].clone();
            match run_chained_child(chain, index) {
                Ok((Some(child), _)) => pending.push((chain_idx, cmd, child)),
                Ok((None, _)) => {}
                Err(e) => {
                    handles[chain_idx].push(ProcInfo::new(
                        cmd,
                        ExitStatus::default(),
                        Some(e.to_string()),
                        None,
                        true,
                    ));
                    indices[chain_idx] += 1;
                }
            }
        }

        if !any_active {
            break;
        }

        for (chain_idx, cmd, child) in pending {
            match child.wait_with_output() {
                Ok(output) => {
                    let status = output.status;
                    let err = if output.stderr.is_empty() {
                        None
                    } else {
                        Some(String::from_utf8_lossy(&output.stderr).to_string())
                    };
                    let out = Some(output);
                    handles[chain_idx].push(ProcInfo::new(cmd, status, err, out, false));
                }
                Err(e) => {
                    handles[chain_idx].push(ProcInfo::new(
                        cmd,
                        ExitStatus::default(),
                        Some(e.to_string()),
                        None,
                        true,
                    ));
                }
            }
            indices[chain_idx] += 1;
        }
    }
    handles
}
