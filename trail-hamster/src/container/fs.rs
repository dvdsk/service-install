use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::path::PathBuf as StdPathBuf;

use crate::Container;

mod read_dir;
use read_dir::ReadDir;
mod path;
use path::PathBuf;

#[derive(Debug)]
pub struct ContainerFs<'a> {
    pub container: &'a Container,
    pub mount_path: StdPathBuf,
}

impl<'a> ContainerFs<'a> {
    fn local_path<P: AsRef<Path>>(&self, path: P) -> StdPathBuf {
        let path = self.mount_path.join(path.as_ref());
        assert!(path.starts_with(&self.mount_path));
        path
    }
    pub fn create_file<P: AsRef<Path>>(&'a self, path: P) -> io::Result<ContainerFile<'a>> {
        self.open_options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }
    pub fn open_file<P: AsRef<Path>>(&'a self, path: P) -> io::Result<ContainerFile<'a>> {
        self.open_options().read(true).open(path)
    }
    pub fn open_options(&'a self) -> OpenOptions<'a> {
        OpenOptions {
            fs: self,
            append: false,
            create: false,
            create_new: false,
            read: false,
            truncate: false,
            write: false,
        }
    }
    pub fn read<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.open_file(path)?.read_to_end(&mut buf)?;
        Ok(buf)
    }
    pub fn read_to_string<P: AsRef<Path>>(&self, path: P) -> io::Result<String> {
        let mut buf = String::new();
        self.open_file(path)?.read_to_string(&mut buf)?;
        Ok(buf)
    }
    pub fn read_dir<P: AsRef<Path>>(&self, path: P) -> io::Result<ReadDir> {
        let path = self.local_path(path);
        ReadDir::new(path, &self.mount_path)
    }
    pub fn metadata<P: AsRef<Path>>(&self, path: P) -> io::Result<std::fs::Metadata> {
        let path = self.local_path(path);
        std::fs::metadata(path)
    }
}

#[derive(Debug)]
pub struct OpenOptions<'a> {
    fs: &'a ContainerFs<'a>,
    append: bool,
    create: bool,
    create_new: bool,
    read: bool,
    truncate: bool,
    write: bool,
}

macro_rules! open_option {
    ($property:ident) => {
        pub fn $property(mut self, $property: bool) -> Self {
            self.$property = $property;
            self
        }
    };
}

impl<'a> OpenOptions<'a> {
    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<ContainerFile<'a>> {
        use std::fs::OpenOptions as StdOptenOptions;
        let path = self.fs.local_path(path);
        let handle = StdOptenOptions::new()
            .append(self.append)
            .create(self.create)
            .create_new(self.create_new)
            .read(self.read)
            .truncate(self.truncate)
            .write(self.write)
            .open(path)?;

        Ok(ContainerFile {
            _fs: self.fs,
            handle,
        })
    }

    open_option!(append);
    open_option!(create);
    open_option!(create_new);
    open_option!(read);
    open_option!(truncate);
    open_option!(write);
}

pub struct ContainerFile<'a> {
    _fs: &'a ContainerFs<'a>,
    handle: File,
}

impl<'a> Write for ContainerFile<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.handle.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.handle.flush()
    }
}

impl<'a> Read for ContainerFile<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.handle.read(buf)
    }
}
