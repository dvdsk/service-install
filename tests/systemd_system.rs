use std::env;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};

fn build_image(image: &str) {
    let cwd = env::var("CARGO_MANIFEST_DIR").unwrap();

    // Build the images used for all tests
    let output = Command::new("podman")
        .arg("build")
        .arg("--file")
        .arg(&format!("{cwd}/tests/{image}.dockerfile"))
        .arg("--force-rm")
        .arg("--tag")
        .arg("cli:latest")
        .arg(".")
        .output()
        .unwrap();

    if !output.status.success() {
        panic!("stderr: {}", String::from_utf8(output.stderr).unwrap());
    }
}

fn image_exists(image: &str) -> bool {
    let output = Command::new("podman").arg("images").output().unwrap();
    if !output.status.success() {
        panic!("stderr: {}", String::from_utf8(output.stderr).unwrap());
    }

    let list = String::from_utf8(output.stdout).unwrap();
    let mut list = list
        .lines()
        .map(str::split_whitespace)
        .filter_map(|mut w| w.nth(0));

    let image_id = format!("localhost/{image}");
    list.any(|word| word == image_id)
}

pub struct Container {
    name: String,
    handle: Child,
}

impl Container {
    fn run(image: &str) -> Self {
        static FREE_CONTAINER_ID: AtomicUsize = AtomicUsize::new(0);

        let container_id = FREE_CONTAINER_ID.fetch_add(1, Ordering::SeqCst);
        let name = format!("test_{}_{container_id}", env!("CARGO_PKG_NAME"));
        let image_id = format!("localhost/{image}");
        let handle = Command::new("podman")
            .arg("run")
            .arg(&image_id)
            .arg("--name")
            .arg(&name)
            .spawn()
            .unwrap();

        Self { name, handle }
    }

    // fn output(&mut self) -> String {
    //     self.handle.wait_with_output()
    // }
}

impl Drop for Container {
    fn drop(&mut self) {
        let output = Command::new("podman")
            .arg("stop")
            .arg(&self.name)
            .output()
            .unwrap();

        let no_container_err = output
            .stderr
            .starts_with(b"Error: no container with name or ID");
        if !output.status.success() && !no_container_err {
            panic!("stderr: {}", String::from_utf8(output.stderr).unwrap());
        }
        let output = Command::new("podman")
            .arg("rm")
            .arg(&self.name)
            .output()
            .unwrap();

        let no_container_err = output
            .stderr
            .starts_with(b"Error: no container with name or ID");
        if !output.status.success() && !no_container_err {
            panic!("stderr: {}", String::from_utf8(output.stderr).unwrap());
        }
    }
}

#[test]
fn test() {
    if !image_exists("cli") {
        build_image("cli");
    }

    let container = Container::run("cli");
}
