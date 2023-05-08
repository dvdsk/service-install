// offer user and system install
// place files somewhere that makes sense
// build the unit files
// enable/disable
// remove unit files

mod error;
mod install;
mod schedual;
mod system;
mod user;

pub use install::Error as InstallError;
pub use install::Install;

pub use schedual::Schedule;
