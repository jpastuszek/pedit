[![Latest Version]][crates.io] [![Documentation]][docs.rs] ![License]

`pedit` is command line utility that helps with editing configuration files.

Features:
* Operations of this tool are idempotent which makes it suitable for use in administration script or systems like Puppet or Chef.
* Ensure line in a text file is present or absent.
* Ensure key-value pair in a text file is present or absent.
* Key-value pairs can also be defined multiple times with different value (`--multikey`) or single key-value pair will be managed by default.
* Relative placement of lines or key-value pairs in respect to existing lines in the text file.
* By setting `--check` switch the tool will signal if change would have been required with exit status code.
* By setting `--diff` switch the tool will show changes applied.

Example usage
=====

Ensure that `ssh_config` file contains `StrictHostKeyChecking` set to `yes` and if it is absent put it before line containing `User`.

```sh
pedit -i ~/.ssh/ssh_config --diff --check line-pair -s " " -m "StrictHostKeyChecking yes" present relative-to "User" before
```

[crates.io]: https://crates.io/crates/pedit
[Latest Version]: https://img.shields.io/crates/v/pedit.svg
[Documentation]: https://docs.rs/pedit/badge.svg
[docs.rs]: https://docs.rs/pedit
[License]: https://img.shields.io/crates/l/pedit.svg
