# rustagit

[stagit](https://git.codemadness.org/stagit/) is a static git page generator.

rustagit is a reimplementation of it in Rust.

rustagit is available under MIT License, just like stagit.

## Features

 - [x] commit log in HTML
 - [x] browsable files tree
 - [x] syntax highlighting in files thanks to [syntect](https://lib.rs/crates/syntect)
 - [x] extraction of .git/description and .git/url
 - [ ] list of branches and tags
 - [ ] quick link to README and LICENSE
 - [ ] generator of common index page for all repositories
 - [ ] commit log in RSS/Atom
 - [ ] line numbers in files
 - [ ] nice styling

## How to use

```shell
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --git https://git.hinata.iscute.ovh/rustagit/ --branch main
rustagit ./repository ./directory-to-put-files-in
```
