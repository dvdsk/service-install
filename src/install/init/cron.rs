use std::path::PathBuf;

use crate::install::Mode;

use super::{Params, SetupError, Steps, System, TearDownError};

pub struct Cron;

impl System for Cron {
    fn name(&self) -> &'static str {
        "cron"
    }
    fn not_available(&self) -> Result<bool, SetupError> {
        todo!()
    }
    fn set_up_steps(&self, _params: &Params) -> Result<Steps, SetupError> {
        todo!()
    }
    fn tear_down_steps(&self, _params: &str, _: Mode) -> Result<(Steps, PathBuf), TearDownError> {
        todo!()
    }
}
