use std::path::{Path, PathBuf};

use sysinfo::Pid;

use crate::install::init;
use crate::install::init::PathCheckError;

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
    NotRunning,
}

pub(crate) fn check(target: &Path, init_systems: &[init::System]) -> Result<IdRes, PathCheckError> {
    use sysinfo::{ProcessRefreshKind, System, UpdateKind};

    let mut s = System::new();
    s.refresh_processes_specifics(
        ProcessRefreshKind::new()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always),
    );
    let Some(to_kill) = s
        .processes()
        .iter()
        .map(|(_, process)| process)
        .find(|p| p.exe() == Some(target))
    else {
        return Ok(IdRes::NotRunning);
    };

    let mut process = to_kill;
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

    if tree.is_empty() {
        return Ok(IdRes::NoParent);
    }

    for parent in &tree {
        for init in init_systems {
            if init.is_init_path(parent)? {
                return Ok(IdRes::ParentIsInit {
                    init: init.clone(),
                    pid: to_kill.pid(),
                });
            }
        }
    }

    Ok(IdRes::ParentNotInit {
        parents: tree.into_iter().map(PathBuf::from).collect(),
    })
}
