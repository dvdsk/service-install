use std::path::{Path, PathBuf};
use std::process::Command;

use itertools::Itertools;
use sysinfo::Pid;

use crate::install::init::PathCheckError;
use crate::install::{init, InstallStep};

#[derive(Debug)]
pub(crate) enum IdRes {
    /// Process locking up the file has no parent, must be orphaned
    NoParent,
    ParentIsInit {
        init: init::System,
        pid: Pid,
    },
    ParentNotInit {
        parents: Vec<PathBuf>,
        pid: Pid,
    },
}

impl IdRes {
    fn from_tree_and_pid(
        tree: Vec<&Path>,
        pid: Pid,
        init_systems: &[init::System],
    ) -> Result<IdRes, PathCheckError> {
        let Some(direct_parent) = tree.first() else {
            return Ok(IdRes::NoParent);
        };

        for init in init_systems {
            if init.is_init_path(direct_parent)? {
                return Ok(IdRes::ParentIsInit {
                    init: init.clone(),
                    pid,
                });
            }
        }

        Ok(IdRes::ParentNotInit {
            pid,
            parents: tree.into_iter().map(PathBuf::from).collect(),
        })
    }
}

pub(crate) fn list(
    target: &Path,
    init_systems: &[init::System],
) -> Result<Vec<IdRes>, PathCheckError> {
    use sysinfo::{ProcessRefreshKind, System, UpdateKind};

    let mut s = System::new();
    s.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always),
    );

    let using_target: Vec<_> = s
        .processes()
        .iter()
        .map(|(_, process)| process)
        .filter(|p| p.exe() == Some(target))
        .collect();

    let without_children = using_target.iter().filter(|p| {
        if let Some(parent) = p.parent() {
            !using_target.iter().any(|p| p.pid() == parent)
        } else {
            true
        }
    });

    without_children
        .copied()
        .map(|p| {
            let mut process = p;
            let mut tree = Vec::new();

            while let Some(parent) = process.parent() {
                if let Some(parent) = s.process(parent) {
                    process = parent;
                    if let Some(exe) = process.exe() {
                        tree.push(exe);
                    } else {
                        let cmd = process.cmd();
                        let path = Path::new(&cmd[0]);
                        tree.push(path);
                    }
                }
            }
            (tree, p.pid())
        })
        .map(|(tree, pid)| IdRes::from_tree_and_pid(tree, pid, init_systems))
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum KillOldError {
    #[error("Could not run the kill command")]
    KillUnavailable(#[source] std::io::Error),
    #[error("The kill command faild with: {0}")]
    KillFailed(String),
}

pub struct KillOld {
    pid: Pid,
    parents: Vec<PathBuf>,
}

impl InstallStep for KillOld {
    fn describe(&self, tense: crate::Tense) -> String {
        match tense {
            crate::Tense::Past => {
                "there was a program running with the same name taking up the \
                    install location it has been terminated"
            }
            crate::Tense::Questioning => {
                "there is a program running with the same name taking up the \
                    install location terminate it?"
            }
            crate::Tense::Active => {
                "there is a program running with the same name taking up the \
                    install location, terminating it"
            }
            crate::Tense::Future => {
                "there is a program running with the same name taking up the \
                    install location, it will be terminated"
            }
        }
        .to_string()
    }

    fn perform(
        &mut self,
    ) -> Result<Option<Box<dyn crate::install::RollbackStep>>, crate::install::InstallError> {
        let output = Command::new("kill")
            .arg("--signal")
            .arg("TERM")
            .arg(format!("{}", self.pid))
            .output()
            .map_err(KillOldError::KillUnavailable)
            .map_err(crate::install::InstallError::KillOld)?;

        if output.status.success() {
            Ok(None)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(crate::install::InstallError::KillOld(
                KillOldError::KillFailed(stderr),
            ))
        }
    }

    fn describe_detailed(&self, tense: crate::Tense) -> String {
        let list = if self.parents.len() == 1 {
            self.parents
                .first()
                .expect("len just checked")
                .display()
                .to_string()
        } else {
            self.parents
                .iter()
                .map(|p| p.display().to_string())
                .join("\n\twhich was started by: ")
        };

        match tense {
            crate::Tense::Past => format!(
                "there was a program running with the same name taking up the \
            install location. It was was started by: {list}\nIt had to be terminated \
            before we could continue."
            ),
            crate::Tense::Questioning => format!(
                "there is a program running with the same name taking up the \
            install location. It was was started by: {list}\nIt must be terminated \
            before we can continue. Terminating might not work or the parent \
            can restart the program. Do you wish to try to stop the program and \
            continue installation?"
            ),
            crate::Tense::Active => format!(
                "there is a program running with the same name taking up the \
            install location. It was was started by: {list}\nIt must be terminated \
            before we can continue. Terminating might not work or the parent \
            can restart the program. Stopping the program and continuing installation"
            ),
            crate::Tense::Future => format!(
                "there is a program running with the same name taking up the \
            install location. It was was started by: {list}\nIt must be terminated \
            before we can continue. Terminating might not work or the parent \
            can restart the program. Will try to stop the program and continuing \
            installation"
            ),
        }
    }
}

pub(crate) fn kill_old_steps(pid: Pid, parents: Vec<PathBuf>) -> Box<dyn InstallStep> {
    Box::new(KillOld { pid, parents })
}
