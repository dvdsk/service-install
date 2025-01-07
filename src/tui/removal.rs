use dialoguer::Confirm;

use crate::install::{RemoveError, RemoveSteps};
use crate::Tense;

use dialoguer;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The removal was canceled by the user")]
    Canceled,
    #[error("User aborted removal after one or more errors happened, errors: {0:?}")]
    AbortedAfterError(Vec<RemoveError>),
    #[error("Removal done however one or more errors happened, errors: {0:?}")]
    CompletedWithErrors(Vec<RemoveError>),
    #[error("Could not get input from the user")]
    UserInputFailed(
        #[from]
        #[source]
        dialoguer::Error,
    ),
}

/// Start an interactive removal wizard using the provided [remove steps](RemoveSteps). This will ask
/// the user to confirm each of the step. If anything goes wrong the user will
/// be prompted if they wish to continue or abort.
///
/// # Errors
/// This returns an error if the user canceled the removal, something went wrong
/// getting user input or anything during the removal failed.
///
/// In that last case either [`AbortedAfterError`](Error::AbortedAfterError) or
/// [`CompletedWithErrors`](Error::CompletedWithErrors) is returned depending on
/// if the user aborted the removal of continued
pub fn start(steps: RemoveSteps) -> Result<(), Error> {
    let mut errors = Vec::new();
    for mut step in steps {
        if !Confirm::new()
            .with_prompt(format!("{}?", step.describe(Tense::Questioning)))
            .interact()?
        {
            return Err(Error::Canceled);
        }

        if let Err(e) = step.perform() {
            errors.push(e);
            if !Confirm::new()
                .with_prompt("Error happened during removal, do you want to try and continue?")
                .interact()?
            {
                return Err(Error::AbortedAfterError(errors));
            }
        }
    }

    if !errors.is_empty() {
        return Err(Error::CompletedWithErrors(errors));
    }

    Ok(())
}
