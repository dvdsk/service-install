mod builder;

/// Errors and settings related to installing files
pub mod files;
/// Errors and settings related to the supported init systems
pub mod init;

use std::ffi::OsString;
use std::fmt::Display;

pub use builder::Spec;
use itertools::{Either, Itertools};

use crate::Tense;

use self::builder::ToAssign;
use self::init::SetupError;

/// Whether to install system wide or for the current user only
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// install for the current user, does not require running the installation
    /// as superuser/admin
    User,
    /// install to the entire system, the installation/removal must be ran as
    /// superuser/admin or it will return
    /// [`InstallError::NeedRootForSysInstall`] or [`PrepareRemoveError::NeedRoot`]
    System,
}

impl Mode {
    fn is_user(self) -> bool {
        match self {
            Mode::User => true,
            Mode::System => false,
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::User => f.write_str("user"),
            Mode::System => f.write_str("system"),
        }
    }
}

/// Errors that can occur when preparing for or performing an installation
#[allow(clippy::module_name_repetitions)]
#[derive(thiserror::Error, Debug)]
pub enum PrepareInstallError {
    #[error("Error setting up init: {0}")]
    Init(#[from] init::SetupError),
    #[error("Failed to move files: {0}")]
    Move(#[from] files::MoveError),
    #[error("Need to run as root to install to system")]
    NeedRootForSysInstall,
    #[error("Need to run as root to setup service to run as another user")]
    NeedRootToRunAs,
    #[error("Could not find an init system we can set things up for")]
    NoInitSystemRecognized,
    #[error("Install configured to run as a user: `{0}` however this user does not exist")]
    UserDoesNotExist(String),
    #[error("All supported init systems found failed, errors: {0:?}")]
    SupportedInitSystemFailed(Vec<InitSystemFailure>),
}

/// The init system was found and we tried to set up the service but ran into an
/// error.
///
/// When there is another init system that does work this error is ignored. If
/// no other system is available or there is but it/they fail too this error is
/// reported.
///
/// A warning is always issued if the `tracing` feature is enabled.
#[derive(Debug, thiserror::Error)]
#[error("Init system: {name} ran into error: {error}")]
pub struct InitSystemFailure {
    name: String,
    error: SetupError,
}

/// Errors that can occur when preparing for or removing an installation
#[derive(thiserror::Error, Debug)]
pub enum PrepareRemoveError {
    #[error("Could not find this executable's location: {0}")]
    GetExeLocation(std::io::Error),
    #[error("Failed to remove files: {0}")]
    Move(#[from] files::DeleteError),
    #[error("Removing from init system: {0}")]
    Init(#[from] init::TearDownError),
    #[error("Could not find any installation in any init system")]
    NoInstallFound,
    #[error("Need to run as root to remove a system install")]
    NeedRoot,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Could not get crontab, needed to add our lines, error: {0}")]
    GetCrontab(#[from] init::cron::GetCrontabError),
    #[error("{0}")]
    CrontabChanged(#[from] init::cron::teardown::CrontabChanged),
    #[error("Could not set crontab, needed to add our lines, error: {0}")]
    SetCrontab(#[from] init::cron::SetCrontabError),
    #[error("Something went wrong interacting with systemd: {0}")]
    Systemd(#[from] init::systemd::Error),
    #[error("Could not copy executable: {0}")]
    CopyExe(std::io::Error),
    #[error("Could not set the owner of the installed executable to be root: {0}")]
    SetRootOwner(std::io::Error),
    #[error("Could not make the installed executable read only: {0}")]
    SetReadOnly(#[from] files::SetReadOnlyError),
}

/// One step in the install process. Can be executed or described.
#[allow(clippy::module_name_repetitions)]
pub trait InstallStep {
    /// A short (one line) description of what this performing this step will
    /// do. Pass in the tense you want for the description (past, present or
    /// future)
    fn describe(&self, tense: Tense) -> String;
    /// A verbose description of what performing this step will do to the
    /// system. Includes as many details as possible. Pass in the tense you want
    /// for the description (past, present or future)
    fn describe_detailed(&self, tense: Tense) -> String {
        self.describe(tense)
    }
    /// Perform this install step making a change to the system. This may return
    /// a [`RollbackStep`] that can be used to undo the change made in the
    /// future. This can be used in an install wizard to roll back changes when
    /// an error happens.
    ///
    /// # Errors
    /// The system can change between preparing to install and actually
    /// installing. For example all disk space could be used. Or the install
    /// could run into an error that was not checked for while preparing. If you
    /// find this happens please make an issue.
    fn perform(&mut self) -> Result<Option<Box<dyn RollbackStep>>, InstallError>;
}

impl std::fmt::Debug for &dyn InstallStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe(Tense::Future))
    }
}

impl Display for &dyn InstallStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe_detailed(Tense::Future))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveError {
    #[error("Could not get crontab, needed tot filter out our added lines, error: {0}")]
    GetCrontab(#[from] init::cron::GetCrontabError),
    #[error("{0}")]
    CrontabChanged(#[from] init::cron::teardown::CrontabChanged),
    #[error("Could not set crontab, needed tot filter out our added lines, error: {0}")]
    SetCrontab(#[from] init::cron::SetCrontabError),
    #[error("Could not remove file(s), error: {0}")]
    DeleteError(#[from] files::DeleteError),
    #[error("Something went wrong interacting with systemd: {0}")]
    Systemd(#[from] init::systemd::Error),
}

/// One step in the remove process. Can be executed or described.
pub trait RemoveStep {
    /// A short (one line) description of what this performing this step will
    /// do. Pass in the tense you want for the description (past, present or future)
    fn describe(&self, tense: Tense) -> String;
    /// A verbose description of what performing this step will do to the
    /// system. Includes as many details as possible. Pass in the tense you want
    /// for the description (past, present or future)
    fn describe_detailed(&self, tense: Tense) -> String {
        self.describe(tense)
    }
    /// Executes this remove step. This can be used when building an
    /// uninstall/remove wizard. For example to ask the user confirmation
    /// before each step.
    ///
    /// # Errors
    /// The system can change between preparing to remove and actually removing
    /// the install. For example a file could have been removed by the user of
    /// the system. Or the removal could run into an error that was not checked
    /// for while preparing. If you find this happens please make an issue.
    fn perform(&mut self) -> Result<(), RemoveError>;
}

impl std::fmt::Debug for &dyn RemoveStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe(Tense::Future))
    }
}

impl Display for &dyn RemoveStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe_detailed(Tense::Future))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RollbackError {
    #[error("Could not rollback error while removing: {0}")]
    Removing(#[from] RemoveError),
    #[error("Can not rollback setting up cron, must be done manually")]
    Impossible,
}

/// Undoes a [`InstallStep`]. Can be executed or described.
pub trait RollbackStep {
    /// Executes this rollback step. This can be used when building an install
    /// wizard. You can [`describe()`](RollbackStep::describe) and then ask the
    /// end user if the want to perform it.
    ///
    /// # Errors
    /// The system could have changed between the install and the rollback.
    /// Leading to various errors, mostly IO.
    fn perform(&mut self) -> Result<(), RollbackError>;
    fn describe(&self, tense: Tense) -> String;
}

impl std::fmt::Debug for &dyn RollbackStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe(Tense::Future))
    }
}

impl Display for &dyn RollbackStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe(Tense::Future))
    }
}

impl<T: RemoveStep> RollbackStep for T {
    fn perform(&mut self) -> Result<(), RollbackError> {
        Ok(self.perform()?)
    }

    fn describe(&self, tense: Tense) -> String {
        self.describe(tense)
    }
}

/// Changes to the system that need to be applied to do the installation.
///
/// Returned by [`Spec::prepare_install`].Use
/// [`install()`](InstallSteps::install) to apply all changes at once. This
/// implements [`IntoIterator`] yielding [`InstallSteps`](InstallStep). These
/// steps can be described possibly in detail and/or performed one by one.
#[allow(clippy::module_name_repetitions)]
pub struct InstallSteps(pub(crate) Vec<Box<dyn InstallStep>>);

impl std::fmt::Debug for InstallSteps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for step in self.0.iter().map(|step| step.describe(Tense::Future)) {
            write!(f, "{step\n}")?;
        }
        Ok(())
    }
}

impl Display for InstallSteps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for step in self
            .0
            .iter()
            .map(|step| step.describe_detailed(Tense::Future))
        {
            write!(f, "{step\n}")?;
        }
        Ok(())
    }
}

impl IntoIterator for InstallSteps {
    type Item = Box<dyn InstallStep>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl InstallSteps {
    /// Perform all steps needed to install.
    ///
    /// # Errors
    /// The system can change between preparing to install and actually
    /// installing. For example all disk space could be used. Or the install
    /// could run into an error that was not checked for while preparing. If you
    /// find this happens please make an issue.
    pub fn install(self) -> Result<String, Box<dyn std::error::Error>> {
        let mut description = Vec::new();
        for mut step in self.0 {
            description.push(step.describe(Tense::Past));
            step.perform()?;
        }

        Ok(description.join("\n"))
    }
}

impl<T: ToAssign> Spec<builder::Set, builder::Set, builder::Set, T> {
    /// Prepare for installing. This makes a number of checks and if they are
    /// passed it returns the [`InstallSteps`]. These implement [`IntoIterator`] and
    /// can be inspected and executated one by one or executed in one step using
    /// [`InstallSteps::install`].
    ///
    /// # Errors
    /// Returns an error if:
    ///  - the install is set to be system wide install while not running as admin/superuser
    ///  - the service should run as another user then the current one while not running as admin/superuser
    ///  - the service should run for a nonexisting user
    ///  - no suitable install directory could be found
    ///  - the path for the executable does not point to a file
    pub fn prepare_install(self) -> Result<InstallSteps, PrepareInstallError> {
        let builder::Spec {
            mode,
            path: Some(source),
            name: Some(name),
            bin_name,
            args,
            trigger: Some(trigger),
            overwrite_existing,
            working_dir,
            run_as,
            description,
            ..
        } = self
        else {
            unreachable!("type sys guarantees path, name and trigger set")
        };

        let not_root = matches!(sudo::check(), sudo::RunningAs::User);
        if let Mode::System = mode {
            if not_root {
                return Err(PrepareInstallError::NeedRootForSysInstall);
            }
        }

        if let Some(ref user) = run_as {
            let curr_user = uzers::get_current_username()
                .ok_or_else(|| PrepareInstallError::UserDoesNotExist(user.clone()))?;
            if curr_user != OsString::from(user) && not_root {
                return Err(PrepareInstallError::NeedRootToRunAs);
            }
        }

        let (mut steps, exe_path) = files::move_files(source, mode, overwrite_existing)?;
        let params = init::Params {
            name,
            bin_name,
            description,

            exe_path,
            exe_args: args,
            working_dir,

            trigger,
            run_as,
            mode,
        };

        let mut errors = Vec::new();
        for init in self.init_systems.unwrap_or_else(init::System::all) {
            if init.not_available().map_err(PrepareInstallError::Init)? {
                continue;
            }

            match init.set_up_steps(&params) {
                Ok(init_steps) => {
                    steps.extend(init_steps);
                    return Ok(InstallSteps(steps));
                }
                Err(err) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Could not set up init using {}, error: {err}", init.name());
                    errors.push(InitSystemFailure {
                        name: init.name().to_owned(),
                        error: err,
                    });
                }
            };
        }

        if errors.is_empty() {
            Err(PrepareInstallError::NoInitSystemRecognized)
        } else {
            Err(PrepareInstallError::SupportedInitSystemFailed(errors))
        }
    }
}

/// Changes to the system that need to be applied to remove the installation.
///
/// Returned by [`Spec::prepare_remove`].Use
/// [`remove()`](RemoveSteps::remove) to apply all changes at once. This
/// implements [`IntoIterator`] yielding [`RemoveSteps`](RemoveStep). These
/// steps can be described possibly in detail and/or performed one by one.
pub struct RemoveSteps(pub(crate) Vec<Box<dyn RemoveStep>>);

impl std::fmt::Debug for RemoveSteps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for step in self.0.iter().map(|step| step.describe(Tense::Future)) {
            write!(f, "{step\n}")?;
        }
        Ok(())
    }
}

impl Display for RemoveSteps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for step in self
            .0
            .iter()
            .map(|step| step.describe_detailed(Tense::Future))
        {
            write!(f, "{step\n}")?;
        }
        Ok(())
    }
}

impl IntoIterator for RemoveSteps {
    type Item = Box<dyn RemoveStep>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl RemoveSteps {
    /// Perform all steps needed to remove an installation. Report what was done
    /// at the end. Aborts on error.
    ///
    /// # Errors
    /// The system can change between preparing to remove and actually removing
    /// the install. For example a file could have been removed by the user of
    /// the system. Or the removal could run into an error that was not checked
    /// for while preparing. If you find this happens please make an issue.
    pub fn remove(self) -> Result<String, Box<dyn std::error::Error>> {
        let mut description = Vec::new();
        for mut step in self.0 {
            description.push(step.describe(Tense::Past));
            step.perform()?;
        }

        Ok(description.join("\n"))
    }

    /// Perform all steps needed to remove an installation. If any fail keep
    /// going. Collect all the errors and report them at the end.
    ///
    /// # Errors
    /// The system can change between preparing to remove and actually removing
    /// the install. For example a file could have been removed by the user of
    /// the system. Or the removal could run into an error that was not checked
    /// for while preparing. If you find this happens please make an issue.
    pub fn best_effort_remove(self) -> Result<String, BestEffortRemoveError> {
        let (description, failures): (Vec<_>, Vec<_>) =
            self.0
                .into_iter()
                .partition_map(|mut step| match step.perform() {
                    Ok(()) => Either::Left(step.describe(Tense::Past)),
                    Err(e) => Either::Right((step.describe_detailed(Tense::Active), e)),
                });

        if failures.is_empty() {
            Ok(description.join("\n"))
        } else {
            Err(BestEffortRemoveError { failures })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub struct BestEffortRemoveError {
    failures: Vec<(String, RemoveError)>,
}

impl Display for BestEffortRemoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Ran into one or more issues trying to remove an install")?;
        writeln!(f, "You should resolve/check these issues manually")?;
        for (task, error) in &self.failures {
            let task = task.to_lowercase();
            writeln!(f, "* Tried to {task}\nfailed because: {error}")?;
        }
        Ok(())
    }
}

impl<P: ToAssign, T: ToAssign, I: ToAssign> Spec<P, builder::Set, T, I> {
    /// Prepare for removing an install. This makes a number of checks and if
    /// they are passed it returns the [`RemoveSteps`]. These implement
    /// [`IntoIterator`] and can be inspected and executated one by one or
    /// executed in one step using [`RemoveSteps::remove`].
    ///
    /// # Errors
    /// Returns an error if:
    ///  - trying to remove a system install while not running as admin/superuser
    ///  - no install is found
    ///  - anything goes wrong setting up the removal
    pub fn prepare_remove(self) -> Result<RemoveSteps, PrepareRemoveError> {
        let builder::Spec {
            mode,
            name: Some(name),
            bin_name,
            run_as,
            ..
        } = self
        else {
            unreachable!("type sys guarantees name and trigger set")
        };

        if let Mode::System = mode {
            if let sudo::RunningAs::User = sudo::check() {
                return Err(PrepareRemoveError::NeedRoot);
            }
        }

        let mut inits = self.init_systems.unwrap_or(init::System::all()).into_iter();
        let (mut steps, path) = loop {
            let Some(init) = inits.next() else {
                return Err(PrepareRemoveError::NoInstallFound);
            };

            if let Some(install) = init.tear_down_steps(&name, bin_name, mode, run_as.as_deref())? {
                break install;
            }
        };

        let remove_step = files::remove_files(path);
        steps.push(Box::new(remove_step));
        Ok(RemoveSteps(steps))
    }
}
