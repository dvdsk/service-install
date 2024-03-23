#![allow(clippy::missing_errors_doc)]
// ^needed as we have a lib and a main, pub crate would
// only allow access from the lib. However since the lib is not
// public it makes no sense to document errors.

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};
use std::{fs, io};

use crate::install::builder::Trigger;
use crate::install::files::NoHomeError;

use super::{Mode, Params, SetupError, Steps, System, TearDownError};

mod setup;
mod teardown;

#[derive(thiserror::Error, Debug)]
pub enum SystemCtlError {
    #[error("Could not run systemctl")]
    Io(#[from] std::io::Error),
    #[error("Systemctl failed: {reason}")]
    Failed { reason: String },
    #[error("Timed out trying to enable service")]
    EnableTimeOut,
    #[error("Timed out trying to enable service")]
    DisableTimeOut,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Could not configure systemd: {0}")]
    SystemCtl(#[from] SystemCtlError),
    #[error("Could not write out unit file to {path}. Error: {e}")]
    Writing { e: io::Error, path: PathBuf },
    #[error("Could not remove the unit files")]
    Removing(io::Error),
    #[error("Could not verify unit files where created by us")]
    Verifying(io::Error),
    #[error("Could not check if this system uses systemd (init system path could not be resolved")]
    CheckingInitSys(io::Error),
}

const COMMENT_PREAMBLE: &str = "created by: ";
const COMMENT_SUFFIX: &str = " during its installation, might get removed by it in the future. Remove this comment to prevent that";

pub struct Systemd {}

impl System for Systemd {
    fn name(&self) -> &'static str {
        "systemd"
    }

    // check if systemd is the init system (pid 1)
    fn not_available(&self) -> Result<bool, SetupError> {
        use sysinfo::{Pid, ProcessRefreshKind, System, UpdateKind};
        let mut s = System::new();
        s.refresh_pids_specifics(
            &[Pid::from(1)],
            ProcessRefreshKind::new().with_cmd(UpdateKind::Always),
        );
        let init_sys = &s
            .process(Pid::from(1))
            .expect("there should always be an init system")
            .cmd()[0];
        let init_path = Path::new(init_sys.as_str())
            .canonicalize()
            .map_err(Error::CheckingInitSys)?;

        Ok(!init_path
            .components()
            .filter_map(|c| match c {
                Component::Normal(cmp) => Some(cmp),
                _other => None,
            })
            .filter_map(|c| c.to_str())
            .any(|c| c == "systemd"))
    }

    fn set_up_steps(&self, params: &Params) -> Result<Steps, SetupError> {
        let path_without_extension = match params.mode {
            Mode::User => user_path()?,
            Mode::System => system_path(),
        }
        .join(&params.name);

        match params.trigger {
            Trigger::OnSchedule(ref schedule) => {
                setup::with_timer(path_without_extension, params, schedule)
            }
            Trigger::OnBoot => setup::without_timer(path_without_extension, params),
        }
    }

    fn tear_down_steps(&self, name: &str, mode: Mode) -> Result<(Steps, PathBuf), TearDownError> {
        let without_extension = match mode {
            Mode::User => user_path()?,
            Mode::System => system_path(),
        }
        .join(&name);

        let mut steps = Vec::new();
        let service_path = without_extension.with_extension("service");

        let exe_path = if our_service(&service_path)? {
            steps.extend(teardown::remove_then_disable_service(
                service_path.clone(),
                name,
                mode,
            ));
            exe_path(service_path).map_err(TearDownError::FindingExePath)?
        } else {
            return Err(TearDownError::NoService);
        };

        let timer_path = without_extension.with_extension("timer");
        if our_service(&timer_path)? {
            steps.extend(teardown::remove_then_disable_timer(timer_path, name, mode));
        }

        Ok((steps, exe_path))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FindExeError {
    #[error("Could not read systemd unit file at: {path}, error: {err}")]
    ReadingUnit { err: std::io::Error, path: PathBuf },
    #[error("ExecStart (use to find binary) is missing from file")]
    ExecLineMissing,
    #[error("Path to binary extracted from systemd unit does not lead to a file, path: {0}")]
    ExacPathNotFile(PathBuf),
}

fn exe_path(service_unit: PathBuf) -> Result<PathBuf, FindExeError> {
    let unit = std::fs::read_to_string(&service_unit).map_err(|err| FindExeError::ReadingUnit {
        err,
        path: service_unit,
    })?;
    let path = unit
        .lines()
        .map(|l| l.trim())
        .find_map(|l| l.strip_prefix("ExacStart="))
        .ok_or(FindExeError::ExecLineMissing)?;
    let path = PathBuf::from_str(path).expect("infallible");
    if !path.is_file() {
        Err(FindExeError::ExacPathNotFile(path))
    } else {
        Ok(path)
    }
}

fn user_path() -> Result<PathBuf, NoHomeError> {
    Ok(home::home_dir()
        .ok_or(NoHomeError)?
        .join(".config/systemd/user/"))
}

fn system_path() -> PathBuf {
    PathBuf::from("/etc/systemd/system")
}

fn our_service(service_path: &Path) -> Result<bool, Error> {
    let service = match fs::read_to_string(service_path) {
        Ok(service) => service,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(Error::Verifying(e)),
    };

    return Ok(service.contains(COMMENT_PREAMBLE) && service.contains(COMMENT_SUFFIX));
}

fn systemctl(args: &[&'static str], service: &str) -> Result<(), SystemCtlError> {
    let output = Command::new("systemctl").args(args).arg(service).output()?;

    if output.status.success() {
        return Ok(());
    }

    let reason = String::from_utf8(output.stderr).unwrap();
    Err(SystemCtlError::Failed { reason })
}

fn is_active(service: &str, mode: Mode) -> Result<bool, SystemCtlError> {
    let args = match mode {
        Mode::System => &["is-active"][..],
        Mode::User => &["is-active", "--user"][..],
    };

    let output = Command::new("systemctl").args(args).arg(service).output()?;
    Ok(output.status.code().unwrap() == 0)
}

fn wait_for(
    service: &str,
    state: bool,
    mode: Mode,
    timeout_error: SystemCtlError,
) -> Result<(), SystemCtlError> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        if state == is_active(service, mode)? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(timeout_error)
}

fn enable(unit: &str, mode: Mode) -> Result<(), SystemCtlError> {
    let args = match mode {
        Mode::System => &["enable", "--now"][..],
        Mode::User => &["enable", "--user", "--now"][..],
    };
    systemctl(args, unit)?;
    wait_for(unit, true, mode, SystemCtlError::EnableTimeOut)
}

fn disable(unit: &str, mode: Mode) -> Result<(), SystemCtlError> {
    let args = match mode {
        Mode::System => &["disable", "--now"][..],
        Mode::User => &["disable", "--user", "--now"][..],
    };
    systemctl(args, unit)?;
    wait_for(unit, false, mode, SystemCtlError::DisableTimeOut)
}
