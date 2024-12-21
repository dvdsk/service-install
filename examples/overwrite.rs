use std::time::Duration;
use std::{env, thread};

use service_install::schedule::Schedule;
use service_install::Tense;
use service_install::{install::init, install_user};

fn cleanup() {
    // cleanup
    let steps = install_user!().service_name("cli").prepare_remove().unwrap();
    for mut step in steps {
        println!("{}", step.describe_detailed(Tense::Questioning));
        step.perform().unwrap();
    }
    println!("Remove complete")
}

fn install(schedule: Option<Schedule>) {
    let spec = install_user!()
        .current_exe()
        .unwrap()
        .service_name("cli")
        .arg("--simulate-service")
        .allowed_inits(&[init::System::Systemd])
        .overwrite_existing(true);

    let spec = if let Some(schedule) = schedule {
        spec.on_schedule(schedule)
    } else {
        spec.on_boot()
    };
    let steps = spec.prepare_install().unwrap();

    for mut step in steps {
        println!("{}", step.describe_detailed(Tense::Active));
        step.perform().unwrap();
    }
}

fn main() {
    if env::args()
        .skip(1)
        .next()
        .is_some_and(|arg| arg == "--simulate-service")
    {
        thread::sleep(Duration::from_secs(920));
        return;
    }

    // let soon = time::OffsetDateTime::now_local().unwrap().time() + Duration::from_secs(65);
    let soon = time::OffsetDateTime::now_local().unwrap().time() + Duration::from_secs(5);

    install(Some(Schedule::Daily(soon)));
    println!("Install 1 complete");
    println!("Sleeping for 65 seconds to allow the service to start\n\n");
    // thread::sleep(Duration::from_secs(5));
    thread::sleep(Duration::from_secs(20));
    install(None);
    println!("Install 2 complete\n\n");

    thread::sleep(Duration::from_secs(30));
    cleanup()
}
