use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use service_install::{install_user, Schedule};
use time::Time;

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive(LevelFilter::WARN.into()))
        .init();

    let schedule = Schedule::Daily(Time::from_hms(10, 10, 10).unwrap());
    let done = install_user!()
        .current_exe()
        .unwrap()
        .name("cli")
        .on_schedule(schedule)
        .prepare_install()
        .unwrap()
        .install()
        .unwrap();

    println!("Install complete, did: \n{done}");

    let done = install_user!()
        .name("cli")
        .prepare_remove()
        .unwrap()
        .remove()
        .unwrap();

    println!("Remove complete, did: \n{done}")
}
