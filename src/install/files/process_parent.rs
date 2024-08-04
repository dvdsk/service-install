use std::path::{Path, PathBuf};

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
    },
}

impl IdRes {
    fn from_tree_and_pid(
        tree: Vec<&Path>,
        pid: Pid,
        init_systems: &[init::System],
    ) -> Result<IdRes, PathCheckError> {
        if tree.is_empty() {
            return Ok(IdRes::NoParent);
        }

        for parent in &tree {
            for init in init_systems {
                if init.is_init_path(parent)? {
                    return Ok(IdRes::ParentIsInit {
                        init: init.clone(),
                        pid,
                    });
                }
            }
        }

        Ok(IdRes::ParentNotInit {
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
        ProcessRefreshKind::new()
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
        .cloned()
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

struct NotifyStep {
    parents: Vec<PathBuf>,
}

impl InstallStep for NotifyStep {
    fn describe(&self, tense: crate::Tense) -> String {
        match tense {
            crate::Tense::Past => "the executable replaced was running",
            crate::Tense::Questioning => "the executable is running",
            crate::Tense::Active | crate::Tense::Future => {
                "the to be replaced executable is running"
            }
        }
        .to_string()
    }

    fn perform(
        &mut self,
    ) -> Result<Option<Box<dyn crate::install::RollbackStep>>, crate::install::InstallError> {
        Ok(None)
    }

    fn describe_detailed(&self, tense: crate::Tense) -> String {
        let start = match tense {
            crate::Tense::Past => "the executable replaced was running",
            crate::Tense::Questioning => "the executable is running",
            crate::Tense::Active | crate::Tense::Future => {
                "the to be replaced executable is running"
            }
        };

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
                .join("\n\t-")
        };
        format!("{start}, it was started by {list}")
    }
}

pub(crate) fn notify_steps(parents: Vec<PathBuf>) -> Box<dyn InstallStep> {
    Box::new(NotifyStep { parents })
}
