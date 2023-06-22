use std::path::Path as StdPath;
use std::path::PathBuf as StdPathBuf;

pub struct PathBuf<'a> {
    pub(crate) local_path: StdPathBuf,
    pub(crate) mount_path: &'a StdPath,
}

impl<'a> PathBuf<'a> {
    pub fn into_incorrect_std_path(self) -> StdPathBuf {
        self.local_path
    }

    pub fn into_std_path(self) -> StdPathBuf {
        self.mount_path.join(self.local_path)
    }

    pub fn as_incorrect_std_path(&self) -> &StdPath {
        self.local_path.as_path()
    }
}
