use core::fmt;
use std::ffi::OsStr;
use std::process::{Child, Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub struct Image {
    pub id: String,
    pub repo: String,
    pub tag: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Container {
    pub id: String,
    pub image: String,
    pub name: String,
}

pub trait ContainerEngine {
    type Error: fmt::Debug;

    fn images() -> Result<Vec<Image>, Self::Error>;
    fn containers() -> Result<Vec<Container>, Self::Error>;
    fn stop(id: &str) -> Result<(), Self::Error>;
    fn remove(id: &str) -> Result<(), Self::Error>;
    fn spawn(image: String, name: &str) -> Result<Child, Self::Error>;
    fn exec<I, S>(container: impl AsRef<OsStr>, cmd: I) -> Result<String, Self::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;
}

pub struct Podman;

impl ContainerEngine for Podman {
    type Error = CommandError;
    fn images() -> Result<Vec<Image>, Self::Error> {
        Ok(podman_cmd(&[&"images"])?
            .lines()
            .skip(1)
            .map(str::split_whitespace)
            .map(|mut w| Image {
                repo: w.next().unwrap().to_string(),
                tag: w.next().unwrap().to_string(),
                id: w.next().unwrap().to_string(),
            })
            .collect())
    }

    fn containers() -> Result<Vec<Container>, Self::Error> {
        Ok(podman_cmd(&[&"ps", &"-a"])?
            .lines()
            .skip(1)
            .map(str::split_whitespace)
            .map(|mut w| Container {
                id: w.next().unwrap().to_string(),
                image: w.next().unwrap().to_string(),
                name: w.next_back().unwrap().to_string(),
            })
            .collect())
    }

    fn stop(id: &str) -> Result<(), Self::Error> {
        match podman_cmd(&[&"stop", &id]) {
            Ok(_) => Ok(()),
            Err(CommandError::Failed { stderr })
                if stderr.starts_with("Error: no container with name or ID") =>
            {
                Ok(())
            }
            Err(other) => Err(other),
        }
    }

    fn remove(id: &str) -> Result<(), Self::Error> {
        match podman_cmd(&[&"rm", &id]) {
            Ok(_) => Ok(()),
            Err(CommandError::Failed { stderr })
                if stderr.starts_with("Error: no container with name or ID") =>
            {
                Ok(())
            }
            Err(other) => Err(other),
        }
    }

    fn spawn(image: String, name: &str) -> Result<Child, Self::Error> {
        Command::new("podman")
            .arg("run")
            .arg("--name")
            .arg(name)
            .arg("--privileged")
            .arg(&image)
            // needed to run systemd in container. Container will
            // still run as a user in the host system.
            .stderr(Stdio::piped())
            .spawn()
            .map_err(CommandError::Io)
    }

    // blocks until exec is done, returns stdout
    fn exec<I, S>(container_id: impl AsRef<OsStr>, cmd: I) -> Result<String, Self::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let cmd: Vec<_> = cmd.into_iter().collect();
        let cmd_args = cmd.iter().map(|s| s.as_ref());
        let args = [OsStr::new("exec"), container_id.as_ref()]
            .into_iter()
            .chain(cmd_args);

        let output = podman_cmd(args)?;
        Ok(output)
    }
}

#[derive(Debug)]
pub enum CommandError {
    Io(std::io::Error),
    Failed { stderr: String },
}

fn podman_cmd<I, S>(args: I) -> Result<String, CommandError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("podman")
        .args(args)
        .output()
        .map_err(CommandError::Io)?;
    if !output.status.success() {
        Err(CommandError::Failed {
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }
}