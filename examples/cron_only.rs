use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use service_install::{install_user, InitSystem, Schedule, Tense};
use time::Time;

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive(LevelFilter::WARN.into()))
        .init();

    let schedule = Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    let steps = install_user!()
        .current_exe()
        .unwrap()
        .name("cli")
        .on_schedule(schedule)
        .allowed_inits(&[InitSystem::Cron])
        .prepare_install()
        .unwrap();

    for mut step in steps {
        println!("{}", step.describe_detailed(Tense::Present));
        step.perform().unwrap();
    }
    println!("Install complete");

    let steps = install_user!().name("cli").prepare_remove().unwrap();

    for mut step in steps {
        println!("{}", step.describe_detailed(Tense::Present));
        step.perform().unwrap();
    }
    println!("Remove complete")
}
