use std::fs;
use std::path::PathBuf;

use crate::install::init::systemd::api::on_seperate_tokio_thread;
use crate::install::init::RSteps;
use crate::install::Mode;
use crate::install::RemoveError;
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
            Tense::Questioning => "Remove",
            Tense::Future => "Will remove",
            Tense::Active => "Removing",
        };
        let path = self.path.display();
        format!(
            "{verb} systemd service unit{} at:\n|\t{path}",
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<(), RemoveError> {
        fs::remove_file(&self.path).map_err(Error::Removing)?;
        Ok(())
    }
}

pub(crate) struct DisableService {
    pub(crate) name: String,
    pub(crate) mode: Mode,
    pub(crate) stop: bool,
}

impl RemoveStep for DisableService {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Disabled",
            Tense::Questioning => "Disable",
            Tense::Future => "Will disable",
            Tense::Active => "Disabling",
        };
        let stop = if self.stop {
            match tense {
                Tense::Past => "and stopped ",
                Tense::Questioning | Tense::Future => "and stop ",
                Tense::Active => "and stopping ",
            }
        } else {
            ""
        };
        format!(
            "{verb} {stop}systemd {} service: {}{}",
            self.mode,
            self.name,
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<(), RemoveError> {
        let name = self.name.clone() + ".service";
        on_seperate_tokio_thread! {{
            disable(name.as_ref(), self.mode, self.stop).await.map_err(RemoveError::Systemd)
        }}
    }
}

pub(crate) struct RemoveTimer {
    pub(crate) path: PathBuf,
}

impl RemoveStep for RemoveTimer {
    fn describe(&self, tense: Tense) -> String {
        let verb = match tense {
            Tense::Past => "Removed",
            Tense::Questioning => "Remove",
            Tense::Future => "Will remove",
            Tense::Active => "Removing",
        };
        let path = self.path.display();
        format!("{verb} systemd timer{} at:\n|\t{path}", tense.punct())
    }

    fn perform(&mut self) -> Result<(), RemoveError> {
        fs::remove_file(self.path.clone()).map_err(Error::Removing)?;
        Ok(())
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
            Tense::Questioning => "Disable",
            Tense::Future => "Will disable",
            Tense::Active => "Disabling",
        };
        format!(
            "{verb} systemd {} timer: {}{}",
            self.mode,
            self.name,
            tense.punct()
        )
    }

    fn perform(&mut self) -> Result<(), RemoveError> {
        let name = self.name.clone() + ".timer";
        on_seperate_tokio_thread! {{
            disable(name.as_ref(), self.mode, true).await.map_err(RemoveError::Systemd)
        }}?;
        Ok(())
    }
}

pub(crate) fn disable_then_remove_service(service_path: PathBuf, name: &str, mode: Mode) -> RSteps {
    vec![
        Box::new(DisableService {
            name: name.to_owned(),
            mode,
            stop: true,
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
