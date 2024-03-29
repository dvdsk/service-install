use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::RemoveStep;

use super::{Mode, Rollback, Step, Tense};

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
pub struct NoHomeError;

fn user_dir() -> Result<Option<PathBuf>, NoHomeError> {
    let possible_paths: &[&'static Path] = &[".local/hi bin"].map(Path::new);

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

impl Step for Move {
    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Copied",
            Tense::Present => "Copying",
            Tense::Future => "Will copy",
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
            Tense::Present => "Copying",
            Tense::Future => "Will copy",
        };
        let name = self.name.to_string_lossy();
        let target = self
            .target
            .parent()
            .expect("path points to file, so has parent")
            .display();
        format!("{verb} executable `{name}` to:\n\t{target}")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn Rollback>>, Box<dyn std::error::Error>> {
        std::fs::copy(&self.source, &self.target)?;
        Ok(Some(Box::new(Remove {
            target: self.target.clone(),
        })))
    }
}

struct SetRootOwner {
    path: PathBuf,
}

impl Step for SetRootOwner {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Set",
            Tense::Present => "Setting",
            Tense::Future => "Will set",
        };
        format!("{verb} executables owner to root")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn Rollback>>, Box<dyn std::error::Error>> {
        const ROOT: u32 = 0;
        std::os::unix::fs::chown(&self.path, Some(ROOT), Some(ROOT))?;
        Ok(None)
    }
}

struct SetReadOnly {
    path: PathBuf,
}

impl Step for SetReadOnly {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Made",
            Tense::Present => "Making",
            Tense::Future => "Will make",
        };
        format!("{verb} the executable read only")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn Rollback>>, Box<dyn std::error::Error>> {
        fs::metadata(&self.path)?.permissions().set_readonly(true);
        Ok(None)
    }
}

type Steps = Vec<Box<dyn Step>>;
pub(crate) fn move_files(source: PathBuf, mode: Mode) -> Result<(Steps, PathBuf), MoveError> {
    let dir = match mode {
        Mode::User => user_dir()?.ok_or(MoveError::UserDirNotAvailable)?,
        Mode::System => system_dir().ok_or(MoveError::SystemDirNotAvailable)?,
    };

    let name = source
        .file_name()
        .ok_or(MoveError::SourceNotFile)?
        .to_owned();
    let target = dir.join(&name);

    let mut steps = vec![
        Box::new(Move {
            name,
            source,
            target: target.clone(),
        }) as Box<dyn Step>,
        Box::new(SetReadOnly {
            path: target.clone(),
        }),
    ];
    if let Mode::System = mode {
        steps.push(Box::new(SetRootOwner {
            path: target.clone(),
        }));
    }

    Ok((steps, target))
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
    IO(std::io::Error),
    #[error("Could not get the current executable's location")]
    GetExeLocation(std::io::Error),
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
            crate::Tense::Past => "Removed",
            crate::Tense::Present => "Removing",
            crate::Tense::Future => "Will remove",
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
        format!("{verb} installed executable `{bin}` at:\n\t{dir}")
    }

    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::remove_file(&self.target)
            .map_err(DeleteError::IO)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

pub(crate) fn remove_files(installed: PathBuf) -> Remove {
    Remove { target: installed }
}