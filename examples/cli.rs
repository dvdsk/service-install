use systemd_install::Install;
use time::Time;

fn main() {
    let schedule = systemd_install::Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    Install::system()
        .current_exe()
        .unwrap()
        .name("cli")
        .on_schedule(schedule)
        .perform()
        .unwrap();

    todo!("do testy stuff?")
}
