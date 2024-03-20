use std::path::{Path, PathBuf};

mod builder;
mod systemd;
use systemd::Systemd;

pub use builder::Install;

use self::builder::{ToAssign, Trigger};

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    User,
    System,
}

#[derive(Debug, Clone)]
pub struct InitParams {
    name: String,
    description: Option<String>,

    exe_path: PathBuf,
    exe_args: Vec<String>,
    working_dir: Option<PathBuf>,

    trigger: Trigger,
    mode: Mode,
}

impl InitParams {
    fn description(&self) -> String {
        self.description
            .clone()
            .unwrap_or_else(|| format!("starts {}", self.name))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InitSetupError {
    #[error("systemd specific error")]
    Systemd(#[from] systemd::Error),
    #[error("could not find current users home dir")]
    NoHome,
}

#[derive(thiserror::Error, Debug)]
pub enum InitTearDownError {
    #[error("systemd specific error")]
    Systemd(#[from] systemd::Error),
    #[error("could not find current users home dir")]
    NoHome,
}

#[derive(thiserror::Error, Debug)]
pub enum InstallError {
    #[error("Error setting up init: {0}")]
    Init(#[from] InitSetupError),
    #[error("Failed to move files: {0}")]
    Move(#[from] MoveError),
}

#[derive(thiserror::Error, Debug)]
pub enum RemoveError {
    #[error("Could not find this executable's location")]
    GetExeLocation(std::io::Error),
    #[error("Failed to remove files: {0}")]
    Move(#[from] DeleteError),
}

trait InitSystem {
    fn name(&self) -> &'static str;
    fn set_up(&self, params: &InitParams) -> Result<(), InitSetupError>;
    fn tear_down(&self, name: &str) -> Result<(), InitTearDownError>;
}

struct Cron;

impl InitSystem for Cron {
    fn name(&self) -> &'static str {
        "cron"
    }
    fn set_up(&self, _params: &InitParams) -> Result<(), InitSetupError> {
        todo!()
    }
    fn tear_down(&self, _params: &str) -> Result<(), InitTearDownError> {
        todo!()
    }
}

const INIT_SYSTEMS: [&dyn InitSystem; 2] = [&Systemd {}, &Cron {}];

impl Install<builder::Set, builder::Set, builder::Set> {
    pub fn install(self) -> Result<(), InstallError> {
        let builder::Install {
            mode,
            path: Some(source),
            name: Some(name),
            args,
            trigger: Some(trigger),
            working_dir,
            description,
            ..
        } = self
        else {
            unreachable!("type sys guarantees path, name and trigger set")
        };

        let path = move_files(&source, mode)?;

        let params = InitParams {
            name,
            description,

            exe_path: path,
            exe_args: args,
            working_dir,

            trigger,
            mode,
        };

        for init in INIT_SYSTEMS {
            let Err(error) = init.set_up(&params) else {
                return Ok(());
            };
            tracing::info!("Could set up init using {}, error: {error}", init.name())
        }

        Ok(())
    }
}

impl<T: ToAssign, P: ToAssign> Install<P, builder::Set, T> {
    pub fn remove(self) -> Result<(), RemoveError> {
        let builder::Install {
            mode,
            path,
            name: Some(name),
            working_dir,
            description,
            ..
        } = self
        else {
            unreachable!("type sys guarantees path and name set")
        };

        let source = std::env::current_exe().map_err(RemoveError::GetExeLocation)?;
        let path = remove_files(&source, mode)?;

        for init in INIT_SYSTEMS {
            init.tear_down(&name);
        }

        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MoveError {
    #[error("could not find current users home dir")]
    NoHome(#[from] NoHomeError),
    #[error("none of the usual dirs for user binaries exist")]
    UserDirNotAvailable,
    #[error("none of the usual dirs for system binaries exist")]
    SystemDirNotAvailable,
    #[error("the path did not point to a binary")]
    SourceNotFile,
    #[error("could not move binary to install location: {0}")]
    IO(#[from] std::io::Error),
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
struct NoHomeError;

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

fn move_files(source: &Path, mode: Mode) -> Result<PathBuf, MoveError> {
    let dir = match mode {
        Mode::User => user_dir()?.ok_or(MoveError::UserDirNotAvailable)?,
        Mode::System => system_dir().ok_or(MoveError::SystemDirNotAvailable)?,
    };

    let name = source.file_name().ok_or(MoveError::SourceNotFile)?;
    let target = dir.join(name);
    std::fs::copy(source, &target)?;
    Ok(target)
}

#[derive(thiserror::Error, Debug)]
pub enum DeleteError {
    #[error("could not find current users home dir")]
    NoHome(#[from] NoHomeError),
    #[error("none of the usual dirs for user binaries exist")]
    UserDirNotAvailable,
    #[error("none of the usual dirs for system binaries exist")]
    SystemDirNotAvailable,
    #[error("the path did not point to a binary")]
    SourceNotFile,
    #[error("could not move binary to install location: {0}")]
    IO(#[from] std::io::Error),
}

fn remove_files(source: &Path, mode: Mode) -> Result<PathBuf, DeleteError> {
    let dir = match mode {
        Mode::User => user_dir()?.ok_or(DeleteError::UserDirNotAvailable)?,
        Mode::System => system_dir().ok_or(DeleteError::SystemDirNotAvailable)?,
    };

    let name = source.file_name().ok_or(DeleteError::SourceNotFile)?;
    let target = dir.join(name);
    std::fs::copy(source, &target)?;
    Ok(target)
}
