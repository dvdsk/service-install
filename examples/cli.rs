use service_install::{Install, Schedule};
use time::Time;

fn main() {
    let schedule = Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    Install::system()
        .current_exe()
        .unwrap()
        .name("cli")
        .on_schedule(schedule)
        .install()
        .unwrap();

    Install::system()
        .name("cli")
        .remove()
}
