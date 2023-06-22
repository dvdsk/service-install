use std::thread;
use std::time::Duration;

use trail_hamster::Container;

#[test]
fn ls_matches_read_dir() {
    let mut container = Container::run("systemd");
    thread::sleep(Duration::from_millis(250));
    container.check().unwrap();

    let ls_output = container.command("ls").output().unwrap().stdout;
    let ls_output = String::from_utf8(ls_output).unwrap();
    println!("{ls_output}");

    // let files = container.fs().unwrap().open_file(path)
}

#[test]
fn call_fs_multiple_times() {
    let container = Container::run("systemd");
    let fs1 = container.fs();
    let fs2 = container.fs();
}
