use std::thread;
use std::time::Duration;

use trail_hamster::Container;

#[test]
fn install() {
    let mut container = Container::run("systemd");
    thread::sleep(Duration::from_millis(250));
    container.check().unwrap();

    dbg!(container.exec(&["ls", "-la"]));

}
