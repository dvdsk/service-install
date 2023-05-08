use std::marker::PhantomData;
use std::path::PathBuf;

use crate::Schedule;

use super::Mode;

#[derive(Debug, Default)]
pub struct Set;
#[derive(Debug, Default)]
pub struct NotSet;

pub trait ToAssign: core::fmt::Debug {}
pub trait Assigned: ToAssign {}
pub trait NotAssigned: ToAssign {}

impl ToAssign for Set {}
impl ToAssign for NotSet {}

#[derive(Debug, Clone)]
pub(crate) enum Trigger {
    OnSchedual(Schedule),
    OnBoot,
}

pub struct Install<Path, Name, TriggerSet>
where
    Path: ToAssign,
    Name: ToAssign,
    TriggerSet: ToAssign,
{
    pub(crate) mode: Mode,
    pub(crate) path: Option<PathBuf>,
    pub(crate) name: Option<String>,
    pub(crate) trigger: Option<Trigger>,
    pub(crate) description: Option<String>,
    pub(crate) working_dir: Option<PathBuf>,
    pub(crate) args: Vec<String>,

    pub(crate) path_set: PhantomData<Path>,
    pub(crate) name_set: PhantomData<Name>,
    pub(crate) trigger_set: PhantomData<TriggerSet>,
}

impl Install<NotSet, NotSet, NotSet> {
    #[must_use]
    pub fn system() -> Install<NotSet, NotSet, NotSet> {
        Install {
            mode: Mode::System,
            path: None,
            name: None,
            trigger: None,
            description: None,
            working_dir: None,
            args: Vec::new(),

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }

    #[must_use]
    pub fn user() -> Install<NotSet, NotSet, NotSet> {
        Install {
            mode: Mode::User,
            path: None,
            name: None,
            trigger: None,
            description: None,
            working_dir: None,
            args: Vec::new(),

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }
}

impl<Path, Name, TriggerSet> Install<Path, Name, TriggerSet>
where
    Path: ToAssign,
    Name: ToAssign,
    TriggerSet: ToAssign,
{
    #[must_use]
    pub fn path(self, path: impl Into<PathBuf>) -> Install<Set, Name, TriggerSet> {
        Install {
            mode: self.mode,
            path: Some(path.into()),
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            args: self.args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }

    #[must_use]
    pub fn current_exe(self) -> Result<Install<Set, Name, TriggerSet>, std::io::Error> {
        Ok(Install {
            mode: self.mode,
            path: Some(std::env::current_exe()?),
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            args: self.args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        })
    }

    #[must_use]
    pub fn name(self, name: impl Into<String>) -> Install<Path, Set, TriggerSet> {
        Install {
            mode: self.mode,
            path: self.path,
            name: Some(name.into()),
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            args: self.args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }

    #[must_use]
    pub fn on_schedule(self, schedule: Schedule) -> Install<Path, Name, Set> {
        Install {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: Some(Trigger::OnSchedual(schedule)),
            description: self.description,
            working_dir: self.working_dir,
            args: self.args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }

    #[must_use]
    pub fn on_boot(self) -> Install<Path, Name, Set> {
        Install {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: Some(Trigger::OnBoot),
            description: self.description,
            working_dir: self.working_dir,
            args: self.args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }

    #[must_use]
    pub fn description(self, description: String) -> Install<Path, Name, Set> {
        Install {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: self.trigger,
            description: Some(description),
            working_dir: self.working_dir,
            args: self.args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }

    #[must_use]
    pub fn args(self, args: Vec<String>) -> Install<Path, Name, Set> {
        Install {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }

    #[must_use]
    pub fn working_dir(self, dir: PathBuf) -> Install<Path, Name, Set> {
        Install {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: Some(dir),
            args: self.args,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
        }
    }
}
