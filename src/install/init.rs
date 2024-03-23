mod cron;
mod systemd;

use std::path::PathBuf;

pub use cron::Cron;

use self::systemd::FindExeError;

use super::builder::Trigger;
use super::files::NoHomeError;
use super::{Mode, Step};

type Steps = Vec<Box<dyn Step>>;

pub trait System {
    fn name(&self) -> &'static str;
    fn not_available(&self) -> Result<bool, SetupError>;
    fn set_up_steps(&self, params: &Params) -> Result<Steps, SetupError>;
    fn tear_down_steps(&self, name: &str, mode: Mode) -> Result<(Steps, PathBuf), TearDownError>;
}

#[derive(thiserror::Error, Debug)]
pub enum SetupError {
    #[error("systemd specific error: {0}")]
    Systemd(#[from] systemd::Error),
    #[error("could not find current users home dir")]
    NoHome(#[from] NoHomeError),
}

#[derive(thiserror::Error, Debug)]
pub enum TearDownError {
    #[error("systemd specific error: {0}")]
    Systemd(#[from] systemd::Error),
    #[error("could not find current users home dir")]
    NoHome(#[from] NoHomeError),
    #[error("no service file therefore could not figure out where the binary is")]
    NoService,
    #[error("Could not find path to executable: {0}")]
    FindingExePath(#[from] FindExeError),
}

#[derive(Debug, Clone)]
pub struct Params {
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
