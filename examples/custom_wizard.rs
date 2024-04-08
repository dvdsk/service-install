use dialoguer::Confirm;
use service_install::{install_user, Tense};

fn main() {
    let steps = install_user!()
        .current_exe()
        .unwrap()
        .name("cli")
        .on_boot()
        .prepare_install()
        .unwrap();

    let mut canceld = false;
    let mut rollback_steps = Vec::new();
    for mut step in steps {
        if !Confirm::new()
            .with_prompt(format!("{}?", step.describe(Tense::Question)))
            .interact()
            .unwrap()
        {
            canceld = true;
            break;
        }
        if let Some(rollback) = step.perform().unwrap() {
            rollback_steps.push(rollback);
        }
    }

    if !canceld {
        return;
    }

    if rollback_steps.is_empty() {
        println!("Install aborted, no changes have been made");
        return;
    } else {
        if Confirm::new()
            .with_prompt("Install aborted, do you want to roll back any changes made?")
            .interact()
            .unwrap()
        {
            for step in &mut rollback_steps {
                let did = step.describe();
                step.perform().unwrap();
                println!("{}", did);
            }
        }
    }
}
