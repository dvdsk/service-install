#![doc= include_str!("../README.md")]

/// Changes the tense of the string returned by the `describe` functions for
/// [`InstallStep`](install::InstallStep), [`RemoveStep`](install::RemoveStep) and
/// [Rollback](install::RollbackStep). Final punctuation is missing and must be added.
pub enum Tense {
    Past,
    Questioning,
    Future,
    Active,
}

impl Tense {
    pub fn punct(&self) -> &str {
        match self {
            Tense::Questioning => "?",
            Tense::Past | Tense::Future | Tense::Active => ".",
        }
    }
}

/// Installation (or removal) configuration, steps and errors.
pub mod install;
/// Scheduling options
pub mod schedule;
#[cfg(feature = "tui")]
/// A pre made basic TUI that functions as an install and removal wizard
pub mod tui;

