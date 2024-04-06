use std::fmt;
use std::io::Write;
use std::process::{Command, Stdio};

use super::{Params, SetupError, Steps};
use crate::install::Rollback;

pub mod setup;
pub mod teardown;

pub(crate) use setup::set_up_steps;
pub(crate) use teardown::tear_down_steps;

pub(super) fn not_available() -> Result<bool, SetupError> {
    use sysinfo::{ProcessRefreshKind, System, UpdateKind};
    let mut s = System::new();
    s.refresh_processes_specifics(ProcessRefreshKind::new().with_cmd(UpdateKind::Always));
    let cron_running = s.processes().iter().any(|(_, process)| {
        process
            .cmd()
            .iter()
            .any(|part| part.ends_with("/cron"))
    });
    Ok(!cron_running)
}

struct RollbackImpossible;
#[derive(Debug, thiserror::Error)]
#[error("Can not rollback setting up cron, must be done manually")]
struct RollbackImpossibleErr;

impl Rollback for RollbackImpossible {
    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Err(Box::new(RollbackImpossibleErr))
    }

    fn describe(&self) -> String {
        "Rollback of cron setup is not possible, check crontab manually".to_string()
    }
}

#[derive(Debug, Clone)]
struct Line {
    /// line number in the crontab
    pos: usize,
    text: String,
}

impl Line {
    fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { pos, text } = self;
        f.write_fmt(format_args!("{pos}: {text}"))
    }
}

#[must_use]
fn crontab_lines(text: String) -> Vec<Line> {
    const HEADER_ADDED_BY_LIST_CMD: &str = "# DO NOT EDIT THIS FILE";
    let header_lines = if text.starts_with(HEADER_ADDED_BY_LIST_CMD) {
        3
    } else {
        0
    };

    text.lines()
        .skip(header_lines)
        .map(str::to_owned)
        .enumerate()
        .map(|(source, text)| Line { text, pos: source })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum GetCrontabError {
    #[error("Could not run the crontab program: {0}")]
    CouldNotRun(std::io::Error),
    #[error("Command `crontab -l` failed, stderr:\n\t")]
    CommandFailed { stderr: String },
}

fn current_crontab(user: Option<&str>) -> Result<Vec<Line>, GetCrontabError> {
    let mut command = Command::new("crontab");
    command.arg("-l");
    if let Some(user) = user {
        command.arg("-u");
        command.arg(user);
    }

    let output = command.output().map_err(GetCrontabError::CouldNotRun)?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).expect("crontab should return utf8");
        let crontab = crontab_lines(stdout);
        return Ok(crontab);
    }

    let stderr = String::from_utf8(output.stderr).expect("crontab should return utf8");
    Err(GetCrontabError::CommandFailed { stderr })
}

#[derive(Debug, thiserror::Error)]
enum SetCrontabError {
    #[error("Could not run the crontab program: {0}")]
    CouldNotRun(std::io::Error),
    #[error("Command `crontab -` failed, stderr:\n\t")]
    CommandFailed { stderr: String },
    #[error("Failed to open crontab stdin")]
    StdinClosed,
    #[error("Error while writing to crontab's stdin: {0}")]
    WritingStdin(std::io::Error),
    #[error("Could not wait on output of crontab program, err: {0}")]
    FailedToWait(std::io::Error),
}

fn set_crontab(new_crontab: String, user: Option<&str>) -> Result<(), SetCrontabError> {
    let mut command = Command::new("crontab");
    command.arg("-");
    if let Some(user) = user {
        command.arg("-u");
        command.arg(user);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(SetCrontabError::CouldNotRun)?;

    let mut stdin = child.stdin.take().ok_or(SetCrontabError::StdinClosed)?;
    stdin
        .write_all(new_crontab.as_bytes())
        .map_err(SetCrontabError::WritingStdin)?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .map_err(SetCrontabError::FailedToWait)?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8(output.stderr).expect("crontab should return utf8");
        Err(SetCrontabError::CommandFailed { stderr })
    }
}
