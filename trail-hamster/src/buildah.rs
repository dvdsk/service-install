use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

pub struct Buildah;

impl Buildah {
    pub fn remove_image(id: &str) -> Result<(), CommandError> {
        match buildah_cmd(&[&"rmi", &id]) {
            Ok(_) => Ok(()),
            Err(CommandError::Failed { stderr })
                if stderr.starts_with("Error: no image with name or ID") =>
            {
                Ok(())
            }
            Err(other) => Err(other),
        }
    }

    pub fn rename(curr: &str, new: &str) -> Result<(), CommandError> {
        match buildah_cmd(&[&"tag", &curr, &new]) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn build(build_script: &Path) -> Result<(), CommandError> {
        buildah_cmd(&[&"unshare", &build_script])?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum CommandError {
    Io(std::io::Error),
    Failed { stderr: String },
}

fn buildah_cmd(args: &[&dyn AsRef<OsStr>]) -> Result<String, CommandError> {
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
