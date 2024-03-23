#![doc= include_str!("../README.md")]

// offer user and system install
// place files somewhere that makes sense
// build the unit files
// enable/disable
// remove unit files

mod error;
mod install;
mod schedule;
mod system;
mod user;

pub use install::Install;
pub use install::InstallError;
pub use install::Step;
pub use install::Tense;

pub use schedule::Schedule;
