use std::iter;
use std::path::PathBuf;

use itertools::Itertools;

use super::{teardown, Params, SetupError, Steps};
use crate::install::builder::Trigger;
use crate::install::init::{autogenerated_comment, ShellEscape};
use crate::install::{InstallError, InstallStep, RollbackStep, Tense};
use crate::schedule::Schedule;

use super::Line;
use super::RollbackImpossible;
use super::{current_crontab, set_crontab};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Command `crontab -l` failed, stderr:\n\t{stderr}")]
    ListFailed { stderr: String },
    #[error("Could not get the current crontab")]
    GetCrontab(#[source] super::GetCrontabError),
    #[error("Comment for previous install at the end of the crontab")]
    CrontabCorrupt,
    #[error("Failed to open crontab stdin")]
    StdinClosed,
    #[error("Error while writing to crontab's stdin")]
    WritingStdin(#[source] std::io::Error),
    #[error("Could not wait on output of crontab program")]
    FailedToWait(#[source] std::io::Error),
    #[error("Crontab was modified while installation ran, you should manually verify it")]
    CrontabChanged,
    #[error("Could not find an existing install in crontab")]
    NoExistingInstallFound,
}

pub(crate) fn set_up_steps(params: &Params) -> Result<Steps, SetupError> {
    use Schedule as S;
    use Trigger::{OnBoot, OnSchedule};

    let current = current_crontab(params.run_as.as_deref()).map_err(Error::GetCrontab)?;
    let landmark_comment = autogenerated_comment(params.bin_name);

    let to_remove = current
        .windows(landmark_comment.lines().count() + 1)
        .map(|w| w.split_last().expect("window size always >= 2"))
        .find(|(_, comments)| comments.iter().map(Line::text).eq(landmark_comment.lines()));

    let mut steps = Vec::new();
    if let Some((rule, comment)) = to_remove {
        steps.push(Box::new(RemovePrevious {
            comments: comment.to_vec(),
            rule: rule.clone(),
            user: params.run_as.clone(),
        }) as Box<dyn InstallStep>);
    }

    let when = match params.trigger {
        OnSchedule(S::Daily(time)) => format!("{} {} * * *", time.minute(), time.hour()),
        OnBoot => "@reboot".to_owned(),
    };

    let exe_path = params.exe_path.shell_escaped();
    let exe_args: String = params.exe_args.iter().map(String::shell_escaped).join(" ");
    let set_working_dir = params
        .working_dir
        .as_ref()
        .map(PathBuf::shell_escaped)
        .map(|dir| format!("cd {dir} && "))
        .unwrap_or_default();
    let set_env_vars = if params.environment.is_empty() {
        String::new()
    } else {
        "export".to_owned()
            + &params
                .environment
                .iter()
                .map(|(key, val)| format!("{}={}", key.shell_escaped(), val.shell_escaped()))
                .join(" ")
            + ";"
    };

    let command = format!("{set_env_vars}{set_working_dir}{exe_path} {exe_args}");
    let rule = format!("{when} {command}");

    steps.push(Box::new(Add {
        user: params.run_as.clone(),
        comment: landmark_comment,
        rule,
    }));
    Ok(steps)
}

#[derive(Debug, Clone)]
pub(crate) struct Add {
    pub(crate) user: Option<String>,
    pub(crate) comment: String,
    pub(crate) rule: String,
}

impl InstallStep for Add {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Appended",
            Tense::Questioning => "Append",
            Tense::Future => "Will append",
            Tense::Active => "Appending",
        };
        if let Some(run_as) = &self.user {
            format!(
                "{verb} comment and rule to {run_as}'s crontab{}",
                tense.punct()
            )
        } else {
            format!("{verb} comment and rule to crontab{}", tense.punct())
        }
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Appended",
            Tense::Questioning => "Append",
            Tense::Future => "Will append",
            Tense::Active => "Appending",
        };
        let Self {
            comment,
            rule,
            user,
        } = self;
        let comment = comment.replace('\n', "\n|\t");
        if let Some(run_as) = user {
            format!(
                "{verb} comment and rule to {run_as}'s crontab{}\n| comment:\n|\t{comment}\n| rule:\n|\t{rule}", tense.punct()
            )
        } else {
            format!(
                "{verb} comment and rule to crontab{}\n| comment:\n|\t{comment}\n| rule:\n|\t{rule}", tense.punct()
            )
        }
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let Self {
            comment,
            rule,
            user,
        } = self.clone();
        let current_crontab = current_crontab(user.as_deref())?;
        let new_crontab: String = current_crontab
            .iter()
            .map(Line::text)
            .chain(iter::once(comment.as_str()))
            .chain(iter::once(rule.as_str()))
            .interleave_shortest(iter::once("\n").cycle())
            .chain(iter::once("\n")) // some say cron likes a newline at the end
            .collect();
        set_crontab(&new_crontab, user.as_deref())?;

        Ok(Some(Box::new(RollbackImpossible)))
    }
}
pub(crate) struct RemovePrevious {
    pub(crate) comments: Vec<Line>,
    pub(crate) rule: Line,
    pub(crate) user: Option<String>,
}
impl InstallStep for RemovePrevious {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Questioning => "Remove",
            Tense::Future => "Will remove",
            Tense::Active => "Removing",
        };
        let user = self
            .user
            .as_ref()
            .map(|n| format!("{n}'s "))
            .unwrap_or_default();
        format!(
            "{verb} comment and rule from previous installation from {user}crontab{}",
            tense.punct()
        )
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Questioning => "Remove",
            Tense::Future => "Will remove",
            Tense::Active => "Removing",
        };
        let user = self
            .user
            .as_ref()
            .map(|n| format!("{n}'s "))
            .unwrap_or_default();
        #[allow(clippy::format_collect)]
        let comment: String = self
            .comments
            .iter()
            .map(|Line { pos, text }| format!("\n|\t{pos}: {text}"))
            .collect();
        let rule = format!("|\t{}: {}", self.rule.pos, self.rule.text);
        format!("{verb} a comment and rule from previous installation from {user}crontab{}\n| comment:\t{comment}\n| rule:\n{rule}", tense.punct())
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let Self {
            comments,
            rule,
            user,
        } = self;
        let current_crontab = current_crontab(user.as_deref())?;

        let new_lines = teardown::filter_out(&current_crontab, rule, comments)?;

        let new_crontab: String = new_lines
            .into_iter()
            .interleave_shortest(iter::repeat("\n"))
            .collect();
        set_crontab(&new_crontab, user.as_deref())?;

        Ok(Some(Box::new(RollbackImpossible)))
    }
}
