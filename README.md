# bygge - Build your project.

What if you could build a Rust project without Cargo[^1]?
Bygge can.

> **bygge** [v]. (Danish, Norwegian Bokmål)
>
> 1. to build, construct,
> 2. to craft
>
> (via [Wiktionary])

[^1]: `bygge` still requires cargo for fetching dependencies.

## Why?

I wanted to understand all the work cargo does, without needing to dissect cargo itself.
I followed the output of `cargo build --verbose` and turned that into simple build instructions.

Additionally I've been reading about build systems, such as [Ninja][ninja-essay]
and was intrigued to rebuild that. I haven't done that yet, but at least I'm using Ninja.

## What it does

`bygge create` generates a Ninja build configuration (in `build.ninja` by default),
listing all the targets a binary crate depends on, including all crate dependencies.
`ninja` can then take this configuration and assemble the final binary.
The result should be about the same as an invocation of `cargo build`.

## What it doesn't

`bygge` is and never will be an alternative to Cargo.

Cargo is a full-fledged build system, aware of different build targets, allowing to enable features per dependency, easily cross-compile to different targets and run the built programs as well as tests and generate documentation.

`bygge` ... builds.

## Features

* Builds itself
* Builds cargo dependencies as listed in a project's Cargo.toml
* Can build only crates with a single binary target
* Runs on (at least) macOS and Linux
* No support for `build.rs` files
* No support for linking non-Rust libraries

## Requirements

* [Rust]
* [Ninja] v1.10.0

## Build `bygge`

`bygge` can create a Ninja build configuration to build itself.
But first you need a compiled `bygge`.
Use the bundled pre-generated configuration for that:

```
ninja -f manual.ninja
```

Then create the default build configuration and build:

```
build/bygge create
build/bygge build
```

## License

bygge is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

[Rust]: https://www.rust-lang.org/
[ninja]: https://ninja-build.org/
[wiktionary]: https://en.wiktionary.org/wiki/bygge
[ninja-essay]: https://www.aosabook.org/en/posa/ninja.html
