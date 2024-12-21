use service_install::{install_user, schedule::Schedule};
use time::Time;

fn main() {
    let schedule = Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    let done = install_user!()
        .current_exe()
        .unwrap()
        .service_name("cli")
        .on_schedule(schedule)
        .prepare_install()
        .unwrap()
        .install()
        .unwrap();

    println!("Install complete, did: \n{done}");

    let done = install_user!()
        .service_name("cli")
        .prepare_remove()
        .unwrap()
        .remove()
        .unwrap();

    println!("Remove complete, did: \n{done}")
}
