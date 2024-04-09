#![doc= include_str!("../README.md")]

/// Changes the tense of the string returned by the `describe` functions for
/// [`InstallStep`](install::InstallStep), [`RemoveStep`](install::RemoveStep) and
/// [Rollback](install::RollbackStep).
pub enum Tense {
    Past,
    Present,
    Future,
    Active,
}

#[cfg(feature = "tui")]
/// A pre made basic TUI that functions as an install and removal wizard
pub mod tui;
/// Installation (or removal) configuration, steps and errors.
pub mod install;
/// Scheduling options
pub mod schedule;
