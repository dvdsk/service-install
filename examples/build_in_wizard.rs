use service_install::{install_user, schedule::Schedule, tui};
use time::Time;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schedule = Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    let steps = match install_user!()
        .current_exe()?
        .service_name("cli")
        .on_schedule(schedule)
        .prepare_install()
    {
        Err(e) => {
            eprintln!("Exiting, could not start install:\n\t{e}");
            return Ok(());
        }
        Ok(steps) => steps,
    };

    if let Err(e) = tui::install::start(steps, false) {
        eprintln!("Install failed: {e}")
    }

    // lets remove the install to prevent polluting the system
    install_user!().service_name("cli").prepare_remove()?.best_effort_remove()?;

    Ok(())
}
