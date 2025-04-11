#![allow(clippy::missing_errors_doc)]
// ^needed as we have a lib and a main, pub crate would
// only allow access from the lib. However since the lib is not
// public it makes no sense to document errors.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::{fs, io};

use crate::install::builder::Trigger;
use crate::install::files::NoHomeError;

pub use self::unit::FindExeError;
use self::unit::Unit;

use super::{ExeLocation, Mode, Params, PathCheckError, RSteps, SetupError, Steps, TearDownError};

mod api;
mod disable_existing;
mod setup;
mod teardown;
mod unit;

pub(crate) use disable_existing::disable_step;
pub use disable_existing::DisableError;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

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
    #[error("Could not enable the service")]
    Enabling(#[source] api::Error),
    #[error("Could not start the service")]
    Starting(#[source] api::Error),
    #[error("Could not restart the service")]
    Restarting(#[source] api::Error),
    #[error("Could not disable the service")]
    Disabling(#[source] api::Error),
    #[error("Could not stop the service")]
    Stopping(#[source] api::Error),
    #[error("Could not check if the service is active or not")]
    CheckActive(#[source] api::Error),
    #[error("Error while waiting for service to be started")]
    WaitingForStart(#[source] api::WaitError),
    #[error("Error while waiting for service to be stopped")]
    WaitingForStop(#[source] api::WaitError),
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

// Check if systemd is the init system (PID 1)
pub(super) fn not_available() -> Result<bool, SetupError> {
    use sysinfo::{Pid, System};
    let mut s = System::new();
    s.refresh_processes_specifics(
        ProcessesToUpdate::Some([Pid::from(1)].as_slice()),
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    let init_sys = &s
        .process(Pid::from(1))
        .expect("there should always be an init system")
        .cmd()
        .first()
        .expect("we requested command");
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

async fn enable(unit: &str, mode: Mode, and_start: bool) -> Result<(), Error> {
    api::enable_service(unit, mode)
        .await
        .map_err(Error::Enabling)?;
    if and_start {
        api::start_service(unit, mode)
            .await
            .map_err(Error::Starting)?;
        api::wait_for_active(unit, mode)
            .await
            .map_err(Error::WaitingForStart)?;
    }
    Ok(())
}

async fn restart(unit_file_name: &str, mode: Mode) -> Result<(), Error> {
    api::restart(unit_file_name, mode)
        .await
        .map_err(Error::Restarting)
}

async fn disable(unit_file_name: &str, mode: Mode, and_stop: bool) -> Result<(), Error> {
    api::disable_service(unit_file_name, mode)
        .await
        .map_err(Error::Disabling)?;
    if and_stop {
        stop(unit_file_name, mode).await?;
        api::wait_for_active(unit_file_name, mode)
            .await
            .map_err(Error::WaitingForStop)?;
    }
    Ok(())
}

async fn stop(unit_file_name: &str, mode: Mode) -> Result<(), Error> {
    api::stop_service(unit_file_name, mode)
        .await
        .map_err(Error::Stopping)
}

async fn is_active(unit_file_name: &str, mode: Mode) -> Result<bool, Error> {
    api::is_active(unit_file_name, mode)
        .await
        .map_err(Error::CheckActive)
}
