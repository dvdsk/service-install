mod builder;
mod init;
mod files;

pub use builder::Install;

use self::builder::ToAssign;

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    User,
    System,
}

#[derive(thiserror::Error, Debug)]
pub enum InstallError {
    #[error("Error setting up init: {0}")]
    Init(#[from] init::SetupError),
    #[error("Failed to move files: {0}")]
    Move(#[from] files::MoveError),
    #[error("Need to run as root to install to system")]
    NeedRoot,
    #[error("Could not find an init system we can set things up for")]
    NoInitSystemRecognized,
}

#[derive(thiserror::Error, Debug)]
pub enum RemoveError {
    #[error("Could not find this executable's location: {0}")]
    GetExeLocation(std::io::Error),
    #[error("Failed to remove files: {0}")]
    Move(#[from] files::DeleteError),
    #[error("Removing from init system: {0}")]
    Init(#[from] init::TearDownError),
    #[error("Could not find any installation in any init system")]
    NotInUse,
    #[error("Need to run as root to remove a system install")]
    NeedRoot,
}

#[derive(thiserror::Error, Debug)]
pub enum FindInstallError {}

/// Changes when in action takes place in the Step::describe
/// function.
pub enum Tense {
    Past,
    Present,
    Future,
}

pub trait Step {
    fn describe(&self, tense: Tense) -> String;
    fn perform(self) -> Result<(), Box<dyn std::error::Error>>;
}

const INIT_SYSTEMS: [&dyn init::System; 1] = [/* &Systemd {}, */ &init::Cron {}];
pub struct InstallSteps(pub Vec<Box<dyn Step>>);

impl<T: ToAssign> Install<builder::Set, builder::Set, builder::Set, T> {
    pub fn prepare_install(self) -> Result<InstallSteps, InstallError> {
        let builder::Install {
            mode,
            path: Some(source),
            name: Some(name),
            bin_name,
            args,
            trigger: Some(trigger),
            working_dir,
            run_as,
            description,
            ..
        } = self
        else {
            unreachable!("type sys guarantees path, name and trigger set")
        };

        if let Mode::System = mode {
            if let sudo::RunningAs::User = sudo::check() {
                return Err(InstallError::NeedRoot);
            }
        }

        let move_step = files::move_files(source, mode)?;
        let exe_path = move_step.target.clone();

        let mut steps = vec![Box::new(move_step) as Box<dyn Step>];

        let params = init::Params {
            name,
            bin_name,
            description,

            exe_path,
            exe_args: args,
            working_dir,

            trigger,
            run_as,
            mode,
        };

        for init in INIT_SYSTEMS {
            if init.not_available().map_err(InstallError::Init)? {
                continue;
            }

            match init.set_up_steps(&params) {
                Ok(init_steps) => {
                    steps.extend(init_steps);
                    return Ok(InstallSteps(steps));
                }
                Err(error) => {
                    tracing::warn!("Could set up init using {}, error: {error}", init.name())
                }
            };
        }

        Err(InstallError::NoInitSystemRecognized)
    }
}

pub struct RemoveSteps(pub Vec<Box<dyn Step>>);
impl<P: ToAssign, T: ToAssign, I: ToAssign> Install<P, builder::Set, T, I> {
    pub fn prepare_remove(self) -> Result<RemoveSteps, RemoveError> {
        let builder::Install {
            mode,
            name: Some(name),
            ..
        } = self
        else {
            unreachable!("type sys guarantees name and trigger set")
        };

        if let Mode::System = mode {
            if let sudo::RunningAs::User = sudo::check() {
                return Err(RemoveError::NeedRoot);
            }
        }

        let mut inits = INIT_SYSTEMS.iter();
        let (mut steps, path) = loop {
            let Some(init) = inits.next() else {
                return Err(RemoveError::NotInUse)
            };

            break init.tear_down_steps(&name, mode)?;
        };

        let remove_step = files::remove_files(path);
        steps.push(Box::new(remove_step));
        Ok(RemoveSteps(steps))
    }
}

