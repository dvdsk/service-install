use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::install::init::{extract_path, COMMENT_PREAMBLE, COMMENT_SUFFIX};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unit {
    body: String,
    pub(crate) path: PathBuf,
    pub(crate) file_name: OsString,
}

/// The executables location could not be found. It is needed to safely
/// uninstall.
#[derive(Debug, thiserror::Error)]
pub enum FindExeError {
    #[error("Could not read systemd unit file at: {path}")]
    ReadingUnit { #[source] err: std::io::Error, path: PathBuf },
    #[error("ExecStart (use to find binary) is missing from servic unit at: {0}")]
    ExecLineMissing(PathBuf),
    #[error("Path to binary extracted from systemd unit does not lead to a file, path: {0}")]
    ExecPathNotFile(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("File has no file name, can not be a systemd unit")]
    NoName,
    #[error("Could not read unit's content: {0}")]
    FailedToRead(#[from] #[source] std::io::Error),
}

impl Unit {
    pub(crate) fn from_path(path: PathBuf) -> Result<Self, Error> {
        Ok(Self {
            body: std::fs::read_to_string(&path)?,
            file_name: path.file_name().ok_or(Error::NoName)?.to_os_string(),
            path,
        })
    }

    pub(crate) fn exe_path(&self) -> Result<PathBuf, FindExeError> {
        let exe_path = self
            .body
            .lines()
            .map(str::trim)
            .find_map(|l| l.strip_prefix("ExecStart="))
            .map(extract_path::split_unescaped_whitespace_once)
            .ok_or(FindExeError::ExecLineMissing(self.path.clone()))?;
        let exe_path = Path::new(&exe_path).to_path_buf();
        if exe_path.is_file() {
            Ok(exe_path)
        } else {
            Err(FindExeError::ExecPathNotFile(exe_path))
        }
    }

    pub(crate) fn our_service(&self) -> bool {
        self.body.contains(COMMENT_PREAMBLE) && self.body.contains(COMMENT_SUFFIX)
    }

    pub(crate) fn has_install(&self) -> bool {
        self.body.contains("[Install]")
    }

    pub(crate) fn name(&self) -> OsString {
        self.path
            .with_extension("")
            .file_name()
            .expect("Checked in Unit::from_path")
            .to_os_string()
    }
}
