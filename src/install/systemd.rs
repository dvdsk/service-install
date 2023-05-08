#![allow(clippy::missing_errors_doc)]
// ^needed as we have a lib and a main, pub crate would
// only allow access from the lib. However since the lib is not
// public it makes no sense to document errors.

use itertools::Itertools;
use std::borrow::Cow;
use std::io;
use std::io::Write;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::install::builder::Trigger;
use crate::Schedule;

use super::{InitError, InitParams, InitSystem, Mode};

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
    #[error("Could not configure systemd")]
    SystemCtl(#[from] SystemCtlError),
    #[error("Could not write out unit files")]
    Io(#[from] std::io::Error),
}

pub struct Systemd {}

impl InitSystem for Systemd {
    fn name(&self) -> &'static str {
        "systemd"
    }
    fn set_up(&self, params: &InitParams) -> Result<(), InitError> {
        let path_without_extension = match params.mode {
            Mode::User => home::home_dir()
                .ok_or(InitError::NoHome)?
                .join("~/.config/systemd/user/")
                .join(&params.name),
            Mode::System => Path::new("/etc/systemd/system").join(&params.name),
        };

        if let Mode::User = params.mode {
            todo!("loginctl enable-linger username")
        }

        match params.trigger {
            Trigger::OnSchedual(ref schedule) => {
                let service = render_service(&params);
                write_unit(path_without_extension.with_extension("service"), &service)
                    .map_err(Error::Io)?;
                let timer = render_timer(&params, schedule);
                write_unit(path_without_extension.with_extension("timer"), &timer)
                    .map_err(Error::Io)?;
                enable(&(params.name.clone() + ".timer")).map_err(Error::SystemCtl)?;
            }
            Trigger::OnBoot => {
                let service = render_service(&params);
                write_unit(path_without_extension.with_extension("service"), &service)
                    .map_err(Error::Io)?;
                enable(&(params.name.clone() + ".service")).map_err(Error::SystemCtl)?;
            }
        }
        Ok(())
    }
}

fn render_service(params: &InitParams) -> String {
    let InitParams {
        exe_path,
        working_dir,
        exe_args,
        trigger,
        ..
    } = params;

    let description = params.description();
    let ty = match trigger {
        Trigger::OnSchedual(_) => "oneshot",
        Trigger::OnBoot => "simple",
    };

    let exe_path = exe_path.display();
    let exe_args: String = Itertools::intersperse(
        exe_args
            .iter()
            .map(String::as_str)
            .map(Cow::Borrowed)
            .map(|s| shell_escape::escape(s))
            .map(Cow::into_owned),
        String::from(" "),
    )
    .collect();

    let working_dir_section = working_dir
        .as_ref()
        .map(|d| format!("\n   WorkingDirectory={}", d.display()))
        .unwrap_or_else(String::new);

    format!(
        "[Unit]
            Description={description}
            After=network.target

            [Service]
            Type={ty}{working_dir_section}
            ExecStart={exe_path} {exe_args}

            [Install]
            WantedBy=multi-user.target
            ",
    )
}

fn render_timer(params: &InitParams, schedule: &Schedule) -> String {
    let description = params.description();
    let on_calander = match schedule {
        Schedule::Daily(time) => {
            format!("*-*-* {}:{}:{}", time.hour(), time.minute(), time.second())
        }
    };

    format!(
        "[Unit]
        Description={description}
        [Timer]
        OnCalendar={on_calander}
        AccuracySec=60
        [Install]
        WantedBy=timers.target
        "
    )
}

fn write_unit(path: PathBuf, unit: &str) -> Result<(), io::Error> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(unit.as_bytes())?;
    let meta = f.metadata()?;
    let mut perm = meta.permissions();
    perm.set_mode(0o664);
    Ok(())
}

fn systemctl(args: &[&'static str], service: &str) -> Result<(), SystemCtlError> {
    let output = Command::new("systemctl").args(args).arg(service).output()?;

    if output.status.success() {
        return Ok(());
    }

    let reason = String::from_utf8(output.stderr).unwrap();
    Err(SystemCtlError::Failed { reason })
}

fn is_active(service: &str) -> Result<bool, SystemCtlError> {
    let output = Command::new("systemctl")
        .arg("is-active")
        .arg(service)
        .output()?;

    Ok(output.status.code().unwrap() == 0)
}

fn wait_for(
    service: &str,
    state: bool,
    timeout_error: SystemCtlError,
) -> Result<(), SystemCtlError> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        if state == is_active(service)? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(timeout_error)
}

fn enable(unit: &str) -> Result<(), SystemCtlError> {
    systemctl(&["enable", "--now"], unit)?;
    wait_for(unit, true, SystemCtlError::EnableTimeOut)
}

fn disable(unit: &str) -> Result<(), SystemCtlError> {
    systemctl(&["disable", "--now"], unit)?;
    wait_for(unit, false, SystemCtlError::DisableTimeOut)
}
