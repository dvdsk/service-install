> ** Easily provide users an install method**


[![Crates.io](https://img.shields.io/crates/v/service-install?style=flat-square)](https://crates.io/crates/dbstruct)
[![Crates.io](https://img.shields.io/crates/d/service-install?style=flat-square)](https://crates.io/crates/dbstruct)
[![API](https://docs.rs/service-install/badge.svg)](https://docs.rs/dbstruct)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

**Note this is an early release, there might be bugs**

I would love some help to find them though!

### Features
 - Install the service to run on boot or a schedule
 - Perform the install step by step or in one go
 - Print each step or all at once (or make a tui/prompt!)
 - Roll back on failure
 - Configure the location or let us find a suitable one on the system
 - Uses systemd or cron
 - Specify which user the service should run as
 - Undo de installation tearing down the service and removing the files

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
        .name("cli")
        .on_schedule(schedule)
        .prepare_install()
        .unwrap()
        .install()
        .unwrap();
}
```
For more detailed examples (such as a working Tui/Prompt) see 

### Future work
 - Offer a pre build TUI/Prompt. That would allow the end user to go through the
   install interactively.
 - Windows support (could use some help here, not a big windows user myself)

### Contribution
Please let me know if there is anything you would like to see! PR's and issues
are very welcome!
