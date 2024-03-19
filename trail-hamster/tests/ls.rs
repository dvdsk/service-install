use std::collections::HashSet;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use trail_hamster::Container;

#[test]
fn ls_matches_read_dir() {
    setup_tracing();

    let mut container = Container::run("systemd");
    thread::sleep(Duration::from_millis(250));
    container.check().unwrap();

    let ls_output = container.command("ls").output().unwrap().stdout;
    let ls_output = String::from_utf8(ls_output).unwrap();
    let ls_files: HashSet<_> = ls_output.split_whitespace().map(PathBuf::from).collect();

    let fs = container.fs().unwrap();
    let dir = fs.read_dir("/root").unwrap();
    let read_dir_files: HashSet<_> = dir
        .into_iter()
        .map(Result::unwrap)
        .map(|e| e.path().into_incorrect_std_path())
        .collect();
    dbg!(read_dir_files);

    loop {}
    assert_eq!(ls_files, read_dir_files);
}

#[test]
fn call_fs_multiple_times() {
    let container = Container::run("systemd");
    let fs1 = container.fs();
    let fs2 = container.fs();
}

fn setup_tracing() {
    use tracing_subscriber::filter;
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;

    let filter = filter::EnvFilter::builder()
        .parse("trail-hamster=debug,info")
        .unwrap();

    let fmt = fmt::layer()
        .pretty()
        .with_line_number(true)
        .with_test_writer();

    let _ignore_err = tracing_subscriber::registry()
        .with(filter)
        .with(fmt)
        .try_init();
}
