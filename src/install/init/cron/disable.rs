use std::iter;
use std::path::Path;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use itertools::Itertools;
use sysinfo::Pid;
use sysinfo::ProcessRefreshKind;
use sysinfo::Signal;

use crate::install::init::autogenerated_comment;
use crate::install::init::cron::setup::RemovePrevious;
use crate::install::init::cron::Line;
use crate::install::InstallError;
use crate::install::InstallStep;
use crate::install::RollbackError;
use crate::install::RollbackStep;
use crate::Tense;

use super::current_crontab;
use super::set_crontab;
use super::teardown::CrontabChanged;
use super::GetCrontabError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not get the current crontab")]
    GetCrontab(GetCrontabError),
    #[error("Failed to find a rule starting the target")]
    NoRuleFound,
    #[error("Process spawnedby cron will not stop")]
    FailedToStop,
}

pub(crate) fn step(
    target: &Path,
    pid: Pid,
    run_as: Option<&str>,
) -> Result<Vec<Box<dyn InstallStep>>, Error> {
    let crontab = current_crontab(run_as).map_err(Error::GetCrontab)?;

    let bin_name = target
        .file_name()
        .expect("target always gets a file name")
        .to_str()
        .expect("file name is valid ascii");
    let landmark_comment = autogenerated_comment(bin_name);

    let previous_install = crontab
        .windows(landmark_comment.lines().count() + 1)
        .map(|w| w.split_last().expect("window size always >= 2"))
        .find(|(_, comments)| comments.iter().map(Line::text).eq(landmark_comment.lines()));

    if let Some((rule, comment)) = previous_install {
        Ok(vec![
            Box::new(RemovePrevious {
                comments: comment.to_vec(),
                rule: rule.clone(),
                user: run_as.map(String::from),
            }) as Box<dyn InstallStep>,
            Box::new(Kill { pid }) as Box<dyn InstallStep>,
        ])
    } else if let Some(line) = crontab
        .into_iter()
        .filter_map(|line| line.exec().zip(Some(line)))
        .find(|(exec, _)| exec == target)
        .map(|(_, line)| line)
    {
        Ok(vec![
            Box::new(CommentOutRule {
                rule: line,
                user: run_as.map(String::from),
            }) as Box<dyn InstallStep>,
            Box::new(Kill { pid }) as Box<dyn InstallStep>,
        ])
    } else {
        Ok(vec![Box::new(Kill { pid }) as Box<dyn InstallStep>])
    }
}

struct Kill {
    pid: Pid,
}

impl InstallStep for Kill {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Stopped",
            Tense::Present => "Stop",
            Tense::Future => "Will stop",
            Tense::Active => "Stopping",
        };
        let pid = self.pid;
        format!("{verb} the service started by cron with pid: `{pid}`")
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Stopped",
            Tense::Present => "Stop",
            Tense::Future => "Will stop",
            Tense::Active => "Stopping",
        };
        let pid = self.pid;
        format!("{verb} the service started by cron with pid: `{pid}`\n| using signal:\n|\t - Stop\n| if that does not work:\n|\t - Kill\n| and if that fails:\n|\t - Abort")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        const ESCALATE: Duration = Duration::from_millis(200);
        let mut last_attempt = Instant::now()
            .checked_sub(ESCALATE)
            .expect("Instant should not be at unix zero aka 1970");
        let mut signals = [Signal::Stop, Signal::Kill, Signal::Abort].into_iter();

        loop {
            let mut s = sysinfo::System::new();
            s.refresh_process_specifics(self.pid, ProcessRefreshKind::new());
            let Some(process) = s.process(self.pid) else {
                return Ok(None);
            };

            if last_attempt.elapsed() < ESCALATE {
                continue;
            }

            last_attempt = Instant::now();
            let signal = signals.next().ok_or(InstallError::CouldNotStop)?;
            let send_ok = process
                .kill_with(signal)
                .expect("signal should exist on linux");
            if !send_ok {
                for _ in 0..10 {
                    // retry a limited amount
                    let mut s = sysinfo::System::new();
                    s.refresh_process_specifics(self.pid, ProcessRefreshKind::new());
                    match s.process(self.pid) {
                        None => return Ok(None),
                        Some(_) => (),
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                panic!("cant kill :(");
            }
        }
    }
}

struct CommentOutRule {
    rule: Line,
    user: Option<String>,
}

impl InstallStep for CommentOutRule {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Commented out",
            Tense::Present => "Comment out",
            Tense::Future => "Will comment out",
            Tense::Active => "Commenting out",
        };
        format!("{verb} a cron rule that is preventing the installation")
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Commented out",
            Tense::Present => "Comment out",
            Tense::Future => "Will comment out",
            Tense::Active => "Commenting out",
        };
        format!(
            "{verb} a cron rule that is preventing the installation\n| rule:\n|\t{}",
            self.rule
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let Self { rule, user } = self;
        let mut crontab = current_crontab(user.as_deref())?;

        let commented_rule = Line {
            text: "# ".to_string() + &rule.text,
            pos: rule.pos,
        };
        for line in &mut crontab {
            if line.pos == rule.pos {
                if line.text == rule.text {
                    line.text = commented_rule.text.clone();
                } else {
                    return Err(InstallError::CrontabChanged(CrontabChanged));
                }
            }
        }

        let new_crontab: String = crontab
            .iter()
            .map(Line::text)
            .interleave_shortest(iter::repeat("\n"))
            .collect();
        set_crontab(&new_crontab, user.as_deref())?;

        Ok(Some(Box::new(RollbackCommentOut {
            commented_rule,
            original_rule: rule.clone(),
            user: user.clone(),
        })))
    }
}

struct RollbackCommentOut {
    commented_rule: Line,
    original_rule: Line,
    user: Option<String>,
}

impl RollbackStep for RollbackCommentOut {
    fn perform(&mut self) -> Result<(), RollbackError> {
        let Self {
            commented_rule,
            original_rule,
            user,
        } = self;

        let mut crontab = current_crontab(user.as_deref())?;

        for line in &mut crontab {
            if line.pos == commented_rule.pos {
                if line.text == commented_rule.text {
                    line.text = original_rule.text.clone();
                } else {
                    return Err(RollbackError::CrontabChanged(CrontabChanged));
                }
            }
        }

        let new_crontab: String = crontab
            .iter()
            .map(Line::text)
            .interleave_shortest(iter::repeat("\n"))
            .collect();
        Ok(set_crontab(&new_crontab, user.as_deref())?)
    }

    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Uncommented",
            Tense::Present => "Uncomment",
            Tense::Future => "Will uncomment",
            Tense::Active => "Uncommenting",
        };
        format!(
            "{verb} a cron rule that was commented out as it prevented the installation\n| rule:\n|\t{}",
            self.commented_rule
        )
    }
}
