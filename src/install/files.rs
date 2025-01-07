use std::env::current_exe;
use std::ffi::OsString;
use std::fmt::Display;
use std::fs::{self, Permissions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use itertools::Itertools;

use crate::install::files::process_parent::IdRes;
use crate::install::RemoveStep;

use super::init::PathCheckError;
use super::{
    init, BackupError, InstallError, InstallStep, Mode, RemoveError, RollbackError, RollbackStep,
    Tense,
};

pub mod process_parent;

#[derive(thiserror::Error, Debug)]
pub enum MoveError {
    #[error("could not find current users home dir")]
    NoHome(
        #[from]
        #[source]
        NoHomeError,
    ),
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
    TargetInUse(
        #[from]
        #[source]
        TargetInUseError,
    ),
    #[error("could not check if already existing file is read only")]
    CheckExistingFilePermissions(#[source] std::io::Error),
    #[error("could not check if we are running from the target location")]
    ResolveCurrentExe(#[source] std::io::Error),
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
        format!(
            "{verb} executable `{name}`{}\n| from:\n|\t{source}\n| to:\n|\t{target}",
            tense.punct()
        )
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
        format!(
            "{verb} executable `{name}` to:\n|\t{target}{}",
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let rollback_step = if self.target.is_file() {
            let target_content = fs::read(&self.target)
                .map_err(BackupError::Read)
                .map_err(InstallError::Backup)?;

            let mut backup = tempfile::tempfile()
                .map_err(BackupError::Create)
                .map_err(InstallError::Backup)?;
            backup
                .write_all(&target_content)
                .map_err(BackupError::Write)
                .map_err(InstallError::Backup)?;

            Box::new(MoveBack {
                backup,
                target: self.target.clone(),
            }) as Box<dyn RollbackStep>
        } else {
            Box::new(Remove {
                target: self.target.clone(),
            }) as Box<dyn RollbackStep>
        };

        match std::fs::copy(&self.source, &self.target) {
            Err(e) => Err(InstallError::CopyExeError(e)),
            Ok(_) => Ok(Some(rollback_step)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MoveBackError {
    #[error("Could not read backup from file")]
    ReadingBackup(#[source] std::io::Error),
    #[error("Could not write to target")]
    WritingToTarget(#[source] std::io::Error),
}

struct MoveBack {
    /// created by tempfile will be auto cleaned by OS when
    /// this drops
    backup: std::fs::File,
    target: PathBuf,
}

impl RollbackStep for MoveBack {
    fn perform(&mut self) -> Result<(), RollbackError> {
        let mut buf = Vec::new();
        self.backup
            .read_to_end(&mut buf)
            .map_err(MoveBackError::ReadingBackup)
            .map_err(RollbackError::MovingBack)?;
        fs::write(&self.target, buf)
            .map_err(MoveBackError::WritingToTarget)
            .map_err(RollbackError::MovingBack)
    }

    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Moved",
            Tense::Questioning => "Move",
            Tense::Active => "Moving",
            Tense::Future => "Will move",
        };
        format!(
            "{verb} back the file that was origonally at the install location{}",
            tense.punct()
        )
    }
}

struct SetRootOwner {
    path: PathBuf,
}

impl InstallStep for SetRootOwner {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past | Tense::Questioning => "Set",
            Tense::Active => "Setting",
            Tense::Future => "Will set",
        };
        format!("{verb} executables owner to root{}", tense.punct())
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
        format!(
            "{verb} the executable read and execute only{}",
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        use std::os::unix::fs::PermissionsExt;

        let org_permissions = fs::metadata(&self.path)
            .map_err(SetReadOnlyError::GetPermissions)?
            .permissions();
        let mut permissions = org_permissions.clone();
        permissions.set_mode(0o555);
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
    fn perform(&mut self) -> Result<(), RollbackError> {
        match fs::set_permissions(&self.path, self.org_permissions.clone()) {
            Ok(()) => Ok(()),
            // overwrite may have been set or the file removed by the user
            // we should no abort the rollback because the file is not there
            Err(io) if io.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("Could not restore permissions, file is not there");
                Ok(())
            }
            Err(other) => Err(RollbackError::RestoringPermissions(other)),
        }
    }

    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Restored",
            Tense::Active => "Restoring",
            Tense::Questioning => "Restore",
            Tense::Future => "Will Restore",
        };
        format!("{verb} executables previous permissions{}", tense.punct())
    }
}

struct FilesAlreadyInstalled {
    target: PathBuf,
}

impl InstallStep for FilesAlreadyInstalled {
    fn describe(&self, tense: Tense) -> String {
        match tense {
            Tense::Past => "this binary was already installed in the target location",
            Tense::Questioning | Tense::Future | Tense::Active => {
                "this binary is already installed in the target location"
            }
        }
        .to_owned()
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        Ok(None)
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        format!(
            "{}\n\t-target location: {}",
            self.describe(tense),
            self.target.display()
        )
    }

    fn options(&self) -> Option<super::StepOptions> {
        None // this is a notification
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

    if target.is_file() && target == current_exe().map_err(MoveError::ResolveCurrentExe)? {
        let step = FilesAlreadyInstalled {
            target: target.clone(),
        };
        return Ok((vec![Box::new(step) as Box<dyn InstallStep>], target));
    }

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
        format!(
            "{verb} the file taking up the install location removable{}",
            tense.punct()
        )
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Made",
            Tense::Questioning => "Make",
            Tense::Future => "Will make",
            Tense::Active => "Making",
        };
        format!("A read only file is taking up the install location. {verb} it removable by making it writable{}\n| file:\n|\t{}", tense.punct(), self.0.display())
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
    ResolvePath(
        #[from]
        #[source]
        PathCheckError,
    ),
    Parents(Vec<PathBuf>),
    CouldNotDisable(
        #[from]
        #[source]
        DisableError,
    ),
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
                steps.append(&mut init.disable_steps(target, pid, mode, run_as)?);
            }
            IdRes::NoParent => return Err(TargetInUseError::NoParent)?,
            IdRes::ParentNotInit { parents, pid } => {
                steps.push(process_parent::kill_old_steps(pid, parents));
            }
        }
    }

    Ok(steps)
}

#[derive(thiserror::Error, Debug)]
pub enum DeleteError {
    #[error("could not find current users home dir")]
    NoHome(
        #[from]
        #[source]
        NoHomeError,
    ),
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
        format!("{verb} installed executable `{bin}`{}", tense.punct())
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
        format!(
            "{verb} installed executable `{bin}`{} Is installed at:\n|\t{dir}",
            tense.punct()
        )
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
