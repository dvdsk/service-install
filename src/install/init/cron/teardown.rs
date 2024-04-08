use std::iter;
use std::path::PathBuf;
use std::str::FromStr;

use itertools::Itertools;

use crate::install::init::extract_path;
use crate::install::init::{autogenerated_comment, ExeLocation, RSteps, TearDownError};
use crate::install::{Mode, Tense};
use crate::install::RemoveStep;

use super::Line;
use super::{current_crontab, set_crontab, GetCrontabError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not get the current crontab: {0}")]
    GetCrontab(#[from] GetCrontabError),
    // #[error("Failed to extract the path to the executable from crontab: {0}")]
    // NoExistingInstallFound(#[from] extract_path::Error),
    #[error("Comment for previous install at the end of the crontab")]
    CrontabCorrupt,
    #[error("{0}")]
    CrontabChanged(#[from] CrontabChanged),
    #[error("Rule in crontab corrupt, too short")]
    CorruptTooShort,
    // #[error("No rule from previous install in crontab")]
    // NoRule,
    // #[error("The command in crontab should not be empty/length zero")]
    // EmptyCommand,
    // #[error("The command is shell escaped but the second escape character is missing")]
    // EscapedEndMissing,
}

fn from_rule(rule: &str) -> PathBuf {
    let command = if let Some(command) = rule.strip_prefix("@reboot") {
        command.to_string()
    } else {
        rule.splitn(5 + 1, char::is_whitespace).skip(5).collect()
    };
    let command = match command.split_once("&&") {
        Some((_cd, command)) => command.to_string(),
        None => command,
    };

    let command = command.trim_start();
    let command = extract_path::split_unescaped_whitespace_once(command);

    PathBuf::from_str(&command).expect("infallible")
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_from_rule() {
        let case = "10 10 * * *  '/home/david/.local/hi bin/cron_only'";
        assert_eq!(
            &from_rule(case),
            Path::new("/home/david/.local/hi bin/cron_only")
        )
    }
}

pub(crate) fn tear_down_steps(
    bin_name: &str,
    mode: Mode,
    user: Option<&str>,
) -> Result<Option<(RSteps, ExeLocation)>, TearDownError> {
    assert!(
        !(mode.is_user() && user.is_some()),
        "need to run as system to set a different users crontab"
    );

    let current = current_crontab(user).map_err(Error::GetCrontab)?;
    let landmark_comment = autogenerated_comment(bin_name);

    let to_remove = current
        .windows(landmark_comment.lines().count() + 1)
        .map(|w| w.split_last().expect("window size always >= 2"))
        .find(|(_, comments)| {
            comments
                .iter()
                .map(Line::text)
                .eq(landmark_comment.lines())
        });

    let Some((rule, comment)) = to_remove else {
        return Ok(None);
    };

    let install_path = from_rule(&rule.text);
    let step = Box::new(RemoveInstalled {
        comments: comment.to_vec(),
        rule: rule.clone(),
        user: user.map(str::to_owned),
    }) as Box<dyn RemoveStep>;
    Ok(Some((vec![step], install_path)))
}

struct RemoveInstalled {
    user: Option<String>,
    comments: Vec<Line>,
    rule: Line,
}

impl RemoveStep for RemoveInstalled {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Present => "Removing",
            Tense::Future => "Will remove",
            Tense::Question => "Remove",
        };
        let user = self
            .user
            .as_ref()
            .map(|n| format!("{n}'s "))
            .unwrap_or_default();
        format!("{verb} the installs comment and rule from {user}crontab")
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Present => "Removing",
            Tense::Future => "Will remove",
            Tense::Question => "Remove",
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
        format!("{verb} the installs comment and rule from {user}crontab:\n| comment:{comment}\n| rule:\n{rule}")
    }

    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Self {
            comments,
            rule,
            user,
        } = self;
        let current_crontab = current_crontab(user.as_deref())?;
        let new_lines = filter_out(&current_crontab, rule, comments)?;

        let new_crontab: String = new_lines
            .into_iter()
            .interleave_shortest(iter::once("\n").cycle())
            .collect();
        set_crontab(&new_crontab, user.as_deref())?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Crontab was modified while uninstall ran, you should manually verify it")]
pub struct CrontabChanged;

pub(super) fn filter_out<'a>(
    input: &'a [Line],
    rule: &Line,
    comments: &[Line],
) -> Result<Vec<&'a str>, CrontabChanged> {
    // someone could store the steps and execute later, if
    // anything changed refuse to remove lines and abort
    let mut output = Vec::new();
    let mut to_remove = comments.iter().chain(iter::once(rule)).fuse();
    let mut next_to_remove = to_remove.next();
    for line in input {
        if let Some(next) = next_to_remove {
            if line.pos != next.pos {
                continue;
            }

            if line.text != next.text {
                return Err(CrontabChanged);
            }

            next_to_remove = to_remove.next();
            continue;
        }
        output.push(line.text.as_str());
    }

    Ok(output)
}
