> ** Easily provide users an install method**

[![Crates.io](https://img.shields.io/crates/v/service-install?style=flat-square)](https://crates.io/crates/service-install)
[![Crates.io](https://img.shields.io/crates/d/service-install?style=flat-square)](https://crates.io/crates/service-install)
[![API](https://docs.rs/service-install/badge.svg)](https://docs.rs/service-install)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

**Note this is an early release, there might be bugs**

This crate provides the building blocks to build an installer for self contained
binaries without runtime dependencies. Such an installer provides less technical
users with an easy way to set up your program. It is not a full alternative for
integrating with a package manager. For example there is no way to provide
updates. Building your own installer is however significantly less work then
trying to get your application in all the linux package managers. It is also
ideal for tools that are not public. 

### Features
 - Set up a service to run the application on boot or a schedule
 - Perform the install step by step or in one go
 - Print each step or all at once (or make a tui/prompt!)
 - Roll back on failure
 - Configure the install location or find a suitable one automatically
 - Specify which user the service should run as
 - Undo the installation tearing down the service and removing the files

### Example
Installing the current program as a user service named *cli* that should run at
10:42 every day. This does not need superuser/admin permissions.

```rust,ignore
use service_install::{install_user, schedule::Schedule};
use time::Time;

fn main() {
    let schedule = Schedule::Daily(Time::from_hms(10, 42, 00).unwrap());
    let done = install_user!()
        .current_exe()
        .unwrap()
        .service_name("cli")
        .on_schedule(schedule)
        .prepare_install()
        .unwrap()
        .install()
        .unwrap();
}
```
For more detailed examples (such as a working Tui/Prompt) see 

### Future work
 - Make the pre build TUI/Prompt more customizable.
 - Windows support (could use some help here, not a big windows user myself)

### Contribution
Please let me know if there is anything you would like to see! PR's and issues
are very welcome!
