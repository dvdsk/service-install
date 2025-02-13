# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## Version 0.5.6 - 2025-02-13

### Changed
- `install` and `remove` play better with error wrapping libs as they return concrete 
error types now
- we now check if an already existing file is identical byte for byte to what
  we are trying to install
- `Spec` and `Scheduale` are now also available at the crate root.

### Added
- `env_vars` and `env_var` to install builder. They allow setting environmental
  variables.
- Documentation to all install spec methods including examples

## Version 0.5.5 - 2025-01-07

### Changed
- No longer replaces the binary file if its already installed and we are
running from it.
- Restart service if one already existed
- Rollback restores any previous file if where was one 

## Version 0.5.4 - 2025-01-06

### Added
- Offers to stop running processes taking up the install location

## Version 0.5.3 - 2024-12-23

### Changed
- no longer formatting source of error in an error's display implementation.
  Instead relying on `std::error::Error::source`. Use color-eyre/anyhow to
  display the error chain.
### Fixed
- rollback was performed in the wrong order
- `prepare_remove` no longer crashes when service name was not provided

## Version 0.5.2 - 2024-12-22
### Fixed
- `prepare_remove` was broken in 0.5.0

## Version 0.5.1 - 2024-12-22
YANKED (bad release)

## Version 0.5.0 - 2024-12-20
### Changed
- Install spec builder's `name` member renamed to `service_name` to highlight
  that it sets the name for the con job or systemd service and not the
  executable that is installed

## Version 0.4.4 - 2024-12-20
### Changed
- Made error returned by install and removal implement Send+Sync+'static. This makes it easily to use with error crates such as eyre and anyhow

## Version 0.4.3 - 2024-04-21

### Fixed
- Make the executable readable and executable but remove all others if readonly
  is set

## Version 0.4.2 - 2024-04-21

### Changed
- When overwrite is enabled and a file taking up the install location is being
  ran by a service the service is stopped and disabled. If the service was
  created by us previously it is removed too.

## Version 0.4.1 - 2024-04-15

### Fixed
- All errors now print the underlying issue when it is known

## Version 0.4.0 - 2024-04-11

### Added
- New `overwrite_exiting` option on `install::Spec`. Default is false, by
  setting it to true the installer will overwrite existing executables.

## Version 0.3.0 - 2024-04-9

### Changed
- Tense Question is now called Active
- Removed all `Box<dyn Error>` in favor of a tree of enum errors

### Added 
- Added `best_effort_remove` function. A version of `remove` that continues on
  errors and returns what failed and the why (the error).

## Version 0.2.0 - 2024-04-2

### Changed
- Tense now also has the option Question which will turn the step descriptions
  into questions.

### Added 
- Pre made Text ui install wizard

### Fixed
- The target location not being available because there is already a file with
  the same name will now be caught during install preparation.
