use service_install::{install_system, schedule::Schedule};
use time::Time;

fn main() {
    let schedule = Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    install_system!()
        .current_exe()
        .unwrap()
        .name("cli")
        .on_schedule(schedule)
        .run_as("david")
        .prepare_install()
        .unwrap()
        .install()
        .unwrap();

    install_system!()
        .name("cli")
        .prepare_remove()
        .unwrap()
        .remove()
        .unwrap();
}
