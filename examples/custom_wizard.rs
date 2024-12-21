use dialoguer::Confirm;
use service_install::{install_user, Tense};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let steps = install_user!()
        .current_exe()?
        .service_name("cli")
        .on_boot()
        .prepare_install()?;

    let mut canceld = false;
    let mut rollback_steps = Vec::new();
    for mut step in steps {
        if !Confirm::new()
            .with_prompt(format!("{}?", step.describe(Tense::Active)))
            .interact()?
        {
            canceld = true;
            break;
        }
        if let Some(rollback) = step.perform()? {
            rollback_steps.push(rollback);
        }
    }

    if !canceld {
        return Ok(());
    }

    if rollback_steps.is_empty() {
        println!("Install aborted, no changes have been made");
        return Ok(());
    } else {
        if Confirm::new()
            .with_prompt("Install aborted, do you want to roll back any changes made?")
            .interact()?
        {
            for step in &mut rollback_steps {
                let did = step.describe(Tense::Questioning);
                step.perform()?;
                println!("{}", did);
            }
        }
    }

    // lets remove the install to prevent polluting the system
    install_user!().service_name("cli").prepare_remove()?.best_effort_remove()?;
    Ok(())
}
