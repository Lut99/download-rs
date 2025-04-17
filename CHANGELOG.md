# Changelog for `download-rs`
This file keeps track of notable changes to the crate.

`download-rs` uses [semantic versioning](https://semver.org). As such, breaking changes are highlighted whenever they occur.


## 1.0.0 - 2025-04-17
### Added
- Feature flags for controlling `reqwest`'s SSL behaviour.

### Changed
- `reqwest` now does not enable SSL by default **(BREAKING)**.
- Bumped various dependencies to latest releases.

### Fixed
- Using `random` instead of `getrandom` in tests.


## 0.1.0 - 2024-03-12
Initial release!

### Added
- Functions to download files using standard HTTP GET-requests (`download_file()` and `download_file_async()`).
- Functions to unarchive `.tar.gz`-files (`tar::archive()`, `tar::archive_async()`, `tar::unarchive()` and `tar::unarchive_async()`).
- Optional `log`ging support.
- Features to control the enabled content in the library (`download`, `archives`, `tar`, `log`, `async`, `async-tokio`).
