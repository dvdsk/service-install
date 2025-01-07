#![allow(clippy::missing_errors_doc)]
// ^needed as we have a lib and a main, pub crate would
// only allow access from the lib. However since the lib is not
// public it makes no sense to document errors.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use std::{fs, io};

use crate::install::builder::Trigger;
use crate::install::files::NoHomeError;

pub use self::unit::FindExeError;
use self::unit::Unit;

use super::{ExeLocation, Mode, Params, PathCheckError, RSteps, SetupError, Steps, TearDownError};

mod disable_existing;
mod setup;
mod teardown;
mod unit;

pub(crate) use disable_existing::disable_step;
pub use disable_existing::DisableError;

#[derive(thiserror::Error, Debug)]
pub enum SystemCtlError {
    #[error("Could not run systemctl")]
    Io(
        #[from]
        #[source]
        std::io::Error,
    ),
    #[error("Systemctl failed: {reason}")]
    Failed { reason: String },
    #[error("Timed out trying to enable service")]
    RestartTimeOut,
    #[error("Timed out trying to disable service")]
    DisableTimeOut,
    #[error("Timed out trying to stop service")]
    StopTimeOut,
    #[error("Something send a signal to systemctl ending it before it could finish")]
    Terminated,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Could not configure systemd")]
    SystemCtl(
        #[from]
        #[source]
        SystemCtlError,
    ),
    #[error("Could not write out unit file to {path}")]
    Writing {
        #[source]
        e: io::Error,
        path: PathBuf,
    },
    #[error("Could not remove the unit files, error: {0}")]
    Removing(#[source] io::Error),
    #[error("Could not verify unit files where created by us, could not open them")]
    Verifying(
        #[from]
        #[source]
        unit::Error,
    ),
    #[error("Could not check if this system uses systemd")]
    CheckingInitSys(
        #[from]
        #[source]
        PathCheckError,
    ),
    #[error("Could not check if there is an existing service we will replace")]
    CheckingRunning(#[source] SystemCtlError),
}

pub(crate) fn path_is_systemd(path: &Path) -> Result<bool, PathCheckError> {
    let path = path.canonicalize().map_err(PathCheckError)?;

    Ok(path
        .components()
        .filter_map(|c| match c {
            Component::Normal(cmp) => Some(cmp),
            _other => None,
        })
        .filter_map(|c| c.to_str())
        .any(|c| c == "systemd"))
}

// check if systemd is the init system (PID 1)
pub(super) fn not_available() -> Result<bool, SetupError> {
    use sysinfo::{Pid, System};
    let mut s = System::new();
    s.refresh_processes(
        sysinfo::ProcessesToUpdate::Some([Pid::from(1)].as_slice()),
        true,
    );
    let init_sys = &s
        .process(Pid::from(1))
        .expect("there should always be an init system")
        .cmd()[0];
    Ok(!path_is_systemd(Path::new(init_sys)).map_err(Error::from)?)
}

pub(super) fn set_up_steps(params: &Params) -> Result<Steps, SetupError> {
    let path_without_extension = match params.mode {
        Mode::User => user_path()?,
        Mode::System => system_path(),
    }
    .join(&params.name);

    Ok(match params.trigger {
        Trigger::OnSchedule(ref schedule) => {
            setup::with_timer(&path_without_extension, params, schedule)
        }
        Trigger::OnBoot => setup::without_timer(&path_without_extension, params)?,
    })
}

pub(super) fn tear_down_steps(mode: Mode) -> Result<Option<(RSteps, ExeLocation)>, TearDownError> {
    let dir = match mode {
        Mode::User => user_path()?,
        Mode::System => system_path(),
    };

    let mut steps = Vec::new();
    let mut exe_paths = Vec::new();

    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            continue;
        }
        let Some(extension) = path.extension().and_then(OsStr::to_str) else {
            continue;
        };
        let unit = Unit::from_path(path.clone()).unwrap();
        if !unit.our_service() {
            continue;
        }
        let Some(service_name) = path.file_stem().and_then(OsStr::to_str) else {
            continue;
        };

        match extension {
            "timer" => {
                steps.extend(teardown::disable_then_remove_with_timer(
                    unit.path.clone(),
                    service_name,
                    mode,
                ));
            }
            "service" => {
                steps.extend(teardown::disable_then_remove_service(
                    unit.path.clone(),
                    service_name,
                    mode,
                ));
                exe_paths.push(unit.exe_path().map_err(TearDownError::FindingExePath)?);
            }
            _ => continue,
        }
    }

    exe_paths.dedup();
    match (steps.len(), exe_paths.as_slice()) {
        (0, []) => Ok(None),
        (0, [_, ..]) => unreachable!("if we get an exe path we got one service to remove"),
        (1.., []) => Err(TearDownError::TimerWithoutService),
        (1.., [exe_path]) => Ok(Some((steps, exe_path.clone()))),
        (1.., _) => Err(TearDownError::MultipleExePaths(exe_paths)),
    }
}

/// There are other paths, but for now we return the most commonly used one
fn user_path() -> Result<PathBuf, NoHomeError> {
    Ok(home::home_dir()
        .ok_or(NoHomeError)?
        .join(".config/systemd/user/"))
}

/// There are other paths, but for now we return the most commonly used one
fn system_path() -> PathBuf {
    PathBuf::from("/etc/systemd/system")
}

fn systemctl(args: &[&'static str], service: &OsStr) -> Result<(), SystemCtlError> {
    let output = Command::new("systemctl").args(args).arg(service).output()?;

    if output.status.success() {
        return Ok(());
    }

    let reason = String::from_utf8_lossy(&output.stderr).to_string();
    Err(SystemCtlError::Failed { reason })
}

fn is_active(service: impl AsRef<OsStr>, mode: Mode) -> Result<bool, SystemCtlError> {
    let args = match mode {
        Mode::System => &["is-active"][..],
        Mode::User => &["is-active", "--user"][..],
    };

    let output = Command::new("systemctl").args(args).arg(service).output()?;
    Ok(output.status.code().ok_or(SystemCtlError::Terminated)? == 0)
}

fn wait_for(
    service: &OsStr,
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

fn enable(unit: &OsStr, mode: Mode, start: bool) -> Result<(), SystemCtlError> {
    let mut args = match mode {
        Mode::System => vec!["enable"],
        Mode::User => vec!["enable", "--user"],
    };

    if start {
        args.push("--now");
    }

    systemctl(&args, unit)?;
    wait_for(unit, true, mode, SystemCtlError::RestartTimeOut)
}

fn restart(unit: &OsStr, mode: Mode) -> Result<(), SystemCtlError> {
    let args = match mode {
        Mode::System => vec!["restart"],
        Mode::User => vec!["restart", "--user"],
    };

    systemctl(&args, unit)?;
    wait_for(unit, true, mode, SystemCtlError::RestartTimeOut)
}

fn disable(unit: &OsStr, mode: Mode, start: bool) -> Result<(), SystemCtlError> {
    let mut args = match mode {
        Mode::System => vec!["disable"],
        Mode::User => vec!["disable", "--user"],
    };

    if start {
        args.push("--now");
    }

    systemctl(&args, unit)?;
    wait_for(unit, false, mode, SystemCtlError::DisableTimeOut)
}

fn stop(unit: &OsStr, mode: Mode) -> Result<(), SystemCtlError> {
    let args = match mode {
        Mode::System => &["stop"][..],
        Mode::User => &["stop", "--user"][..],
    };

    systemctl(args, unit)?;
    wait_for(unit, false, mode, SystemCtlError::StopTimeOut)
}
