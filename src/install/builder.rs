use std::collections::HashMap;
use std::fmt::Display;
use std::marker::PhantomData;
use std::path::PathBuf;

use crate::schedule::Schedule;

use super::{init, Mode};

pub struct PathIsSet;
pub struct PathNotSet;
impl ToAssign for PathIsSet {}
impl ToAssign for PathNotSet {}

pub struct NameIsSet;
pub struct NameNotSet;
impl ToAssign for NameIsSet {}
impl ToAssign for NameNotSet {}

pub struct TriggerIsSet;
pub struct TriggerNotSet;
impl ToAssign for TriggerIsSet {}
impl ToAssign for TriggerNotSet {}

pub struct InstallTypeNotSet;
impl ToAssign for InstallTypeNotSet {}

pub struct UserInstall;
pub struct SystemInstall;
impl ToAssign for SystemInstall {}
impl ToAssign for UserInstall {}

pub trait ToAssign {}

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
    pub(crate) service_name: Option<String>,
    pub(crate) trigger: Option<Trigger>,
    pub(crate) description: Option<String>,
    pub(crate) working_dir: Option<PathBuf>,
    pub(crate) run_as: Option<String>,
    pub(crate) args: Vec<String>,
    /// key: Environmental variable, value: the value for that variable
    pub(crate) environment: HashMap<String, String>,
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
/// # Example
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # macro_rules! install_system {
/// #     () => {
/// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
/// #     };
/// # }
/// #
/// install_system!()
///     .current_exe()?
///     .service_name("cli")
///     .on_boot()
///     .prepare_install()?
///     .install()?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! install_system {
    () => {
        service_install::install::Spec::__dont_use_use_the_macro_system(env!("CARGO_BIN_NAME"))
    };
}

/// Create a new [`Spec`] for an installation for the current user only
/// # Example
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # macro_rules! install_user {
/// #     () => {
/// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
/// #     };
/// # }
/// #
/// install_user!()
///     .current_exe()?
///     .service_name("cli")
///     .on_boot()
///     .prepare_install()?
///     .install()?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! install_user {
    () => {
        service_install::install::Spec::__dont_use_use_the_macro_user(env!("CARGO_BIN_NAME"))
    };
}

impl Spec<PathNotSet, NameNotSet, TriggerNotSet, InstallTypeNotSet> {
    #[doc(hidden)]
    /// This is an implementation detail and *should not* be called directly!
    pub fn __dont_use_use_the_macro_system(
        bin_name: &'static str,
    ) -> Spec<PathNotSet, NameNotSet, TriggerNotSet, SystemInstall> {
        Spec {
            mode: Mode::System,
            path: None,
            service_name: None,
            trigger: None,
            description: None,
            working_dir: None,
            run_as: None,
            args: Vec::new(),
            environment: HashMap::new(),
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
    ) -> Spec<PathNotSet, NameNotSet, TriggerNotSet, UserInstall> {
        Spec {
            mode: Mode::User,
            path: None,
            service_name: None,
            trigger: None,
            description: None,
            working_dir: None,
            run_as: None,
            args: Vec::new(),
            environment: HashMap::new(),
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
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_system {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_system("doctest")
    /// #     };
    /// # }
    /// #
    /// install_system!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .run_as("David")
    ///     .on_boot()
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
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
    /// Install a copy of the currently running executable.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .path("path/to/binary/weather_checker")
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn path(self, path: impl Into<PathBuf>) -> Spec<PathIsSet, Name, TriggerSet, InstallType> {
        Spec {
            mode: self.mode,
            path: Some(path.into()),
            service_name: self.service_name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            environment: self.environment,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    /// Install a copy of the currently running executable.
    ///
    /// # Errors
    /// Will return an error if the path to the current executable could not be gotten.
    /// This can fail for a number of reasons such as filesystem operations and system call
    /// failures.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn current_exe(
        self,
    ) -> Result<Spec<PathIsSet, Name, TriggerSet, InstallType>, std::io::Error> {
        Ok(Spec {
            mode: self.mode,
            path: Some(std::env::current_exe()?),
            service_name: self.service_name,
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            environment: self.environment,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        })
    }

    /// Name to give the systemd service or cron job
    ///
    /// Only needed for *install*. During uninstall we recognize
    /// the service or con job by the special comment service-install leaves
    /// at the top of each
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn service_name(
        self,
        service_name: impl Display,
    ) -> Spec<Path, NameIsSet, TriggerSet, InstallType> {
        Spec {
            mode: self.mode,
            path: self.path,
            service_name: Some(service_name.to_string()),
            trigger: self.trigger,
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            environment: self.environment,
            bin_name: self.bin_name,
            overwrite_existing: self.overwrite_existing,
            init_systems: self.init_systems,

            path_set: PhantomData {},
            name_set: PhantomData {},
            trigger_set: PhantomData {},
            install_type: PhantomData {},
        }
    }

    /// Start the job on at a certain time every day. See the [Schedule] docs for
    /// how to configure the time.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// use time::Time;
    /// use service_install::Schedule;
    ///
    /// let schedule = Schedule::Daily(Time::from_hms(10, 42, 0).unwrap());
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_schedule(schedule)
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn on_schedule(self, schedule: Schedule) -> Spec<Path, Name, TriggerIsSet, InstallType> {
        Spec {
            mode: self.mode,
            path: self.path,
            service_name: self.service_name,
            trigger: Some(Trigger::OnSchedule(schedule)),
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            environment: self.environment,
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
    /// to be rebooted before the service is started. On systemd its started
    /// immediately.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn on_boot(self) -> Spec<Path, Name, TriggerIsSet, InstallType> {
        Spec {
            mode: self.mode,
            path: self.path,
            service_name: self.service_name,
            trigger: Some(Trigger::OnBoot),
            description: self.description,
            working_dir: self.working_dir,
            run_as: self.run_as,
            args: self.args,
            environment: self.environment,
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
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .description("Sends a notification if a storm is coming")
    ///     .on_boot()
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn description(mut self, description: impl Display) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Should the installer overwrite existing files? Default is false
    ///
    /// Note: we do not even try replace a value if the installed and to be installed
    /// files are identical. This setting only applies to scenarios where there are
    /// files taking up the install location that are different to what would be installed.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .overwrite_existing(true)
    ///     .on_boot()
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn overwrite_existing(mut self, overwrite: bool) -> Self {
        self.overwrite_existing = overwrite;
        self
    }

    /// The args will be shell escaped. If any arguments where already set
    /// this adds to them
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .args(["check", "--location", "North Holland"])
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// The argument will be shell escaped. This does not clear previous set
    /// arguments but adds to it
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .arg("check")
    ///     .arg("--location")
    ///     .arg("North Holland")
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Environmental variables passed to the program when it runs. If you set the
    /// same variable multiple times only the last value will be set for the program.
    ///
    /// # Panics
    /// If any part of any of the environmental variables pairs contains an equal sign.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("cli")
    ///     .on_boot()
    ///     .env_vars([("WAYLAND_display","wayland-1"), ("SHELL", "/bin/bash")])
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn env_vars<S: Into<String>>(mut self, args: impl IntoIterator<Item = (S, S)>) -> Self {
        let vars = args
            .into_iter()
            .map(|(a, b)| (a.into(), b.into()))
            .inspect(|(var, _)| {
                assert!(
                    var.contains(['=']),
                    "The 'key' of environmental variables may not contain an equal sign"
                )
            });
        self.environment.extend(vars);
        self
    }

    /// Environmental variable passed to the program when it runs. If you set the
    /// same variable multiple times only the last value will be set for the program.
    ///
    /// # Panics
    /// If any part of any of the environmental variables pairs contains an equal sign.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("cli")
    ///     .on_boot()
    ///     .env_var("SHELL", "/bin/bash")
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn env_var(mut self, variable: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(variable.into(), value.into());
        self
    }

    /// The working directory of the program when it is started on a schedule.
    /// Can be a relative path. Shell variables like ~ and $Home are not expanded.
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .working_dir("/home/david/.local/share/weather_checker")
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// By default all supported init systems will be tried
    /// Can be set multiple times to try multiple init systems in the
    /// order in which this was set.
    ///
    /// Note: setting this for an uninstall might cause it to fail
    ///
    /// # Example
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # macro_rules! install_user {
    /// #     () => {
    /// #         service_install::install::Spec::__dont_use_use_the_macro_user("doctest")
    /// #     };
    /// # }
    /// #
    /// use service_install::install::init;
    /// install_user!()
    ///     .current_exe()?
    ///     .service_name("weather_checker")
    ///     .on_boot()
    ///     .allowed_inits([init::System::Systemd])
    ///     .prepare_install()?
    ///     .install()?;
    /// # Ok(())
    /// # }
    pub fn allowed_inits(mut self, allowed: impl AsRef<[init::System]>) -> Self {
        self.init_systems = Some(allowed.as_ref().to_vec());
        self
    }
}
