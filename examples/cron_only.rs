use service_install::Tense;
use service_install::schedule::Schedule;

use service_install::{install_user, install::init};
use time::Time;

fn main() {
    let schedule = Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    let steps = install_user!()
        .current_exe()
        .unwrap()
        .service_name("cli")
        .on_schedule(schedule)
        .allowed_inits(&[init::System::Cron])
        .prepare_install()
        .unwrap();

    for mut step in steps {
        println!("{}", step.describe_detailed(Tense::Questioning));
        step.perform().unwrap();
    }
    println!("Install complete\n\n");

    let steps = install_user!().service_name("cli").prepare_remove().unwrap();

    for mut step in steps {
        println!("{}", step.describe_detailed(Tense::Questioning));
        step.perform().unwrap();
    }
    println!("Remove complete")
}
