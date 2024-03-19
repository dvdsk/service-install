use std::collections::hash_map::DefaultHasher;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::thread::JoinHandle;
use std::{env, thread};

use crate::podman::CommandError;

use self::fs::ContainerFs;
use self::process::Command;

use super::buildah::Buildah;
use super::podman;
use super::podman::{ContainerEngine, Podman};

mod fs;
mod process;

fn build_script_path(image: &str) -> PathBuf {
    let cwd = env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(format!("{cwd}/tests/{image}.sh"))
}

fn tag_from(image: &str) -> String {
    let mut hash_state = DefaultHasher::new();
    let path = build_script_path(image);
    std::fs::read_to_string(&path)
        .expect(&format!("build script should be at: {path:?}"))
        .hash(&mut hash_state);
    let hash = hash_state.finish();
    format!("{hash:x}")
}

fn build_image(image: &str, tag: &str) {
    // Build the images used for all tests
    Buildah::build(&build_script_path(image)).unwrap();

    // rename image <name>:<tag> to change the default tag
    // default is `latest`
    let post_build_tag = tag_from(image);
    assert_eq!(
        post_build_tag, tag,
        "image build instructions changed while building"
    );
    Buildah::rename(image, &format!("{image}:{tag}")).unwrap();
    Buildah::remove_image(image).unwrap();
}

fn image_exists(image: &str, tag: &str) -> bool {
    Podman::images()
        .unwrap()
        .into_iter()
        .inspect(|e| eprintln!("{e:?}"))
        .filter(|entry| entry.repo == "localhost/".to_owned() + image)
        .any(|entry| entry.tag == tag)
}

fn remove_containers(predicate: impl FnMut(&podman::Container) -> bool) {
    for container in Podman::containers().unwrap().into_iter().filter(predicate) {
        Podman::remove(&container.id).unwrap()
    }
}

pub struct BackgroundLineReader {
    _handle: JoinHandle<()>,
    lines: mpsc::Receiver<Result<String, io::Error>>,
}

impl BackgroundLineReader {
    pub fn new(reader: impl Read + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let reader = BufReader::new(reader);
            for line in reader.lines() {
                let err_happend = line.is_err();
                tx.send(line).unwrap();
                if err_happend {
                    return;
                }
            }
        });
        Self {
            _handle: handle,
            lines: rx,
        }
    }

    /// get any lines
    fn lines(&mut self) -> Result<Vec<String>, io::Error> {
        let mut lines = Vec::new();
        loop {
            match self.lines.try_recv() {
                Ok(Ok(line)) => lines.push(line),
                Ok(Err(e)) => return Err(e),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return Ok(lines),
            }
        }
    }
}

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Container {
    name: String,
    #[derivative(Debug = "ignore")]
    handle: Child,
    #[derivative(Debug = "ignore")]
    stderr: BackgroundLineReader,
}

#[derive(Debug)]
pub enum ContainerError {
    Engine(CommandError),
    Spawn { stderr: Vec<String> },
    Halt(std::io::Error),
}

impl Container {
    #[must_use]
    fn run_existing(image: &str, tag: &str) -> Self {
        let id: u64 = rand::random();
        let name = format!("test-{}-{id}", env!("CARGO_PKG_NAME"));
        // might be hanging around from previous run
        remove_containers(|e| e.name == name);
        let image = format!("localhost/{image}:{tag}");
        let mut handle = Podman::spawn(image, &name).unwrap();

        let stderr = handle.stderr.take().unwrap();
        let stderr = BackgroundLineReader::new(stderr);
        Self {
            name,
            handle,
            stderr,
        }
    }

    // will build the image if needed
    pub fn run(image: &str) -> Self {
        let tag = tag_from(image);
        if !image_exists(image, &tag) {
            println!("image did not already exist, building it");
            build_image(image, &tag);
        }
        Self::run_existing(image, &tag)
    }

    pub fn check(&mut self) -> Result<(), ContainerError> {
        let lines = self.stderr.lines().unwrap();
        if lines.is_empty() {
            return Ok(());
        }

        Err(ContainerError::Spawn { stderr: lines })
    }

    pub fn kill(&mut self) -> Result<(), ContainerError> {
        self.handle.kill().map_err(ContainerError::Halt)
    }

    pub fn copy_into(&mut self, source: &Path, dest: &Path) -> Result<(), ContainerError> {
        Podman::copy_into(&self.name, source, dest).map_err(ContainerError::Engine)
    }

    pub fn fs<'a>(&'a self) -> Result<ContainerFs<'a>, ContainerError> {
        let mount_path = Podman::mount(&self.name).map_err(ContainerError::Engine)?;
        Ok(ContainerFs {
            container: self,
            mount_path,
        })
    }

    pub fn command<'a, S: Into<OsString>>(&'a self, program: S) -> process::Command<'a> {
        Command::new(self, program.into())
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        // Podman::stop(&self.name).unwrap();
        // Podman::remove(&self.name).unwrap();
    }
}
