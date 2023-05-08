use std::path::{Path, PathBuf};

mod builder;
mod systemd;
use systemd::Systemd;

pub use builder::Install;

use self::builder::Trigger;

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
pub enum InitError {
    #[error("systemd specific error")]
    Systemd(#[from] systemd::Error),
    #[error("could not find current users home dir")]
    NoHome,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error setting up init: {0}")]
    Init(#[from] InitError),
    #[error("Failed to move files: {0}")]
    Move(#[from] MoveError),
}

trait InitSystem {
    fn name(&self) -> &'static str;
    fn set_up(&self, params: &InitParams) -> Result<(), InitError>;
}

struct Cron;

impl InitSystem for Cron {
    fn name(&self) -> &'static str {
        "cron"
    }
    fn set_up(&self, _params: &InitParams) -> Result<(), InitError> {
        todo!()
    }
}

const INIT_SYSTEMS: [&dyn InitSystem; 2] = [&Systemd {}, &Cron {}];

impl Install<builder::Set, builder::Set, builder::Set> {
    pub fn perform(self) -> Result<(), Error> {
        let builder::Install {
            mode,
            path: Some(source),
            name: Some(name),
            args,
            trigger: Some(trigger),
            working_dir,
            description,
            ..
        } = self else {
            unreachable!()
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

#[derive(thiserror::Error, Debug)]
pub enum MoveError {
    #[error("could not find current users home dir")]
    NoHome,
    #[error("none of the usual dirs for user binaries exist")]
    UserDirNotAvailible,
    #[error("none of the usual dirs for system binaries exist")]
    SystemDirNotAvailible,
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

fn user_dir() -> Result<Option<PathBuf>, MoveError> {
    let possible_paths: &[&'static Path] = &[".local/bin"].map(Path::new);

    for relative in possible_paths {
        let path = home::home_dir().ok_or(MoveError::NoHome)?.join(relative);
        if path.parent().expect("never root").is_dir() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn move_files(source: &Path, mode: Mode) -> Result<PathBuf, MoveError> {
    let dir = match mode {
        Mode::User => user_dir()?.ok_or(MoveError::UserDirNotAvailible)?,
        Mode::System => system_dir().ok_or(MoveError::SystemDirNotAvailible)?,
    };

    let name = source.file_name().ok_or(MoveError::SourceNotFile)?;
    let target = dir.join(name);
    std::fs::copy(source, &target)?;
    Ok(target)
}
