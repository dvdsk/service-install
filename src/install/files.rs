use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{Mode, Step, Tense};

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

impl Step for Move {
    fn describe(&self, tense: Tense) -> String {
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
        match tense {
            Tense::Past => format!("Moved {name} from {source} to {target}"),
            Tense::Present => format!("Moving {name} from {source} to {target}"),
            Tense::Future => format!("Will move {name} from {source} to {target}"),
        }
    }

    fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::copy(self.source, self.target)?;
        Ok(())
    }
}

pub(crate) fn move_files(source: PathBuf, mode: Mode) -> Result<Move, MoveError> {
    let dir = match mode {
        Mode::User => user_dir()?.ok_or(MoveError::UserDirNotAvailable)?,
        Mode::System => system_dir().ok_or(MoveError::SystemDirNotAvailable)?,
    };

    let name = source
        .file_name()
        .ok_or(MoveError::SourceNotFile)?
        .to_owned();
    let target = dir.join(&name);
    Ok(Move {
        name,
        source,
        target,
    })
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

impl Step for Remove {
    fn describe(&self, tense: Tense) -> String {
        todo!()
    }

    fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::remove_file(self.target)
            .map_err(DeleteError::IO)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

pub(crate) fn remove_files(installed: PathBuf) -> Remove {
    Remove { target: installed }
}

fn files_equal(a: &Path, b: &Path) -> Result<bool, std::io::Error> {
    let mut a = File::open(a)?;
    let mut b = File::open(b)?;
    let mut buf_a = [0; 40_000];
    let mut buf_b = [0; 40_000];

    loop {
        let mut a_read = a.read(&mut buf_a)?;
        let mut b_read = b.read(&mut buf_b)?;

        if a_read > b_read {
            // should be rare
            b_read += b.read(&mut buf_b[b_read..a_read])?;
        } else if a_read < b_read {
            a_read += a.read(&mut buf_a[a_read..b_read])?;
        }

        let different_size = a_read != b_read;
        let different_content = buf_a.into_iter().zip(buf_b).any(|(a, b)| a != b);

        if different_size || different_content {
            return Ok(false);
        }

        if a_read == 0 {
            assert_eq!(b_read, 0);
            return Ok(true);
        }
    }
}
