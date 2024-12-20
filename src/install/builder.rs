use std::fmt::Display;
use std::marker::PhantomData;
use std::path::PathBuf;

use crate::schedule::Schedule;

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

impl ToAssign for Set {}
impl ToAssign for NotSet {}
impl ToAssign for SystemInstall {}
impl ToAssign for UserInstall {}

#[derive(Debug, Clone)]
pub(crate) enum Trigger {
    OnSchedule(Schedule),
    OnBoot,
}

/// The configuration for the current install, needed to perform the
/// installation or remove an existing one. Create this by using the
/// [`install_system`](crate::install_system) or
/// [`install_user`](crate::install_user) macros.
#[must_use]
#[derive(Debug)]
pub struct Spec<Path, Name, TriggerSet, InstallType>
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
    pub(crate) overwrite_existing: bool,
    /// None means all
    pub(crate) init_systems: Option<Vec<init::System>>,

    pub(crate) path_set: PhantomData<Path>,
    pub(crate) name_set: PhantomData<Name>,
    pub(crate) trigger_set: PhantomData<TriggerSet>,
    pub(crate) install_type: PhantomData<InstallType>,
}

/// Create a new [`Spec`] for a system wide installation
#[macro_export]
macro_rules! install_system {
    () => {
        service_install::install::Spec::__dont_use_use_the_macro_system(env!("CARGO_BIN_NAME"))
    };
}

/// Create a new [`Spec`] for an installation for the current user only
#[macro_export]
macro_rules! install_user {
    () => {
        service_install::install::Spec::__dont_use_use_the_macro_user(env!("CARGO_BIN_NAME"))
    };
}

impl Spec<NotSet, NotSet, NotSet, NotSet> {
    #[doc(hidden)]
    /// This is an implementation detail and *should not* be called directly!
    pub fn __dont_use_use_the_macro_system(
        bin_name: &'static str,
    ) -> Spec<NotSet, NotSet, NotSet, SystemInstall> {
        Spec {
            mode: Mode::System,
            path: None,
            name: None,
            trigger: None,
            description: None,
            working_dir: None,
            run_as: None,
            args: Vec::new(),
            bin_name,
            overwrite_existing: false,
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
    ) -> Spec<NotSet, NotSet, NotSet, UserInstall> {
        Spec {
            mode: Mode::User,
            path: None,
            name: None,
            trigger: None,
            description: None,
            working_dir: None,
            run_as: None,
            args: Vec::new(),
            bin_name,
            overwrite_existing: false,
            init_systems: None,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }
}

impl<Path, Name, TriggerSet> Spec<Path, Name, TriggerSet, SystemInstall>
where
    Path: ToAssign,
    Name: ToAssign,
    TriggerSet: ToAssign,
{
    /// Only available for [`install_system`](crate::install_system)
    pub fn run_as(mut self, user: impl Into<String>) -> Self {
        self.run_as = Some(user.into());
        self
    }
}

impl<Path, Name, TriggerSet, InstallType> Spec<Path, Name, TriggerSet, InstallType>
where
    Path: ToAssign,
    Name: ToAssign,
    TriggerSet: ToAssign,
    InstallType: ToAssign,
{
    pub fn path(self, path: impl Into<PathBuf>) -> Spec<Set, Name, TriggerSet, InstallType> {
        Spec {
            mode: self.mode,
            path: Some(path.into()),
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    /// Install a copy of the currently running exe.
    ///
    /// # Errors
    /// Will return an error if the path to the current executable could not be gotten.
    /// This can fail for a number of reasons such as filesystem operations and syscall
    /// failures.
    pub fn current_exe(self) -> Result<Spec<Set, Name, TriggerSet, InstallType>, std::io::Error> {
        Ok(Spec {
            mode: self.mode,
            path: Some(std::env::current_exe()?),
            name: self.name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        })
    }

    pub fn name(self, name: impl Display) -> Spec<Path, Set, TriggerSet, InstallType> {
        Spec {
            mode: self.mode,
            path: self.path,
            name: Some(name.to_string()),
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    pub fn on_schedule(self, schedule: Schedule) -> Spec<Path, Name, Set, InstallType> {
        Spec {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: Some(Trigger::OnSchedule(schedule)),
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    /// Start the job on boot. When cron is used as init the system needs 
    /// to be rebooted before this applies
    pub fn on_boot(self) -> Spec<Path, Name, Set, InstallType> {
        Spec {
            mode: self.mode,
            path: self.path,
            name: self.name,
            trigger: Some(Trigger::OnBoot),
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    /// The description for the installed service
    pub fn description(mut self, description: impl Display) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Should the installer overwrite existing files? Default is false
    pub fn overwrite_existing(mut self, overwrite: bool) -> Self {
        self.overwrite_existing = overwrite;
        self
    }

    /// These args will be shell escaped. If any arguments where already set
    /// this adds to them
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// The argument will be shell escaped. This does not clear previous set
    /// arguments but adds to it
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
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
