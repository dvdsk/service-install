use crate::install::InstallSteps;
use crate::install::RollbackStep;
use crate::Tense;

use dialoguer::Confirm;
use dialoguer::Select;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("canceld by the user")]
    Canceld,
    #[error("could not get input from the user: {0}")]
    UserInputFailed(#[from] dialoguer::Error),
    #[error("ran into one or more errors, user chose to abort")]
    AbortedAfterError(Vec<Box<dyn std::error::Error>>),
    #[error("user chose to cancel and rollback however rollback failed: {0}")]
    RollbackFollowingCancel(Box<dyn std::error::Error>),
    #[error(
        "ran into error user chose to abort and rollback however rollback failed: {0}"
    )]
    RollbackFollowingError(Box<dyn std::error::Error>),
}

/// Start an interactive installation wizard using the provided [install
/// steps](InstallSteps). This wizard will ask the user to confirm each of the
/// step. If anything goes wrong the user will be prompted if they wish to
/// abort, abort and try to roll back the changes made or continue.
///
/// # Errors
/// This returns an error if the user canceld the removal, something
/// went wrong getting user input or anything during the removal failed.
///
/// In that last case either [`AbortedAfterError`](Error::AbortedAfterError),
/// [`RollbackFollowingError`](Error::RollbackFollowingError) or
/// [`RollbackFollowingCancel`](Error::RollbackFollowingCancel) is returned
/// depending on whether: the user aborted after the error, a rollback failed
/// was started after an install error but the rollback failed *or* happened
/// during install or a rollback was started after the user canceld but it
/// failed.
pub fn start(steps: InstallSteps) -> Result<(), Error> {
    let mut errors = Vec::new();
    let mut rollback_steps = Vec::new();
    for mut step in steps {
        println!("{}", step.describe(Tense::Question));
        if !Confirm::new().interact()? {
            rollback_if_user_wants_to(rollback_steps)?;
            return Err(Error::Canceld);
        }

        match step.perform() {
            Ok(None) => (),
            Ok(Some(rollback)) => rollback_steps.push(rollback),
            Err(e) => {
                let details = e.to_string().replace('\n', "\n\t");
                errors.push(e);

                println!("An error occurred, details:\n\t{details}\t");
                match Select::new()
                    .with_prompt("What do you want to do?")
                    .items(&["rollback and abort", "abort", "continue"])
                    .default(0)
                    .interact()?
                {
                    2 => continue,
                    0 => rollback(rollback_steps).map_err(Error::RollbackFollowingError)?,
                    _ => (),
                }
                return Err(Error::AbortedAfterError(errors));
            }
        }
    }

    Ok(())
}

fn rollback_if_user_wants_to(rollback_steps: Vec<Box<dyn RollbackStep>>) -> Result<(), Error> {
    if rollback_steps.is_empty() {
        println!("Install aborted, no changes have been made");
    } else if Confirm::new()
        .with_prompt("Install aborted, do you want to roll back any changes made?")
        .interact()?
    {
        rollback(rollback_steps).map_err(Error::RollbackFollowingCancel)?;
    }

    Ok(())
}

fn rollback(
    mut rollback_steps: Vec<Box<dyn RollbackStep>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for step in &mut rollback_steps {
        let did = step.describe(Tense::Past);
        step.perform()?;
        println!("{did}");
    }
    Ok(())
}