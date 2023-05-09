use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};

fn dockerfile_tag(image: &str) -> String {
    let cwd = env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = format!("{cwd}/tests/{image}.dockerfile");
    let mut hash_state = DefaultHasher::new();
    std::fs::read_to_string(&path)
        .unwrap()
        .hash(&mut hash_state);
    let hash = hash_state.finish();
    format!("{hash}")
}

fn build_image(image: &str) {
    let cwd = env::var("CARGO_MANIFEST_DIR").unwrap();
    let build_script = format!("{cwd}/tests/{image}.sh");
    let tag = dockerfile_tag(image);

    // Build the images used for all tests
    let output = Command::new("buildah")
        .arg("unshare")
        .arg(build_script)
        .arg("--force-rm")
        .arg("--tag") // takes argument <name>:<tag> .... yeah...
        .arg(format!("{image}:{tag}"))
        .arg(".")
        .output()
        .unwrap();

    if !output.status.success() {
        panic!("stderr: {}", String::from_utf8(output.stderr).unwrap());
    }
}

fn image_exists(image: &str) -> bool {
    #[derive(PartialEq, Eq)]
    struct Entry {
        repo: String,
        tag: String,
    }

    let output = Command::new("podman").arg("images").output().unwrap();
    if !output.status.success() {
        panic!("stderr: {}", String::from_utf8(output.stderr).unwrap());
    }

    let list = String::from_utf8(output.stdout).unwrap();
    let mut list = list.lines().map(str::split_whitespace).map(|mut w| Entry {
        repo: w.next().unwrap().to_string(),
        tag: w.next().unwrap().to_string(),
    });

    let tag = dockerfile_tag(image);
    list.any(|word| word.tag == tag)
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
            // needed to run systemd in container. Container will
            // still run as a user in the host system.
            .arg("--privileged") 
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
        println!("image outdated/missing, building from dockerfile");
        build_image("cli");
    }

    let container = Container::run("cli");
}
