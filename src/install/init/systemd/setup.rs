use crate::install::{init, RollbackStep, Tense};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use itertools::Itertools;

use crate::install::builder::Trigger;
use crate::install::init::{EscapedPath, Params, Steps};
use crate::install::Mode;
use crate::install::InstallStep;
use crate::schedule::Schedule;

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
            Tense::Present => "Writing",
            Tense::Future => "Will write",
        };
        let path = self.path.display();
        format!("{verb} systemd service unit to:\n\t{path}")
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Wrote",
            Tense::Present => "Writing",
            Tense::Future => "Will write",
        };
        let path = self.path.display();
        let content = self.unit.replace('\n', "\n\t");
        format!("{verb} systemd service unit to:\n\t{path}\ncontent:\n\t{content}")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, Box<dyn std::error::Error>> {
        write_unit(&self.path, &self.unit)
            .map_err(|e| Error::Writing {
                e,
                path: self.path.clone(),
            })
            .map_err(Box::new)?;
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
            Tense::Present => "Writing",
            Tense::Future => "Will write",
        };
        let path = self.path.display();
        format!("{verb} systemd timer unit to:\n\t{path}")
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Wrote",
            Tense::Present => "Writing",
            Tense::Future => "Will write",
        };
        let path = self.path.display();
        let content = self.unit.replace('\n', "\n\t");
        format!("{verb} systemd timer unit to:\n\t{path}\ncontent:\n\t{content}")
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, Box<dyn std::error::Error>> {
        write_unit(&self.path, &self.unit)
            .map_err(|e| Error::Writing {
                e,
                path: self.path.clone(),
            })
            .map_err(Box::new)?;
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
            Tense::Present => "Enabling",
            Tense::Future => "Will Enable",
        };
        format!("{verb} systemd {} timer: {}", self.mode, self.name)
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, Box<dyn std::error::Error>> {
        let name = self.name.clone() + ".timer";
        super::enable(&name, self.mode)
            .map_err(Error::SystemCtl)
            .map_err(Box::new)?;
        Ok(Some(Box::new(DisableTimer {
            name: self.name.clone(),
            mode: self.mode,
        })))
    }
}

struct EnableService {
    name: String,
    mode: Mode,
}

impl InstallStep for EnableService {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Enabled",
            Tense::Present => "Enabling",
            Tense::Future => "Will Enable",
        };
        format!("{verb} systemd {} service: {}", self.mode, self.name)
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, Box<dyn std::error::Error>> {
        let name = self.name.clone() + ".service";
        super::enable(&name, self.mode)
            .map_err(Error::SystemCtl)
            .map_err(Box::new)?;
        Ok(Some(Box::new(teardown::DisableService {
            name: self.name.clone(),
            mode: self.mode,
        })))
    }
}

pub(crate) fn with_timer(
    path_without_extension: &Path,
    params: &Params,
    schedule: &Schedule,
) -> Steps {
    let unit = render_service(params);
    let path = path_without_extension.with_extension("service");
    let create_service = Box::new(Service { unit, path });
    let unit = render_timer(params, schedule);
    let path = path_without_extension.with_extension("timer");
    let create_timer = Box::new(Timer { unit, path });
    let enable = Box::new(EnableTimer {
        name: params.name.clone(),
        mode: params.mode,
    });

    vec![create_service, create_timer, enable]
}

pub(crate) fn without_timer(path_without_extension: &Path, params: &Params) -> Steps {
    let unit = render_service(params);
    let path = path_without_extension.with_extension("service");
    let create_service = Box::new(Service { unit, path });

    let enable = Box::new(EnableService {
        name: params.name.clone(),
        mode: params.mode,
    });

    vec![create_service, enable]
}

fn render_service(params: &Params) -> String {
    let Params {
        exe_path,
        working_dir,
        exe_args,
        trigger,
        ..
    } = params;

    let description = params.description();
    let ty = match trigger {
        Trigger::OnSchedule(_) => "oneshot",
        Trigger::OnBoot => "simple",
    };

    let exe_path = exe_path.shell_escaped();
    let exe_args: String = Itertools::intersperse(
        exe_args.iter().map(String::shell_escaped),
        String::from(" "),
    )
    .collect();

    let working_dir_section = working_dir
        .as_ref()
        .map(|d| format!("\n   WorkingDirectory={}", d.shell_escaped()))
        .unwrap_or_default();

    let user = params
        .run_as
        .as_ref()
        .map(|user| format!("\n   User={user}"))
        .unwrap_or_default();

    let comment = init::autogenerated_comment(params.bin_name);
    format!(
        "{comment}\n
[Unit]
Description={description}
After=network.target

[Service]
Type={ty}{working_dir_section}{user}
ExecStart={exe_path} {exe_args}

[Install]
WantedBy=multi-user.target"
    )
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
