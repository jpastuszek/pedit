[![Latest Version]][crates.io] [![Documentation]][docs.rs] ![License]

pedit
=====

`pedit` is a command line utility that helps with automation of editing configuration files.

Features
--------

*   Edits are idempotent which makes the tool suitable for use in administration script and systems like Puppet or Chef.
*   Ensure line in a text file is present or absent.
*   Ensure key-value pair in a text file is present or absent.
*   Key-value pairs can also be defined multiple times with different values (`--multikey`).
*   Support for relative placement of lines or key-value pairs in respect to existing lines in the text file.
*   Regular expressions are used for matching values in the files.
*   Check mode in which the tool will signal with exit status if change was required without performing any changes.
*   Show changes applied or would be applied in diff style.
*   Tested on MacOS as well as Windows.

Example usage
-------------

Ensure that `ssh_config` file contains key `StrictHostKeyChecking` set to value `yes`; if the key is absent put the pair before line containing `UserKnownHostsFile`.

	pedit --in-place ~/.ssh/ssh_config --diff line-pair --separator " " "StrictHostKeyChecking yes" present relative-to "UserKnownHostsFile" before

Installation
------------

	cargo install pedit

[crates.io]: https://crates.io/crates/pedit
[Latest Version]: https://img.shields.io/crates/v/pedit.svg
[Documentation]: https://docs.rs/pedit/badge.svg
[docs.rs]: https://docs.rs/pedit
[License]: https://img.shields.io/crates/l/pedit.svg
