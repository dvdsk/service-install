use std::ffi::OsString;
use std::fmt::Display;
use std::fs::{self, Permissions};
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use itertools::Itertools;

use crate::install::files::process_parent::IdRes;
use crate::install::RemoveStep;

use super::init::PathCheckError;
use super::{init, InstallError, InstallStep, Mode, RemoveError, RollbackStep, Tense};

mod process_parent;

#[derive(thiserror::Error, Debug)]
pub enum MoveError {
    #[error("could not find current users home dir")]
    NoHome(#[from] #[source] NoHomeError),
    #[error("none of the usual dirs for user binaries exist")]
    UserDirNotAvailable,
    #[error("none of the usual dirs for system binaries exist")]
    SystemDirNotAvailable,
    #[error("the path did not point to a binary")]
    SourceNotFile,
    #[error("could not move binary to install location")]
    IO(#[source] std::io::Error),
    #[error("overwrite is not set and there is already a file named {name} at {}", dir.display())]
    TargetExists { name: String, dir: PathBuf },
    #[error("{0}")]
    TargetInUse(#[from] #[source] TargetInUseError),
    #[error("could not check if already existing file is read only")]
    CheckExistingFilePermissions(#[source] std::io::Error),
}

fn system_dir() -> Option<PathBuf> {
    let possible_paths: &[&'static Path] = &["/usr/bin/"].map(Path::new);

    for path in possible_paths {
        if path.parent().expect("never root").is_dir() {
            return Some(path.to_path_buf());
        }
    }
    None
}

#[derive(Debug, thiserror::Error)]
#[error("Home directory not known")]
pub struct NoHomeError;

fn user_dir() -> Result<Option<PathBuf>, NoHomeError> {
    let possible_paths: &[&'static Path] = &[".local/bin"].map(Path::new);

    for relative in possible_paths {
        let path = home::home_dir().ok_or(NoHomeError)?.join(relative);
        if path.parent().expect("never root").is_dir() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

pub(crate) struct Move {
    name: OsString,
    source: PathBuf,
    pub target: PathBuf,
}

impl InstallStep for Move {
    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Copied",
            Tense::Questioning => "Copy",
            Tense::Future => "Will copy",
            Tense::Active => "Copying",
        };
        let name = self.name.to_string_lossy();
        let source = self
            .source
            .parent()
            .expect("path points to file, so has parent")
            .display();
        let target = self
            .target
            .parent()
            .expect("path points to file, so has parent")
            .display();
        format!("{verb} executable `{name}`\n| from:\n|\t{source}\n| to:\n|\t{target}")
    }

    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Copied",
            Tense::Questioning => "Copy",
            Tense::Future => "Will copy",
            Tense::Active => "Copying",
        };
        let name = self.name.to_string_lossy();
        let target = self
            .target
            .parent()
            .expect("path points to file, so has parent")
            .display();
        format!("{verb} executable `{name}` to:\n|\t{target}")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        std::fs::copy(&self.source, &self.target).map_err(InstallError::CopyExe)?;
        Ok(Some(Box::new(Remove {
            target: self.target.clone(),
        })))
    }
}

struct SetRootOwner {
    path: PathBuf,
}

impl InstallStep for SetRootOwner {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Set",
            Tense::Active => "Setting",
            Tense::Questioning => "Set",
            Tense::Future => "Will set",
        };
        format!("{verb} executables owner to root")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        const ROOT: u32 = 0;
        std::os::unix::fs::chown(&self.path, Some(ROOT), Some(ROOT))
            .map_err(InstallError::SetRootOwner)?;
        Ok(None)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SetReadOnlyError {
    #[error("Could not get current permissions for file")]
    GetPermissions(#[source] std::io::Error),
    #[error("Could not set permissions for file")]
    SetPermissions(#[source] std::io::Error),
}

struct MakeReadExecOnly {
    path: PathBuf,
}

impl InstallStep for MakeReadExecOnly {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Made",
            Tense::Questioning => "Make",
            Tense::Future => "Will make",
            Tense::Active => "Making",
        };
        format!("{verb} the executable read and execute only")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        use std::os::unix::fs::PermissionsExt;

        let org_permissions = fs::metadata(&self.path)
            .map_err(SetReadOnlyError::GetPermissions)?
            .permissions();
        let mut permissions = org_permissions.clone();
        permissions.set_mode(555);
        fs::set_permissions(&self.path, permissions).map_err(SetReadOnlyError::SetPermissions)?;
        Ok(Some(Box::new(RestorePermissions {
            path: self.path.clone(),
            org_permissions,
        })))
    }
}

struct RestorePermissions {
    path: PathBuf,
    org_permissions: Permissions,
}

impl RollbackStep for RestorePermissions {
    fn perform(&mut self) -> Result<(), super::RollbackError> {
        fs::set_permissions(&self.path, self.org_permissions.clone())
            .map_err(super::RollbackError::RestoringPermissions)
    }

    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Restored",
            Tense::Active => "Restoring",
            Tense::Questioning => "Restore",
            Tense::Future => "Will Restore",
        };
        format!("{verb} executables previous permissions")
    }
}

type Steps = Vec<Box<dyn InstallStep>>;
pub(crate) fn move_files(
    source: PathBuf,
    mode: Mode,
    run_as: Option<&str>,
    overwrite_existing: bool,
    init_systems: &[init::System],
) -> Result<(Steps, PathBuf), MoveError> {
    let dir = match mode {
        Mode::User => user_dir()?.ok_or(MoveError::UserDirNotAvailable)?,
        Mode::System => system_dir().ok_or(MoveError::SystemDirNotAvailable)?,
    };

    let file_name = source
        .file_name()
        .ok_or(MoveError::SourceNotFile)?
        .to_owned();
    let target = dir.join(&file_name);

    if target.is_file() && !overwrite_existing {
        return Err(MoveError::TargetExists {
            name: file_name.to_string_lossy().to_string(),
            dir,
        });
    }

    let mut steps = Vec::new();
    if let Some(make_removable) = make_removable_if_needed(&target)? {
        steps.push(make_removable);
    }

    let disable_steps = disable_if_running(&target, init_systems, mode, run_as)?;
    steps.extend(disable_steps);

    steps.extend([
        Box::new(Move {
            name: file_name,
            source,
            target: target.clone(),
        }) as Box<dyn InstallStep>,
        Box::new(MakeReadExecOnly {
            path: target.clone(),
        }),
    ]);
    if let Mode::System = mode {
        steps.push(Box::new(SetRootOwner {
            path: target.clone(),
        }));
    }

    Ok((steps, target))
}

struct MakeRemovable(PathBuf);

fn make_removable_if_needed(target: &Path) -> Result<Option<Box<dyn InstallStep>>, MoveError> {
    let permissions = match fs::metadata(target) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(MoveError::CheckExistingFilePermissions(e)),
    }
    .permissions();

    Ok(if permissions.readonly() {
        let step = MakeRemovable(target.to_owned());
        let step = Box::new(step) as Box<dyn InstallStep>;
        Some(step)
    } else {
        None
    })
}

impl InstallStep for MakeRemovable {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Made",
            Tense::Questioning => "Make",
            Tense::Future => "Will make",
            Tense::Active => "Making",
        };
        format!("{verb} the file taking up the install location removable")
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Made",
            Tense::Questioning => "Make",
            Tense::Future => "Will make",
            Tense::Active => "Making",
        };
        format!("A read only file is taking up the install location. {verb} it removable by making it writable\n| file:\n|\t{}", self.0.display())
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let org_permissions = fs::metadata(&self.0)
            .map_err(SetReadOnlyError::GetPermissions)?
            .permissions();
        let mut permissions = org_permissions.clone();
        permissions.set_mode(0o600);
        fs::set_permissions(&self.0, permissions).map_err(SetReadOnlyError::SetPermissions)?;
        Ok(Some(Box::new(RestorePermissions {
            path: self.0.clone(),
            org_permissions,
        })))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TargetInUseError {
    NoParent,
    ResolvePath(#[from] #[source] PathCheckError),
    Parents(Vec<PathBuf>),
    CouldNotDisable(#[from] #[source] DisableError),
}

#[derive(Debug, thiserror::Error)]
pub enum DisableError {
    #[error(transparent)]
    SystemD(#[from] init::systemd::DisableError),
    #[error(transparent)]
    Cron(#[from] init::cron::disable::Error),
}

impl Display for TargetInUseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetInUseError::NoParent => {
                writeln!(f, "There is already a file at the install location. It can not be replaced as it is running. We have no information on how it was started as it has no parent")
            }
            TargetInUseError::ResolvePath(_) => {
                writeln!(f, "There is already a file at the install location. It can not be replaced as it is running. While it has a parent we failed to get information about it")
            }
            TargetInUseError::Parents(tree) => {
                let tree = tree.iter().map(|p| p.display().to_string());
                let tree: String = Itertools::intersperse(tree, " -> ".to_string()).collect();
                writeln!(f, "There is already a file at the install location. It can not be replaced as it is running.\n\tThe process tree that started that:\n\t`{tree}`\nIn this tree the arrow means left started the right process")
            }
            TargetInUseError::CouldNotDisable(err) => {
                writeln!(
                    f,
                    "The file we need to replace is in use by a running service however we could not disable that service. {err}"
                )
            }
        }
    }
}

fn disable_if_running(
    target: &Path,
    init_systems: &[init::System],
    mode: Mode,
    run_as: Option<&str>,
) -> Result<Vec<Box<dyn InstallStep>>, TargetInUseError> {
    let mut steps = Vec::new();

    for parent_info in process_parent::list(target, init_systems)? {
        match parent_info {
            IdRes::ParentIsInit { init, pid } => {
                steps.append(&mut init.disable_steps(target, pid, mode, run_as)?)
            }
            IdRes::NoParent => return Err(TargetInUseError::NoParent)?,
            IdRes::ParentNotInit { parents } => steps.push(process_parent::notify_steps(parents)),
        }
    }

    Ok(steps)
}

#[derive(thiserror::Error, Debug)]
pub enum DeleteError {
    #[error("could not find current users home dir")]
    NoHome(#[from] #[source] NoHomeError),
    #[error("none of the usual dirs for user binaries exist")]
    UserDirNotAvailable,
    #[error("none of the usual dirs for system binaries exist")]
    SystemDirNotAvailable,
    #[error("the path did not point to a binary")]
    SourceNotFile,
    #[error("could not move binary to install location")]
    IO(#[source] std::io::Error),
    #[error("Could not get the current executable's location")]
    GetExeLocation(#[source] std::io::Error),
    #[error("May only uninstall the currently running binary, running: {running} installed: {installed}")]
    ExeNotInstalled {
        running: PathBuf,
        installed: PathBuf,
    },
}

pub(crate) struct Remove {
    target: PathBuf,
}

impl RemoveStep for Remove {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Questioning => "Remove",
            Tense::Future => "Will remove",
            Tense::Active => "Removing",
        };
        let bin = self
            .target
            .file_name()
            .expect("In fn exe_path we made sure target is a file")
            .to_string_lossy();
        format!("{verb} installed executable `{bin}`")
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Questioning => "Remove",
            Tense::Future => "Will remove",
            Tense::Active => "Removing",
        };
        let bin = self
            .target
            .file_name()
            .expect("In fn exe_path we made sure target is a file")
            .to_string_lossy();
        let dir = self
            .target
            .parent()
            .expect("There is always a parent on linux")
            .display();
        format!("{verb} installed executable `{bin}` at:\n|\t{dir}")
    }

    fn perform(&mut self) -> Result<(), RemoveError> {
        std::fs::remove_file(&self.target)
            .map_err(DeleteError::IO)
            .map_err(Into::into)
    }
}

pub(crate) fn remove_files(installed: PathBuf) -> Remove {
    Remove { target: installed }
}
