# download-rs
Collection of Rust functions for downloading files and archives conveniently.

Concretely, offers the following functionality:
- Download files using standard HTTP GET-requests (`download_file()` and `download_file_async()`).
- (Un)archive `.tar.gz`-files (`tar::archive()`, `tar::archive_async()`, `tar::unarchive()` and `tar::unarchive_async()`).


## Installation
To use this crate in your Rust project, simply add it to your `Cargo.toml`-file as a `git`-dependency:
```toml
[dependencies]
download = { git = "https://github.com/Lut99/download-rs" }
```

You can optionally fix yourself to a particular tag, e.g.,
```toml
[dependencies]
download = { git = "https://github.com/Lut99/download-rs", tag = "v0.1.0" }
```

To enable additional features (see [below](#features)), you can combine the above with the `features`-flag.


## Usage
The functions in this crate are documented using docstrings. As such, you can learn about their behaviour and programmetic interface by auto-generating docs:
```bash
cargo doc --no-deps --open --all-features
```

If you're using [rust-analyzer](https://rust-analyzer.github.io/), the docstrings are also parsed and available in IntelliSense.


## Features
This crate supports the following features:
- _Functionality_
    - `download` _(default)_: Enables the toplevel download functions (`download_file()`, `download_file_async()` and associated structures)
    - `tar`: Enables the tarball-related archive/unarchival functions in the `tar`-module (`tar::archive()`, `tar::archive_async()`, `tar::unarchive()`, `tar::unarchive_async()` and associated structures).
    - `log`: Enables printing [`log`](https://docs.rs/log/latest/log/)-statements in functions in the library.
    - `async-tokio`: Enables async versions of functions in the library that use [`tokio`](https://tokio.rs/) as a backend.
- _Aliases_
    - `async`: Enables the "default" backend (`async-tokio`).
    - `archives`: Enables the `tar`-feature.


## Changelog
For more information about version history, consult the [CHANGELOG.md](./CHANGELOG.md) file.


## Contribution
If you want to contribute to this project, welcome! Feel free to [raise an issue](https://github.com/Lut99/download-rs/issues) or [create a pull request](https://github.com/Lut99/download-rs/pulls). Note that this project is maintained by a single developer for hobby purposes, so response may be a little slow.


## Licence
This project is licensed under the Apache 2.0 license. See [LICENSE](./LICENSE) for more information.
