use std::process;
use xtask_wasm::{anyhow::Result, clap};

#[derive(clap::Parser)]
struct Opt {
    #[clap(long = "log", default_value = "Info")]
    log_level: log::LevelFilter,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(clap::Parser)]
enum Command {
    Build(Build),
    Watch(xtask_wasm::Watch),
    Serve(xtask_wasm::DevServer),
}

#[derive(clap::Parser)]
struct Build {
    #[clap(long)]
    optimize: bool,

    #[clap(flatten)]
    base: xtask_wasm::Build,
}

fn main() -> Result<()> {
    let opt: Opt = clap::Parser::parse();

    env_logger::builder()
        .filter(Some("xtask"), opt.log_level)
        .init();

    let mut build_command = process::Command::new("cargo");
    build_command.args(["xtask", "build"]);

    match opt.cmd {
        Command::Build(arg) => {
            log::info!("Starting to build");
            let build_result = arg
                .base
                .static_dir_path("demo-webapp/static")
                .app_name("hello_world")
                .run("demo-webapp")?;
            if arg.optimize {
                xtask_wasm::WasmOpt::level(1)
                    .shrink(2)
                    .optimize(build_result.wasm)?;
            }
        }
        Command::Watch(arg) => {
            log::info!("Starting to watch");
            arg.run(build_command)?;
        }
        Command::Serve(arg) => {
            log::info!("Starting to serve");
            build_command.arg("--optimize");
            arg.command(build_command)
                .start(xtask_wasm::default_build_dir(false))?;
        }
    }

    Ok(())
}
