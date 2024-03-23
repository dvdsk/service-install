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
    install_user!()
        .current_exe()
        .unwrap()
        .name("cli")
        .on_schedule(schedule)
        .install()
        .unwrap();

    install_user!().name("cli").remove().unwrap();
}
