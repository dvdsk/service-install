use crate::install::init::systemd;
use crate::install::{init, InstallError, RollbackStep, Tense};
use std::collections::HashMap;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use itertools::Itertools;

use crate::install::builder::Trigger;
use crate::install::init::{Params, ShellEscape, Steps, SystemdEscape};
use crate::install::InstallStep;
use crate::install::Mode;
use crate::schedule::Schedule;

use super::api::on_seperate_tokio_thread;
use super::teardown::DisableTimer;
use super::{teardown, Error};

struct Service {
    unit: String,
    path: PathBuf,
}

impl InstallStep for Service {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Wrote",
            Tense::Questioning => "Write",
            Tense::Future => "Will write",
            Tense::Active => "Writing",
        };
        let path = self.path.display();
        format!(
            "{verb} systemd service unit{}\n\t| path: {path}",
            tense.punct()
        )
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Wrote",
            Tense::Questioning => "Write",
            Tense::Future => "Will write",
            Tense::Active => "Writing",
        };
        let path = self.path.display();
        let content = self.unit.trim_end().replace('\n', "\n|\t");
        format!(
            "{verb} systemd service unit{}\n| path:\n|\t{path}\n| content:\n|\t{content}",
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        write_unit(&self.path, &self.unit).map_err(|e| Error::Writing {
            e,
            path: self.path.clone(),
        })?;
        Ok(Some(Box::new(teardown::RemoveService {
            path: self.path.clone(),
        })))
    }
}

struct Timer {
    unit: String,
    path: PathBuf,
}

impl InstallStep for Timer {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Wrote",
            Tense::Questioning => "Write",
            Tense::Future => "Will write",
            Tense::Active => "Writing",
        };
        let path = self.path.display();
        format!(
            "{verb} systemd timer unit{}\n\t| path: {path}",
            tense.punct()
        )
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Wrote",
            Tense::Questioning => "Write",
            Tense::Future => "Will write",
            Tense::Active => "Writing",
        };
        let path = self.path.display();
        let content = self.unit.trim_end().replace('\n', "\n|\t");
        format!(
            "{verb} systemd timer unit{}\n| path:\n|\t{path}\n| content:\n|\t{content}",
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        write_unit(&self.path, &self.unit).map_err(|e| Error::Writing {
            e,
            path: self.path.clone(),
        })?;
        Ok(Some(Box::new(teardown::RemoveTimer {
            path: self.path.clone(),
        })))
    }
}

struct EnableTimer {
    name: String,
    mode: Mode,
}

impl InstallStep for EnableTimer {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Enabled",
            Tense::Questioning => "Enable",
            Tense::Future => "Will Enable",
            Tense::Active => "Enabling",
        };
        format!(
            "{verb} systemd {} timer: {}{}",
            self.mode,
            self.name,
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let name = self.name.clone() + ".timer";
        on_seperate_tokio_thread! {{
            super::enable(name.as_ref(), self.mode, true).await
        }}?;
        Ok(Some(Box::new(DisableTimer {
            name: self.name.clone(),
            mode: self.mode,
        })))
    }
}

struct EnableService {
    name: String,
    mode: Mode,
    start: bool,
    already_running: bool,
}

impl InstallStep for EnableService {
    fn describe(&self, tense: Tense) -> String {
        let enable = match tense {
            Tense::Past => "Enabled",
            Tense::Questioning => "Enable",
            Tense::Future => "Will Enable",
            Tense::Active => "Enabling",
        };
        let start = if self.start {
            match (&tense, self.already_running) {
                (Tense::Past, true) => "restarted",
                (Tense::Past, false) => "started",
                (Tense::Questioning | Tense::Future, true) => "restart",
                (Tense::Questioning | Tense::Future, false) => "start",
                (Tense::Active, true) => "restarting",
                (Tense::Active, false) => "starting",
            }
        } else {
            ""
        };
        format!(
            "{enable} and {start} systemd {} service: {}{}",
            self.mode,
            self.name,
            tense.punct(),
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let name = self.name.clone() + ".service";
        on_seperate_tokio_thread! {{
            super::enable(name.as_ref(), self.mode, self.start).await?;

            if self.already_running {
                super::restart(name.as_ref(), self.mode).await?;
            }
            Ok::<_, InstallError>(())
        }}?;

        Ok(Some(Box::new(teardown::DisableService {
            name: self.name.clone(),
            mode: self.mode,
            stop: self.start,
        })))
    }
}

fn with_added_extension(path: &Path, extension: &str) -> PathBuf {
    let mut path = path.as_os_str().to_os_string();
    path.push(".");
    path.push(extension);
    PathBuf::from(path)
}

pub(crate) fn with_timer(
    path_without_extension: &Path,
    params: &Params,
    schedule: &Schedule,
) -> Steps {
    let unit = render_service(params);
    let path = with_added_extension(path_without_extension, "service");
    let create_service = Box::new(Service { unit, path });
    let unit = render_timer(params, schedule);
    let path = with_added_extension(path_without_extension, "timer");
    let create_timer = Box::new(Timer { unit, path });
    let enable = Box::new(EnableTimer {
        name: params.name.clone(),
        mode: params.mode,
    });

    vec![create_service, create_timer, enable]
}

pub(crate) fn without_timer(
    path_without_extension: &Path,
    params: &Params,
) -> Result<Steps, systemd::Error> {
    let unit = render_service(params);
    let path = with_added_extension(path_without_extension, "service");
    let already_running = on_seperate_tokio_thread! {{
        systemd::is_active(&params.name, params.mode).await
    }}?;

    let create_service = Box::new(Service { unit, path });

    let enable = Box::new(EnableService {
        name: params.name.clone(),
        mode: params.mode,
        start: true,
        already_running,
    });

    Ok(vec![create_service, enable])
}

fn render_service(params: &Params) -> String {
    let Params {
        exe_path,
        working_dir,
        exe_args,
        environment,
        trigger,
        ..
    } = params;

    let description = params.description();

    let working_dir_section = working_dir
        .as_ref()
        .map(|d| format!("\nWorkingDirectory={}", d.shell_escaped()))
        .unwrap_or_default();
    let user_section = params
        .run_as
        .as_ref()
        .map(|user| format!("\nUser={user}"))
        .unwrap_or_default();
    let environment_section = render_environment_section(environment);

    let exe_path = exe_path.systemd_escape();
    let exe_args: String = exe_args.iter().map(String::systemd_escape).join(" \\\n\t");

    let target = match params.mode {
        Mode::User => "default.target",
        Mode::System => "multi-user.target",
    };

    let install_section = match trigger {
        Trigger::OnSchedule(_) => String::new(), // started by timer
        Trigger::OnBoot => format!("[Install]\nWantedBy={target}\n"),
    };

    let comment = init::autogenerated_comment(params.bin_name);
    format!(
        "{comment}\n
[Unit]
Description={description}
After=network.target

[Service]
Type=simple{working_dir_section}{user_section}{environment_section}
ExecStart={exe_path} {exe_args}
{install_section}"
    )
}

fn render_environment_section(environment: &HashMap<String, String>) -> String {
    if environment.is_empty() {
        String::new()
    } else {
        let key_val_pairs: String = environment
            .iter()
            .map(|(key, value)| format!("{}={}", key.systemd_escape(), value.systemd_escape()))
            .join(" ");
        format!("\nEnvironment={key_val_pairs}")
    }
}

fn render_timer(params: &Params, schedule: &Schedule) -> String {
    let description = params.description();
    let on_calander = match schedule {
        Schedule::Daily(time) => {
            format!("*-*-* {}:{}:{}", time.hour(), time.minute(), time.second())
        }
    };

    let comment = init::autogenerated_comment(params.bin_name);
    format!(
        "{comment}\n
[Unit]
Description={description}

[Timer]
OnCalendar={on_calander}
AccuracySec=60

[Install]
WantedBy=timers.target"
    )
}

fn write_unit(path: &Path, unit: &str) -> Result<(), io::Error> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(unit.as_bytes())?;
    let meta = f.metadata()?;
    let mut perm = meta.permissions();
    perm.set_mode(0o664);
    Ok(())
}
