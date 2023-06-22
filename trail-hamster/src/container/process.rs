use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::process::Output;
use std::process::Stdio;

use std::process::Child as StdChild;
use std::process::Command as StdCommand;

use shell_escape::escape;

use crate::Container;

// use crate::podman::{ContainerEngine, Podman};

pub struct Command<'a> {
    container: &'a Container,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
    stdin: Option<Stdio>,
    program: OsString,
    working_dir: Option<PathBuf>,
    args: Vec<String>,
}

pub struct Child {
    child: StdChild,
}

impl<'a> Command<'a> {
    pub fn arg(&'a mut self, arg: &str) -> &mut Command {
        let escaped = escape(arg.into()).into_owned();
        self.args.push(escaped);
        self
    }
    pub fn args<S, I>(&'a mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let escaped = args.into_iter().map(|a| {
            let a = a.as_ref();
            let a = escape(a.into());
            a.into_owned()
        });
        self.args.extend(escaped);
        self
    }
    pub fn current_dir<P: AsRef<Path>>(&'a mut self, dir: P) -> &mut Command {
        self.working_dir = Some(dir.as_ref().to_owned());
        self
    }

    pub fn env() {
        todo!()
    }
    pub fn env_clear() {
        todo!()
    }
    pub fn env_remove() {
        todo!()
    }
    pub fn envs() {
        todo!()
    }
    pub fn get_args() {
        todo!()
    }
    pub fn get_current_dir() {
        todo!()
    }
    pub fn get_envs() {
        todo!()
    }
    pub fn get_program() {
        todo!()
    }
    pub(crate) fn new(container: &'a Container, program: OsString) -> Self {
        Self {
            container,
            stdout: None,
            stderr: None,
            stdin: None,
            program,
            working_dir: None,
            args: Vec::new(),
        }
    }
    pub fn output(mut self) -> io::Result<Output> {
        self.stdout.get_or_insert(Stdio::piped());
        self.stderr.get_or_insert(Stdio::piped());
        self.stdin.get_or_insert(Stdio::null());
        self.podman_cmd().output()
    }
    pub fn spawn(self) -> io::Result<Child> {
        let child = self.podman_cmd().spawn()?;
        Ok(Child { child })
    }
    pub fn status(self) -> io::Result<ExitStatus> {
        self.podman_cmd().status()
    }
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) {
        self.stderr = Some(cfg.into())
    }
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) {
        self.stdin = Some(cfg.into())
    }
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) {
        self.stdout = Some(cfg.into())
    }

    fn podman_cmd(self) -> StdCommand {
        let mut exec_args = Vec::new();
        if let Some(dir) = self.working_dir {
            exec_args.push("--workdir".into());
            exec_args.push(dir);
        }
        let mut cmd = StdCommand::new("podman");
        cmd.stdin(self.stdin.unwrap_or_else(|| Stdio::inherit()))
            .stdout(self.stdout.unwrap_or_else(|| Stdio::inherit()))
            .stderr(self.stderr.unwrap_or_else(|| Stdio::inherit()))
            .arg("exec")
            .arg(&self.container.name)
            .args(exec_args)
            .arg(self.program)
            .args(self.args);
        cmd
    }
}
