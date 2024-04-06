use std::fmt::Display;
use std::marker::PhantomData;
use std::path::PathBuf;

use crate::Schedule;

use super::{init, Mode};

#[derive(Debug)]
pub struct Set;
#[derive(Debug)]
pub struct NotSet;

#[derive(Debug)]
pub struct UserInstall;
#[derive(Debug)]
pub struct SystemInstall;

pub trait ToAssign {}
pub trait Assigned: ToAssign {}
pub trait NotAssigned: ToAssign {}

impl ToAssign for Set {}
impl ToAssign for NotSet {}
impl ToAssign for SystemInstall {}
impl ToAssign for UserInstall {}


#[derive(Debug, Clone)]
pub(crate) enum Trigger {
    OnSchedule(Schedule),
    OnBoot,
}

#[must_use]
#[derive(Debug)]
pub struct InstallSpec<Path, Name, TriggerSet, InstallType>
where
    Path: ToAssign,
    Name: ToAssign,
    TriggerSet: ToAssign,
    InstallType: ToAssign,
{
    pub(crate) mode: Mode,
    pub(crate) path: Option<PathBuf>,
    pub(crate) name: Option<String>,
    pub(crate) trigger: Option<Trigger>,
    pub(crate) description: Option<String>,
    pub(crate) working_dir: Option<PathBuf>,
    pub(crate) run_as: Option<String>,
    pub(crate) args: Vec<String>,
    pub(crate) bin_name: &'static str,
    /// None means all
    pub(crate) init_systems: Option<Vec<init::System>>,

    pub(crate) path_set: PhantomData<Path>,
    pub(crate) name_set: PhantomData<Name>,
    pub(crate) trigger_set: PhantomData<TriggerSet>,
    pub(crate) install_type: PhantomData<InstallType>,
}

#[macro_export]
macro_rules! install_system {
    () => {
        service_install::InstallSpec::__dont_use_use_the_macro_system(env!("CARGO_BIN_NAME"))
    };
}

#[macro_export]
macro_rules! install_user {
    () => {
        service_install::InstallSpec::__dont_use_use_the_macro_user(env!("CARGO_BIN_NAME"))
    };
}

impl InstallSpec<NotSet, NotSet, NotSet, NotSet> {
    #[doc(hidden)]
    /// This is an implementation detail and *should not* be called directly!
    pub fn __dont_use_use_the_macro_system(
        bin_name: &'static str,
    ) -> InstallSpec<NotSet, NotSet, NotSet, SystemInstall> {
        InstallSpec {
            mode: Mode::System,
            path: None,
            name: None,
            trigger: None,
            description: None,
            working_dir: None,
            run_as: None,
            args: Vec::new(),
            bin_name,
            init_systems: None,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    #[doc(hidden)]
    /// This is an implementation detail and *should not* be called directly!
    pub fn __dont_use_use_the_macro_user(
        bin_name: &'static str,
    ) -> InstallSpec<NotSet, NotSet, NotSet, UserInstall> {
        InstallSpec {
            mode: Mode::User,
            path: None,
            name: None,
            trigger: None,
            description: None,
            working_dir: None,
            run_as: None,
            args: Vec::new(),
            bin_name,
            init_systems: None,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }
}

impl<Path, Name, TriggerSet> InstallSpec<Path, Name, TriggerSet, SystemInstall>
where
    Path: ToAssign,
    Name: ToAssign,
    TriggerSet: ToAssign,
{
    /// Only available for Install::system
    pub fn run_as(mut self, user: impl Into<String>) -> Self {
        self.run_as = Some(user.into());
        self
    }
}

impl<Path, Name, TriggerSet, InstallType> InstallSpec<Path, Name, TriggerSet, InstallType>
where
    Path: ToAssign,
    Name: ToAssign,
    TriggerSet: ToAssign,
    InstallType: ToAssign,
{
    pub fn path(self, path: impl Into<PathBuf>) -> InstallSpec<Set, Name, TriggerSet, InstallType> {
        InstallSpec {
            mode: self.mode,
            path: Some(path.into()),
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    pub fn current_exe(
        self,
    ) -> Result<InstallSpec<Set, Name, TriggerSet, InstallType>, std::io::Error> {
        Ok(InstallSpec {
            mode: self.mode,
            path: Some(std::env::current_exe()?),
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        })
    }

    pub fn name(self, name: impl Display) -> InstallSpec<Path, Set, TriggerSet, InstallType> {
        InstallSpec {
            mode: self.mode,
            path: self.path,
            name: Some(name.to_string()),
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    pub fn on_schedule(self, schedule: Schedule) -> InstallSpec<Path, Name, Set, InstallType> {
        InstallSpec {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: Some(Trigger::OnSchedule(schedule)),
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    pub fn on_boot(self) -> InstallSpec<Path, Name, Set, InstallType> {
        InstallSpec {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: Some(Trigger::OnBoot),
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    pub fn description(mut self, description: impl Display) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// These args will be shell escaped
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// By default all supported init systems will be tried
    /// Can be set multiple times to try multiple init systems in the
    /// order in which this was set.
    ///
    /// Note: setting this for an uninstall might cause it to fail
    pub fn allowed_inits(mut self, allowed: impl AsRef<[init::System]>) -> Self {
        self.init_systems = Some(allowed.as_ref().to_vec());
        self
    }
}
