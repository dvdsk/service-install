use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{fs, io};

use itertools::Itertools;
use tracing::debug;

use crate::install::{InstallError, InstallStep, RollbackError, RollbackStep};
use crate::Tense;

use super::unit::{self, Unit};
use super::{system_path, user_path, FindExeError, Mode};

struct ReEnable {
    units: Vec<Unit>,
    mode: Mode,
}

impl RollbackStep for ReEnable {
    fn perform(&mut self) -> Result<(), RollbackError> {
        for unit in &self.units {
            super::enable(&unit.file_name, self.mode, true)?;
        }
        Ok(())
    }

    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Re-enabled",
            Tense::Active => "Re-enabling",
            Tense::Questioning => "Re-enable",
            Tense::Future => "Will re-enable",
        };
        format!(
            "{verb} the {} services that spawned the original file",
            self.mode
        )
    }
}

struct Disable {
    services: Vec<Unit>,
    timers: Vec<Unit>,
    mode: Mode,
}

impl InstallStep for Disable {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Disabled",
            Tense::Active => "Disabling",
            Tense::Questioning => "Disable",
            Tense::Future => "Will disable",
        };
        format!(
            "{verb} the {} services and/or timers running the file at the install location",
            self.mode
        )
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Disabled",
            Tense::Active => "Disabling",
            Tense::Questioning => "Disable",
            Tense::Future => "Will disable",
        };
        #[allow(clippy::format_collect)]
        let services: String = self
            .services
            .iter()
            .map(|unit| unit.file_name.to_string_lossy().to_string())
            .map(|unit| format!("\n|\t- {unit}"))
            .collect();
        #[allow(clippy::format_collect)]
        let timers: String = self
            .timers
            .iter()
            .map(|unit| unit.file_name.to_string_lossy().to_string())
            .map(|unit| format!("\n|\t- {unit}"))
            .collect();

        match (services.is_empty(), timers.is_empty()) {
            (false, false) => 
        format!(
            "{verb} the {} services and/or timers running the file at the install location\n| services:{services}\n| timers:{timers}",
            self.mode
        ) ,
            (false, true) => 
        format!(
            "{verb} the {} services running the file at the install location\n| services:{services}", self.mode),
            (true, false) => 
        format!(
            "{verb} the {} timers running the file at the install location\n| timers:{timers}",
            self.mode
        ),
            (true, true) => unreachable!("Would have triggered error while constructing the disable installstep.")
        }
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let mut rollback = Box::new(ReEnable {
            mode: self.mode,
            units: Vec::new(),
        });
        for unit in &self.services {
            super::disable(&unit.file_name, self.mode, true).map_err(super::Error::SystemCtl)?;
            rollback.units.push(unit.clone());
        }
        for unit in &self.timers {
            super::disable(&unit.file_name, self.mode, true).map_err(super::Error::SystemCtl)?;
            super::stop(&unit.name(), self.mode).map_err(super::Error::SystemCtl)?;
            rollback.units.push(unit.clone());
        }
        let rollback = rollback as Box<dyn RollbackStep>;
        Ok(Some(rollback))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DisableError {
    #[error("Could not find the service")]
    CouldNotFindIt(#[from] #[source] FindError),
    #[error("Could not open systemd unit")]
    CouldNotReadUnit(#[from] #[source] unit::Error),
    #[error("Could not find the service or (timer) that keeps the file in use")]
    NoServiceOrTimerFound,
}

pub(crate) fn disable_step(
    target: &Path,
    mode: Mode,
) -> Result<Vec<Box<dyn InstallStep>>, DisableError> {
    let path = match mode {
        Mode::User => user_path().unwrap(),
        Mode::System => system_path(),
    };
    let services: Vec<_> = collect_services(&path)
        .map_err(FindError::CouldNotReadDir)?
        .into_iter()
        .map(Unit::from_path)
        .collect::<Result<_, _>>()
        .map_err(DisableError::CouldNotReadUnit)?;
    let timers: Vec<_> = collect_timers(&path)
        .map_err(FindError::CouldNotReadDir)?
        .into_iter()
        .map(Unit::from_path)
        .collect::<Result<_, _>>()
        .map_err(DisableError::CouldNotReadUnit)?;

    let services = find_services_with_target_exe(services, target)?;
    let names: HashSet<_> = services.iter().map(Unit::name).collect();
    let mut timers: Vec<_> = timers
        .into_iter()
        .filter(|timer| names.contains(&timer.name()))
        .collect();
    timers.dedup_by_key(|u| u.name());
    timers.sort_by_key(Unit::name);

    let mut services: Vec<_> = services.into_iter().filter(Unit::has_install).collect();
    services.dedup_by_key(|u| u.name());
    services.sort_by_key(Unit::name);

    if services.is_empty() && timers.is_empty() {
        return Err(DisableError::NoServiceOrTimerFound);
    }
    let disable = Box::new(Disable {
        services,
        timers,
        mode,
    });
    let disable = disable as Box<dyn InstallStep>;
    Ok(vec![disable])
}

fn find_services_with_target_exe(units: Vec<Unit>, target: &Path) -> Result<Vec<Unit>, FindError> {
    let (units, errs): (Vec<_>, Vec<_>) = units
        .into_iter()
        .map(|unit| unit.exe_path().map(|exe| (exe, unit)))
        .filter_ok(|(exe, _)| exe == target)
        .map_ok(|(_, unit)| unit)
        .partition_result();

    if !errs.is_empty() {
        debug!("Some service files failed to parse: {errs:#?}")
    }

    Ok(units)
}

#[derive(Debug, thiserror::Error)]
pub enum FindError {
    #[error(
        "No service spawning the target file found, could not parse some services however: {0:#?}"
    )]
    NotFoundWithErrors(Vec<FindExeError>),
    #[error("Could not read directory")]
    CouldNotReadDir(#[from] #[source] std::io::Error),
}

fn walk_dir(dir: &Path, process_file: &mut impl FnMut(&Path)) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, process_file)?;
            } else if path.is_file() {
                (process_file)(&path);
            }
        }
    }
    Ok(())
}
fn collect_services(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut units = Vec::new();
    walk_dir(dir, &mut |path| {
        if path.extension().is_some_and(|e| e == "service") {
            units.push(path.to_owned());
        }
    })?;
    Ok(units)
}

fn collect_timers(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut units = Vec::new();
    walk_dir(dir, &mut |path| {
        if path.extension().is_some_and(|e| e == "timer") {
            units.push(path.to_owned());
        }
    })?;
    Ok(units)
}
