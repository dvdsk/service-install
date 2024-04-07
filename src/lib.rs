#![doc= include_str!("../README.md")]

/// Changes the tense of the string returned by the `describe` functions for
/// [InstallStep](install::InstallStep), [RemoveStep](install::RemoveStep) and
/// [Rollback](install::RollbackStep).
pub enum Tense {
    Past,
    Present,
    Future,
}

/// Installation (or removal) configuration, steps and errors.
pub mod install;
/// Scheduling options
pub mod schedule;
