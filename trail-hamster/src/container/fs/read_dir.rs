use crate::container::fs::PathBuf;
use std::ffi::OsString;
use std::fs::{FileType, Metadata};
use std::io;
use std::path::Path;

pub struct DirEntry<'a> {
    std_entry: std::fs::DirEntry,
    mount_path: &'a Path,
}

impl<'a> DirEntry<'a> {
    pub fn path(&self) -> PathBuf {
        let container_local_path = self
            .std_entry
            .path()
            .strip_prefix(self.mount_path)
            .unwrap()
            .to_path_buf();
        PathBuf {
            local_path: container_local_path,
            mount_path: self.mount_path
        }
    }
    pub fn metadata(&self) -> io::Result<Metadata> {
        self.std_entry.metadata()
    }
    pub fn file_type(&self) -> io::Result<FileType> {
        self.std_entry.file_type()
    }
    pub fn file_name(&self) -> OsString {
        self.std_entry.file_name()
    }
}

pub struct ReadDir<'a> {
    std_read_dir: std::fs::ReadDir,
    mount_path: &'a Path,
}

impl<'a> Iterator for ReadDir<'a> {
    type Item = io::Result<DirEntry<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let std_entry = self.std_read_dir.next()?;
        let entry = std_entry.map(|e| DirEntry {
            std_entry: e,
            mount_path: self.mount_path,
        });
        Some(entry)
    }
}

impl<'a> ReadDir<'a> {
    pub(super) fn new(full_path: std::path::PathBuf, mount_path: &'a Path) -> io::Result<ReadDir> {
        let read_dir = std::fs::read_dir(full_path)?;
        Ok(ReadDir {
            std_read_dir: read_dir,
            mount_path,
        })
    }
}
