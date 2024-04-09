use std::path::PathBuf;

pub mod cron;
pub mod systemd;
pub(crate) mod extract_path;

pub use systemd::FindExeError;

use crate::install::RemoveStep;

use super::builder::Trigger;
use super::files::NoHomeError;
use super::{Mode, InstallStep};

type Steps = Vec<Box<dyn InstallStep>>;
type RSteps = Vec<Box<dyn RemoveStep>>;

/// Allowed init systems, set using: [`install::Spec::allowed_inits()`](super::Spec::allowed_inits)
#[derive(Debug, Clone)]
pub enum System {
    Systemd,
    Cron,
}

type ExeLocation = PathBuf;
impl System {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            System::Systemd => "Systemd",
            System::Cron => "Cron",
        }
    }
    pub(crate) fn not_available(&self) -> Result<bool, SetupError> {
        match self {
            System::Systemd => systemd::not_available(),
            System::Cron => Ok(cron::not_available()),
        }
    }
    pub(crate) fn set_up_steps(&self, params: &Params) -> Result<Steps, SetupError> {
        match self {
            System::Systemd => systemd::set_up_steps(params),
            System::Cron => cron::set_up_steps(params),
        }
    }
    pub(crate) fn tear_down_steps(
        &self,
        name: &str,
        bin_name: &str,
        mode: Mode,
        user: Option<&str>,
    ) -> Result<Option<(RSteps, ExeLocation)>, TearDownError> {
        match self {
            System::Systemd => systemd::tear_down_steps(name, mode),
            System::Cron => cron::tear_down_steps(bin_name, mode, user),
        }
    }

    pub(crate) fn all() -> Vec<System> {
        vec![Self::Systemd, Self::Cron]
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SetupError {
    #[error("systemd specific error: {0}")]
    Systemd(#[from] systemd::Error),
    #[error("Error while setting up crontab rule: {0}")]
    Cron(#[from] cron::setup::Error),
    #[error("could not find current users home dir")]
    NoHome(#[from] NoHomeError),
}

#[derive(thiserror::Error, Debug)]
pub enum TearDownError {
    #[error("Cron specific error: {0}")]
    Cron(#[from] cron::teardown::Error),
    #[error("Error while setting up systemd service: {0}")]
    Systemd(#[from] systemd::Error),
    #[error("Could not find current users home dir")]
    NoHome(#[from] NoHomeError),
    #[error("No service file while there is a timer file")]
    TimerWithoutService,
    #[error("Could not find path to executable: {0}")]
    FindingExePath(#[from] FindExeError),
}

#[derive(Debug, Clone)]
pub(crate) struct Params {
    pub(crate) name: String,
    pub(crate) bin_name: &'static str,
    pub(crate) description: Option<String>,

    pub(crate) exe_path: PathBuf,
    pub(crate) exe_args: Vec<String>,
    pub(crate) working_dir: Option<PathBuf>,

    pub(crate) trigger: Trigger,
    pub(crate) run_as: Option<String>,
    pub(crate) mode: Mode,
}

impl Params {
    fn description(&self) -> String {
        self.description
            .clone()
            .unwrap_or_else(|| format!("starts {}", self.name))
    }
}

const COMMENT_PREAMBLE: &str = "# created by: ";
const COMMENT_SUFFIX: &str = " during its installation\n# might get removed by it in the future.\n# Remove this comment to prevent that";

fn autogenerated_comment(bin_name: &str) -> String {
    format!("{COMMENT_PREAMBLE}'{bin_name}'{COMMENT_SUFFIX}")
}

trait EscapedPath {
    fn shell_escaped(&self) -> String;
}

impl EscapedPath for std::path::PathBuf {
    fn shell_escaped(&self) -> String {
        let path = self.display().to_string();
        let path = std::borrow::Cow::Owned(path);
        let path = shell_escape::unix::escape(path);
        path.into_owned()
    }
}

impl EscapedPath for &std::path::Path {
    fn shell_escaped(&self) -> String {
        let path = self.display().to_string();
        let path = std::borrow::Cow::Owned(path);
        let path = shell_escape::unix::escape(path);
        path.into_owned()
    }
}

impl EscapedPath for String {
    fn shell_escaped(&self) -> String {
        let s = std::borrow::Cow::Borrowed(self.as_str());
        let s = shell_escape::unix::escape(s);
        s.into_owned()
    }
}
