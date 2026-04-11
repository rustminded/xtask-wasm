# xtask-wasm

[![actions status][actions-badge]][actions-url]
[![crate version][crates-version-badge]][crates-url]
[![documentation][docs-badge]][docs-url]
[![dependencies status][deps-badge]][deps-url]
![licenses][licenses-badge]

[actions-badge]: https://github.com/rustminded/xtask-wasm/actions/workflows/rust.yml/badge.svg
[actions-url]: https://github.com/rustminded/xtask-wasm/actions
[crates-version-badge]: https://img.shields.io/crates/v/xtask-wasm
[crates-url]: https://crates.io/crates/xtask-wasm
[docs-badge]: https://docs.rs/xtask-wasm/badge.svg
[docs-url]: https://docs.rs/xtask-wasm/
[deps-badge]: https://deps.rs/repo/github/rustminded/xtask-wasm/status.svg
[deps-url]: https://deps.rs/repo/github/rustminded/xtask-wasm
[licenses-badge]: https://img.shields.io/crates/l/xtask-wasm

<!-- cargo-rdme start -->

This crate aims to provide an easy and customizable way to help you build
Wasm projects by extending them with custom subcommands, based on the
[`xtask` concept](https://github.com/matklad/cargo-xtask/), instead of using
external tooling like [`wasm-pack`](https://github.com/rustwasm/wasm-pack).

## Why xtask-wasm?

### No external tools to install

`wasm-pack` and `trunk` are separate binaries that must be installed outside
of Cargo — via `cargo install`, a shell script, or a system package manager.
This means every contributor and every CI machine needs an extra installation
step, and there is no built-in guarantee that everyone is running the same
version.

With xtask-wasm, `cargo xtask` is all you need. The build tooling is a
regular Cargo dependency, versioned in your `Cargo.lock` and reproduced
exactly like every other dependency in your project.

### `wasm-bindgen` version is always in sync

This is the most common source of pain with `wasm-pack` and `trunk`: the
`wasm-bindgen` CLI tool version must exactly match the `wasm-bindgen` library
version declared in your `Cargo.toml`. When they drift — after a `cargo
update`, a fresh clone, or a CI cache invalidation — you get a cryptic error
at runtime rather than a clear compile-time failure.

xtask-wasm uses [`wasm-bindgen-cli-support`](https://crates.io/crates/wasm-bindgen-cli-support)
as a library dependency. The version is pinned in your `Cargo.lock` alongside
your `wasm-bindgen` library dependency and kept in sync automatically — no
manual version matching required.

### Fully customizable

Because the build process is plain Rust code living inside your workspace,
you can extend, replace or wrap any step. `wasm-pack` and `trunk` are
opaque binaries driven by configuration files; xtask-wasm gives you the full
build logic as code, under your control.

## Setup

The best way to add xtask-wasm to your project is to create a workspace
with two packages: your project's package and the xtask package.

### Create a project using xtask

* Create a new directory that will contains the two package of your project
  and the workspace's `Cargo.toml`:

  ```console
  mkdir my-project
  cd my-project
  touch Cargo.toml
  ```

* Create the project package and the xtask package using `cargo new`:

  ```console
  cargo new my-project
  cargo new xtask
  ```

* Open the workspace's `Cargo.toml` and add the following:

  ```toml
  [workspace]
  default-members = ["my-project"]
  members = [
      "my-project",
      "xtask",
  ]
  resolver = "2"
  ```

* Create a `.cargo/config.toml` file and add the following content:

  ```toml
  [alias]
  xtask = "run --package xtask --"
  ```

The directory layout should look like this:

```console
project
├── .cargo
│   └── config.toml
├── Cargo.toml
├── my-project
│   ├── Cargo.toml
│   └── src
│       └── ...
└── xtask
    ├── Cargo.toml
    └── src
        └── main.rs
```

And now you can run your xtask package using:

```console
cargo xtask
```

You can find more informations about xtask
[here](https://github.com/matklad/cargo-xtask/).

### Use xtask-wasm as a dependency

Finally, add `xtask-wasm` to your dependencies:

```console
cargo add -p xtask xtask-wasm
```

## Usage

This library gives you three structs:

* [`Dist`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.Dist.html) - Generate a distributed package for Wasm.
* [`Watch`](https://docs.rs/xtask-watch/latest/xtask_watch/struct.Watch.html) -
  Re-run a given command when changes are detected
  (using [xtask-watch](https://github.com/rustminded/xtask-watch)).
* [`DevServer`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.DevServer.html) - Serve your project at a given IP address.

They all implement [`clap::Parser`](https://docs.rs/clap/latest/clap/trait.Parser.html)
allowing them to be added easily to an existing CLI implementation and are
flexible enough to be customized for most use-cases.

The pre and post hooks of [`DevServer`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.DevServer.html)
accept any type implementing the
[`Hook`](https://docs.rs/xtask-wasm/latest/xtask_wasm/trait.Hook.html) trait.
This lets you construct a [`process::Command`](https://doc.rust-lang.org/std/process/struct.Command.html) based on the server's final configuration
— for example, to pass the resolved `dist_dir` or `port` as arguments to an external tool.
A blanket implementation is provided for [`process::Command`](https://doc.rust-lang.org/std/process/struct.Command.html) itself, so no changes are
needed for simple use-cases.

Asset files copied by [`Dist`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.Dist.html)
can be processed by types implementing the
[`Transformer`](https://docs.rs/xtask-wasm/latest/xtask_wasm/trait.Transformer.html) trait.
Transformers are tried in order for each file; the first to return `Ok(true)` claims the file,
while unclaimed files are copied verbatim. When the `sass` feature is enabled,
[`SassTransformer`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.SassTransformer.html)
is available to compile SASS/SCSS files to CSS.

You can find further information for each type at their documentation level.

## Examples

### A basic implementation

```rust
use std::process::Command;
use xtask_wasm::{anyhow::Result, clap};

#[derive(clap::Parser)]
enum Opt {
    Dist(xtask_wasm::Dist),
    Watch(xtask_wasm::Watch),
    Start(xtask_wasm::DevServer),
}


fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let opt: Opt = clap::Parser::parse();

    match opt {
        Opt::Dist(dist) => {
            log::info!("Generating package...");

            dist
                .assets_dir("my-project/assets")
                .app_name("my-project")
                .build("my-project")?;
        }
        Opt::Watch(watch) => {
            log::info!("Watching for changes and check...");

            let mut command = Command::new("cargo");
            command.arg("check");

            watch.run(command)?;
        }
        Opt::Start(dev_server) => {
            log::info!("Starting the development server...");

            dev_server
                .xtask("dist")
                .start()?;
        }
    }

    Ok(())
}
```

Note: this basic implementation uses `env_logger` and `log`. Add them to the `Cargo.toml` of
your `xtask` (or use your preferred logger).

### [`examples/demo`](https://github.com/rustminded/xtask-wasm/tree/main/examples/demo)

Provides a basic implementation of xtask-wasm to generate the web app
package, an "hello world" app using [Yew](https://yew.rs/). This example
demonstrates a simple directory layout and a customized dist process
that use the `wasm-opt` feature.

The available subcommands are:

* Build the web app package.

  ```console
  cargo xtask dist
  ```
  * Build the web app package, download the
    [`wasm-opt`](https://github.com/WebAssembly/binaryen#tools)
    binary (currently using the 123 version) and optimize the Wasm generated by the dist
    process.

    ```console
    cargo xtask dist --optimize
    ```

* Build the web app package and watch for changes in the workspace root.

  ```console
  cargo xtask watch
  ```

* Serve an optimized web app dist on `127.0.0.1:8000` and watch for
  changes in the workspace root.

  ```console
  cargo xtask start
  ```

Additional flags can be found using `cargo xtask <subcommand> --help`.

This example also demonstrates the use of the `run-example` feature that allows you to use the
following:

```console
cargo run --example run_example
```

This command will run the code in `examples/run_example` using the development server.

## Features

* `wasm-opt`: enable the
  [`WasmOpt`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.WasmOpt.html) struct that helps
  downloading and using [`wasm-opt`](https://github.com/WebAssembly/binaryen#tools) very
  easily.
* `run-example`: a helper to run examples from `examples/` directory using a development
  server.
* `sass`: enable SASS/SCSS compilation via [`SassTransformer`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.SassTransformer.html).
  Add it to your [`Dist`](https://docs.rs/xtask-wasm/latest/xtask_wasm/struct.Dist.html) with `.transformer(SassTransformer::default())`.

## Troubleshooting

When using the re-export of [`clap`](https://docs.rs/clap/latest/clap), you
might encounter this error:

```console
error[E0433]: failed to resolve: use of undeclared crate or module `clap`
 --> xtask/src/main.rs:4:10
  |
4 | #[derive(Parser)]
  |          ^^^^^^ use of undeclared crate or module `clap`
  |
  = note: this error originates in the derive macro `Parser` (in Nightly builds, run with -Z macro-backtrace for more info)
```

This occurs because you need to import clap in the scope too. This error can
be resolved like this:

```rust
use xtask_wasm::clap;

#[derive(clap::Parser)]
struct MyStruct {}
```

Or like this:

```rust
use xtask_wasm::{clap, clap::Parser};

#[derive(Parser)]
struct MyStruct {}
```

<!-- cargo-rdme end -->
