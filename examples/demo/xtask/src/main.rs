use anyhow::Result;
use clap::Parser;
use std::process;

#[derive(Parser)]
struct Opt {
    #[clap(long = "log", default_value = "Info")]
    log_level: log::LevelFilter,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Parser)]
enum Command {
    Build(Build),
    Watch(xtask_wasm::Watch),
    Serve(xtask_wasm::DevServer),
}

#[derive(Parser)]
struct Build {
    #[clap(long)]
    optimize: bool,

    #[clap(flatten)]
    base: xtask_wasm::Build,
}

fn main() -> Result<()> {
    let opt = Opt::parse();

    env_logger::builder()
        .filter(Some("xtask"), opt.log_level)
        .init();

    let mut build_command = process::Command::new("cargo");
    build_command.args(["xtask", "build"]);

    match opt.cmd {
        Command::Build(mut arg) => {
            log::info!("Starting to build");
            arg.base.static_dir_path("demo-webapp/static");
            let build_dir = arg.base.run("demo-webapp")?;
            if arg.optimize {
                xtask_wasm::wasm_opt(build_dir.join("app_bg.wasm"), 0, 0, true)?;
            }
        }
        Command::Watch(arg) => {
            log::info!("Starting to watch");
            arg.run(build_command)?;
        }
        Command::Serve(mut arg) => {
            log::info!("Starting to serve");
            arg.command(build_command);
            arg.start(xtask_wasm::default_build_dir(false))?;
        }
    }

    Ok(())
}
