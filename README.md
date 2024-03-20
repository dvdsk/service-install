Easily provide users an install method on Linux systems. 

**Note this is an early release, it is only suitable for use in my own projects
right now**

### Features
 - Install the service to run on boot or a schedule
 - Finds suitable install location on the system
 - Specify user to run service as
 - Changes the owner of installed binary if needed
 - Undo de installation tearing down the service and removing the files

### Future work
 - For now we only support systemd, the lib is setup such that supporting
   other init or schedualling systems (like cron) should be easy.
 - In the future I would like to offer a TUI/Prompt. That would allow the user
   to go through the install interactively.
