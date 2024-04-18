use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::{fs, io};

use itertools::Itertools;

use crate::install::{InstallError, InstallStep, RollbackError, RollbackStep};
use crate::Tense;

use super::{exe_path, system_path, user_path, FindExeError, Mode};

struct ReEnable {
    units: Vec<OsString>,
    mode: Mode,
}

impl RollbackStep for ReEnable {
    fn perform(&mut self) -> Result<(), RollbackError> {
        let mut rollback = ReEnable {
            mode: self.mode,
            units: Vec::new(),
        };
        for unit in &self.units {
            super::enable(unit, self.mode)?;
            rollback.units.push(unit.clone());
        }
        Ok(())
    }

    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Re-enabled",
            Tense::Active => "Re-enabling",
            Tense::Present => "Re-enable",
            Tense::Future => "Will re-enable",
        };
        format!(
            "{verb} the {} services that spawned the original file",
            self.mode
        )
    }
}

struct Disable {
    units: Vec<OsString>,
    mode: Mode,
}

impl InstallStep for Disable {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "disabled",
            Tense::Active => "disabling",
            Tense::Present => "disable",
            Tense::Future => "Will disable",
        };
        format!(
            "{verb} the {} services running the file at the target location",
            self.mode
        )
    }

    fn describe_detailed(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "disabled",
            Tense::Active => "disabling",
            Tense::Present => "disable",
            Tense::Future => "Will disable",
        };
        #[allow(clippy::format_collect)]
        let services: String = self
            .units
            .iter()
            .map(|unit| unit.to_string_lossy().to_string())
            .map(|unit| format!("\n|\t- {unit}"))
            .collect();

        format!(
            "{verb} the {} services running the file at the target location\n|\tservices:{services}", self.mode
        )
    }

    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError> {
        let mut rollback = Box::new(ReEnable {
            mode: self.mode,
            units: Vec::new(),
        });
        for unit in &self.units {
            super::enable(unit, self.mode).map_err(super::Error::SystemCtl)?;
            rollback.units.push(unit.clone());
        }
        let rollback = rollback as Box<dyn RollbackStep>;
        Ok(Some(rollback))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DisableError {
    #[error("Could not find the service: {0}")]
    CouldNotFindIt(#[from] FindError),
}

pub(crate) fn disable_step(
    target: &Path,
    mode: Mode,
) -> Result<Vec<Box<dyn InstallStep>>, DisableError> {
    let units = find_services_spawning(target, mode)?;
    let step = Box::new(Disable { units, mode });
    let step = step as Box<dyn InstallStep>;
    Ok(vec![step])
}

fn find_services_spawning(target: &Path, mode: Mode) -> Result<Vec<OsString>, FindError> {
    let mut units = Vec::new();
    let path = match mode {
        Mode::User => user_path().unwrap(),
        Mode::System => system_path(),
    };

    collect_units_into(&mut units, &path)?;

    let (units, errs): (Vec<_>, Vec<_>) = units
        .into_iter()
        .map(exe_path)
        .filter_ok(|path| path == target)
        .map_ok(|path| {
            path.file_name()
                .expect("collected units end in .service")
                .to_owned()
        })
        .partition_result();

    if units.is_empty() && !errs.is_empty() {
        Err(FindError::Errors(errs))
    } else {
        Ok(units)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FindError {
    #[error(
        "No service spawning the target file found, could not parse some services however: {0:#?}"
    )]
    Errors(Vec<FindExeError>),
    #[error("Could not read directory")]
    CouldNotReadDir(#[from] std::io::Error),
}

fn collect_units_into(units: &mut Vec<PathBuf>, dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_units_into(units, &path)?;
            } else if path.is_file() && path.ends_with(".service") {
                units.push(path);
            }
        }
    }
    Ok(())
}
