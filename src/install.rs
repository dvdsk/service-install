mod builder;

/// Errors and settings related to installing files
pub mod files;
/// Errors and settings related to the supported init systems
pub mod init;

use std::ffi::OsString;
use std::fmt::Display;

pub use builder::Spec;
use files::MoveBackError;
use init::systemd;
use itertools::{Either, Itertools};

use crate::Tense;

use self::builder::ToAssign;
use self::init::cron::teardown::CrontabChanged;
use self::init::cron::{GetCrontabError, SetCrontabError};
use self::init::SetupError;

/// Whether to install system wide or for the current user only
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// install for the current user, does not require running the installation
    /// as superuser/admin
    User,
    /// install to the entire system, the installation/removal must be ran as
    /// superuser/admin or it will return
    /// [`PrepareInstallError::NeedRootForSysInstall`] or [`PrepareRemoveError::NeedRoot`]
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
    #[error("Error setting up init")]
    Init(
        #[from]
        #[source]
        init::SetupError,
    ),
    #[error("Failed to move files")]
    Move(
        #[from]
        #[source]
        files::MoveError,
    ),
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
    #[error("Could not find this executable's location")]
    GetExeLocation(#[source] std::io::Error),
    #[error("Failed to remove files")]
    Move(
        #[from]
        #[source]
        files::DeleteError,
    ),
    #[error("Removing from init system")]
    Init(
        #[from]
        #[source]
        init::TearDownError,
    ),
    #[error("Could not find any installation in any init system")]
    NoInstallFound,
    #[error("Need to run as root to remove a system install")]
    NeedRoot,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Could not get crontab, needed to add our lines")]
    GetCrontab(
        #[from]
        #[source]
        init::cron::GetCrontabError,
    ),
    #[error(transparent)]
    CrontabChanged(#[from] init::cron::teardown::CrontabChanged),
    #[error("Could not set crontab, needed to add our lines")]
    SetCrontab(
        #[from]
        #[source]
        init::cron::SetCrontabError,
    ),
    #[error("Something went wrong interacting with systemd")]
    Systemd(
        #[from]
        #[source]
        init::systemd::Error,
    ),
    #[error("Could not set the owner of the installed executable to be root")]
    SetRootOwner(#[source] std::io::Error),
    #[error("Could not make the installed executable read only")]
    SetReadOnly(
        #[from]
        #[source]
        files::SetReadOnlyError,
    ),
    #[error("Can not disable Cron service, process will not stop.")]
    CouldNotStop,
    #[error("Could not kill the process preventing installing the new binary")]
    KillOld(#[source] files::process_parent::KillOldError),
    #[error("Could not copy executable to install location")]
    CopyExeError(#[source] std::io::Error),
    #[error("Failed to make short lived backup of file taking up install location")]
    Backup(#[source] BackupError),
    #[error("Could not spawn a tokio runtime for interacting with systemd")]
    TokioRt(#[source] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("Could not create temporary file")]
    Create(#[source] std::io::Error),
    #[error("Could not write to temporary file")]
    Write(#[source] std::io::Error),
    #[error("Could not read from file")]
    Read(#[source] std::io::Error),
}

pub enum StepOptions {
    YesOrAbort,
}

/// One step in the install process. Can be executed or described.
#[allow(clippy::module_name_repetitions)]
pub trait InstallStep {
    /// A short (one line) description of what running perform will
    /// do. Pass in the tense you want for the description (past, present or
    /// future)
    fn describe(&self, tense: Tense) -> String;
    /// A verbose description of what running perform will do to the
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
    /// Is this a question and if so what options does the user have for responding?
    fn options(&self) -> Option<StepOptions> {
        Some(StepOptions::YesOrAbort)
    }
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
    #[error("Could not get crontab, needed tot filter out our added lines")]
    GetCrontab(
        #[from]
        #[source]
        init::cron::GetCrontabError,
    ),
    #[error(transparent)]
    CrontabChanged(#[from] init::cron::teardown::CrontabChanged),
    #[error("Could not set crontab, needed tot filter out our added lines")]
    SetCrontab(
        #[from]
        #[source]
        init::cron::SetCrontabError,
    ),
    #[error("Could not remove file(s), error")]
    DeleteError(
        #[from]
        #[source]
        files::DeleteError,
    ),
    #[error("Something went wrong interacting with systemd")]
    Systemd(
        #[from]
        #[source]
        init::systemd::Error,
    ),
}

/// One step in the remove process. Can be executed or described.
pub trait RemoveStep {
    /// A short (one line) description of what this step will do to the
    /// system. Pass in the tense you want for the description (past, present
    /// or future)
    fn describe(&self, tense: Tense) -> String;
    /// A verbose description of what this step will do to the
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
    #[error("Could not remove file, error")]
    Removing(
        #[from]
        #[source]
        RemoveError,
    ),
    #[error("error restoring file permissions")]
    RestoringPermissions(#[source] std::io::Error),
    #[error("error re-enabling service")]
    ReEnabling(
        #[from]
        #[source]
        systemd::Error,
    ),
    #[error("Can not rollback setting up cron, must be done manually")]
    Impossible,
    #[error("Crontab changed undoing changes might overwrite the change")]
    CrontabChanged(
        #[from]
        #[source]
        CrontabChanged,
    ),
    #[error("Could not get the crontab, needed to undo a change to it")]
    GetCrontab(
        #[from]
        #[source]
        GetCrontabError,
    ),
    #[error("Could not revert to the original crontab")]
    SetCrontab(
        #[from]
        #[source]
        SetCrontabError,
    ),
    #[error("Could not restore original file")]
    MovingBack(#[source] MoveBackError),
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
    pub fn install(self) -> Result<String, InstallError> {
        let mut description = Vec::new();
        for mut step in self.0 {
            description.push(step.describe(Tense::Past));
            step.perform()?;
        }

        Ok(description.join("\n"))
    }
}

impl<T: ToAssign> Spec<builder::PathIsSet, builder::NameIsSet, builder::TriggerIsSet, T> {
    /// Prepare for installing. This makes a number of checks and if they are
    /// passed it returns the [`InstallSteps`]. These implement [`IntoIterator`] and
    /// can be inspected and executed one by one or executed in one step using
    /// [`InstallSteps::install`].
    ///
    /// # Errors
    /// Returns an error if:
    ///  - the install is set to be system wide install while not running as admin/superuser.
    ///  - the service should run as another user then the current one while not running as admin/superuser.
    ///  - the service should run for a non-existing user.
    ///  - no suitable install directory could be found.
    ///  - the path for the executable does not point to a file.
    pub fn prepare_install(self) -> Result<InstallSteps, PrepareInstallError> {
        let builder::Spec {
            mode,
            path: Some(source),
            service_name: Some(name),
            bin_name,
            args,
            environment,
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

        let init_systems = self.init_systems.unwrap_or_else(init::System::all);
        let (mut steps, exe_path) = files::move_files(
            source,
            mode,
            run_as.as_deref(),
            overwrite_existing,
            &init_systems,
        )?;
        let params = init::Params {
            name,
            bin_name,
            description,

            exe_path,
            exe_args: args,
            environment,
            working_dir,

            trigger,
            run_as,
            mode,
        };

        let mut errors = Vec::new();
        for init in init_systems {
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
    pub fn remove(self) -> Result<String, RemoveError> {
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

impl<M: ToAssign, P: ToAssign, T: ToAssign, I: ToAssign> Spec<M, P, T, I> {
    /// Prepare for removing an install. This makes a number of checks and if
    /// they are passed it returns the [`RemoveSteps`]. These implement
    /// [`IntoIterator`] and can be inspected and executed one by one or
    /// executed in one step using [`RemoveSteps::remove`].
    ///
    /// # Errors
    /// Returns an error if:
    ///  - trying to remove a system install while not running as admin/superuser.
    ///  - no install is found.
    ///  - anything goes wrong setting up the removal.
    pub fn prepare_remove(self) -> Result<RemoveSteps, PrepareRemoveError> {
        let builder::Spec {
            mode,
            bin_name,
            run_as,
            ..
        } = self;

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

            if let Some(install) = init.tear_down_steps(bin_name, mode, run_as.as_deref())? {
                break install;
            }
        };

        let remove_step = files::remove_files(path);
        steps.push(Box::new(remove_step));
        Ok(RemoveSteps(steps))
    }
}
