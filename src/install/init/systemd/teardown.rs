use std::fs;
use std::path::PathBuf;

use crate::install::init::RSteps;
use crate::install::Mode;
use crate::install::RemoveStep;
use crate::install::Tense;

use super::{disable, Error};

pub(crate) struct RemoveService {
    pub(crate) path: PathBuf,
}

impl RemoveStep for RemoveService {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Present => "Removing",
            Tense::Future => "Will remove",
        };
        let path = self.path.display();
        format!("{verb} systemd service unit at:\n\t{path}")
    }

    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        fs::remove_file(&self.path)
            .map_err(Error::Removing)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

pub(crate) struct DisableService {
    pub(crate) name: String,
    pub(crate) mode: Mode,
}

impl RemoveStep for DisableService {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Disabled",
            Tense::Present => "Disabling",
            Tense::Future => "Will disable",
        };
        format!("{verb} systemd {} service: {}", self.mode, self.name)
    }

    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let name = self.name.clone() + ".service";
        disable(&name, self.mode)
            .map_err(Error::SystemCtl)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

pub(crate) struct RemoveTimer {
    pub(crate) path: PathBuf,
}

impl RemoveStep for RemoveTimer {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Present => "Removing",
            Tense::Future => "Will remove",
        };
        let path = self.path.display();
        format!("{verb} systemd timer at:\n\t{path}")
    }

    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        fs::remove_file(self.path.clone())
            .map_err(Error::Removing)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

pub(crate) struct DisableTimer {
    pub(crate) name: String,
    pub(crate) mode: Mode,
}

impl RemoveStep for DisableTimer {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Disabled",
            Tense::Present => "Disabling",
            Tense::Future => "Will disable",
        };
        format!("{verb} systemd {} timer: {}", self.mode, self.name)
    }

    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let name = self.name.clone() + ".timer";
        disable(&name, self.mode)
            .map_err(Error::SystemCtl)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

pub(crate) fn disable_then_remove_service(service_path: PathBuf, name: &str, mode: Mode) -> RSteps {
    vec![
        Box::new(DisableService {
            name: name.to_owned(),
            mode,
        }),
        Box::new(RemoveService { path: service_path }),
    ]
}

pub(crate) fn disable_then_remove_with_timer(
    timer_path: PathBuf,
    name: &str,
    mode: Mode,
) -> RSteps {
    vec![
        Box::new(DisableTimer {
            name: name.to_owned(),
            mode,
        }),
        Box::new(RemoveTimer { path: timer_path }),
    ]
}
