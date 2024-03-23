use std::fs;
use std::path::PathBuf;

use crate::install::init::Steps;
use crate::install::Mode;
use crate::Step;

use super::{disable, Error};

struct RemoveService {
    path: PathBuf,
}

impl Step for RemoveService {
    fn describe(&self, tense: crate::Tense) -> String {
        todo!()
    }

    fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        fs::remove_file(self.path)
            .map_err(Error::Removing)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

struct DisableService {
    name: String,
    mode: Mode,
}

impl Step for DisableService {
    fn describe(&self, tense: crate::Tense) -> String {
        todo!()
    }

    fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        let name = self.name + ".service";
        disable(&name, self.mode)
            .map_err(Error::SystemCtl)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

struct RemoveTimer {
    path: PathBuf,
}

impl Step for RemoveTimer {
    fn describe(&self, tense: crate::Tense) -> String {
        todo!()
    }

    fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        fs::remove_file(self.path)
            .map_err(Error::Removing)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

struct DisableTimer {
    name: String,
    mode: Mode,
}

impl Step for DisableTimer {
    fn describe(&self, tense: crate::Tense) -> String {
        todo!()
    }

    fn perform(self) -> Result<(), Box<dyn std::error::Error>> {
        let name = self.name + ".timer";
        disable(&name, self.mode)
            .map_err(Error::SystemCtl)
            .map_err(Box::new)
            .map_err(Into::into)
    }
}

pub(crate) fn remove_then_disable_service(service_path: PathBuf, name: &str, mode: Mode) -> Steps {
    vec![
        Box::new(RemoveService { path: service_path }),
        Box::new(DisableService {
            name: name.to_owned(),
            mode,
        }),
    ]
}

pub(crate) fn remove_then_disable_timer(timer_path: PathBuf, name: &str, mode: Mode) -> Steps {
    vec![
        Box::new(RemoveTimer { path: timer_path }),
        Box::new(DisableTimer {
            name: name.to_owned(),
            mode,
        }),
    ]
}
