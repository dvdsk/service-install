mod builder;
mod files;
pub mod init;

use std::fmt::Display;

pub use builder::InstallSpec;

use self::builder::ToAssign;

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    User,
    System,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::User => f.write_str("user"),
            Mode::System => f.write_str("system"),
        }
    }
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
    #[error("Install configured to run as a user: `{0}` however this user does not exist")]
    UserDoesNotExists(String),
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
    NoInstallFound,
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
    fn describe_detailed(&self, tense: Tense) -> String {
        self.describe(tense)
    }
    fn perform(&mut self) -> Result<Option<Box<dyn Rollback>>, Box<dyn std::error::Error>>;
}

pub trait RemoveStep {
    fn describe(&self, tense: Tense) -> String;
    fn describe_detailed(&self, tense: Tense) -> String {
        self.describe(tense)
    }
    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait Rollback {
    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn describe(&self) -> String;
}

impl<T: RemoveStep> Rollback for T {
    fn perform(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.perform()
    }

    fn describe(&self) -> String {
        self.describe(Tense::Past)
    }
}

pub struct InstallSteps(pub Vec<Box<dyn Step>>);

impl IntoIterator for InstallSteps {
    type Item = Box<dyn Step>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl InstallSteps {
    pub fn install(self) -> Result<String, Box<dyn std::error::Error>> {
        let mut description = Vec::new();
        for mut step in self.0 {
            description.push(step.describe(Tense::Past));
            step.perform()?;
        }

        Ok(description.join("\n"))
    }
}

impl<T: ToAssign> InstallSpec<builder::Set, builder::Set, builder::Set, T> {
    pub fn prepare_install(self) -> Result<InstallSteps, InstallError> {
        let builder::InstallSpec {
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

        if let Some(ref user) = run_as {
            if !crate::util::user_exists(&user).unwrap_or(true) {
                return Err(InstallError::UserDoesNotExists(user.clone()));
            }
        }

        let (mut steps, exe_path) = files::move_files(source, mode)?;
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

        for init in self
            .init_systems
            .unwrap_or_else(|| init::System::all())
            .into_iter()
        {
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

pub struct RemoveSteps(pub Vec<Box<dyn RemoveStep>>);

impl IntoIterator for RemoveSteps {
    type Item = Box<dyn RemoveStep>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl RemoveSteps {
    pub fn remove(self) -> Result<String, Box<dyn std::error::Error>> {
        let mut description = Vec::new();
        for mut step in self.0 {
            description.push(step.describe(Tense::Past));
            step.perform()?;
        }

        Ok(description.join("\n"))
    }
}

impl<P: ToAssign, T: ToAssign, I: ToAssign> InstallSpec<P, builder::Set, T, I> {
    pub fn prepare_remove(self) -> Result<RemoveSteps, RemoveError> {
        let builder::InstallSpec {
            mode,
            name: Some(name),
            bin_name,
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

        let mut inits = self.init_systems.unwrap_or(init::System::all()).into_iter();
        let (mut steps, path) = loop {
            let Some(init) = inits.next() else {
                return Err(RemoveError::NoInstallFound);
            };

            if let Some(install) = init.tear_down_steps(&name, bin_name, mode)? {
                break install;
            }
        };

        let remove_step = files::remove_files(path);
        steps.push(Box::new(remove_step));
        Ok(RemoveSteps(steps))
    }
}
