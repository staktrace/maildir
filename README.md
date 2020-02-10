maildir
===
[![Build Status](https://travis-ci.org/staktrace/maildir.svg?branch=master)](https://travis-ci.org/staktrace/maildir)
[![Crate](https://img.shields.io/crates/v/maildir.svg)](https://crates.io/crates/maildir)

A simple library to deal with maildir folders

API
---
The primary entry point for this library is the Maildir structure, which can be created from a path, like so:

```rust
    let maildir = Maildir::from("path/to/maildir");
```

The Maildir structure then has functions that can be used to access and modify mail files.

Documentation
---
See the rustdoc at [docs.rs](https://docs.rs/maildir/).
