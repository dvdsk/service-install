#![doc= include_str!("../README.md")]

// offer user and system install
// place files somewhere that makes sense
// build the unit files
// enable/disable
// remove unit files

mod error;
mod install;
mod schedule;

pub use install::InstallSpec;
pub use install::InstallError;
pub use install::{Step, RemoveStep};
pub use install::Tense;
pub use install::init::System as InitSystem;

pub use schedule::Schedule;
