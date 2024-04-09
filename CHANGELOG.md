# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Tense Question is now called Active

### Added 
- Added `best_effort_remove` function. A version of `remove` that continues on
  errors and returns what failed and the why (the error).

# Version 0.2.0 (2023-10-23)

### Changed
- Tense now also has the option Question which will turn the step descriptions
  into questions.

### Added 
- Pre made Text ui install wizard

### Fixed
- The target location not being available because there is already a file with
  the same name will now be caught during install preparation.
