#![allow(clippy::missing_errors_doc)]
// ^needed as we have a lib and a main, pub crate would
// only allow access from the lib. However since the lib is not
// public it makes no sense to document errors.

use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

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
    #[error("Could not run systemctl, error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Systemctl failed: {reason}")]
    Failed { reason: String },
    #[error("Timed out trying to enable service")]
    EnableTimeOut,
    #[error("Timed out trying to disable service")]
    DisableTimeOut,
    #[error("Timed out trying to stop service")]
    StopTimeOut,
    #[error("Something send a signal to systemctl ending it")]
    Terminated,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Could not configure systemd: {0}")]
    SystemCtl(#[from] SystemCtlError),
    #[error("Could not write out unit file to {path}, error: {e}")]
    Writing { e: io::Error, path: PathBuf },
    #[error("Could not remove the unit files, error: {0}")]
    Removing(io::Error),
    #[error("Could not verify unit files where created by us, could not open them, error: {0}")]
    Verifying(#[from] unit::Error),
    #[error("Could not check if this system uses systemd, err: {0}")]
    CheckingInitSys(#[from] PathCheckError),
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

// check if systemd is the init system (pid 1)
pub(super) fn not_available() -> Result<bool, SetupError> {
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
        Trigger::OnBoot => setup::without_timer(&path_without_extension, params),
    })
}

pub(super) fn tear_down_steps(
    name: &str,
    mode: Mode,
) -> Result<Option<(RSteps, ExeLocation)>, TearDownError> {
    let without_extension = match mode {
        Mode::User => user_path()?,
        Mode::System => system_path(),
    }
    .join(name);

    let mut steps = Vec::new();

    let timer_path = without_extension.with_extension("timer");
    let timer = Unit::from_path(timer_path).map_err(Error::Verifying)?;
    if timer.our_service() {
        steps.extend(teardown::disable_then_remove_with_timer(
            timer.path.clone(),
            name,
            mode,
        ));
    }

    let service_path = without_extension.with_extension("service");
    let service = Unit::from_path(service_path).map_err(Error::Verifying)?;

    let exe_path = if service.our_service() {
        steps.extend(teardown::disable_then_remove_service(
            service.path.clone(),
            name,
            mode,
        ));
        service.exe_path().map_err(TearDownError::FindingExePath)?
    } else if timer.our_service() {
        return Err(TearDownError::TimerWithoutService);
    } else {
        return Ok(None);
    };

    Ok(Some((steps, exe_path)))
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

fn is_active(service: &OsStr, mode: Mode) -> Result<bool, SystemCtlError> {
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
    wait_for(unit, true, mode, SystemCtlError::EnableTimeOut)
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
