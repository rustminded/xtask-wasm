use anyhow::Result;
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt)]
enum Command {
    Build(xtask_wasm::Build),
    StartServer(xtask_wasm::DevServer),
    Serve(xtask_wasm::DevServer),
    Watch(xtask_wasm::Watch),
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter(Some("xtask"), log::LevelFilter::Trace)
        .init();

    let mut build_command = process::Command::new("cargo");
    build_command.args(["xtask", "build"]);

    let crate_name = "demo-webapp";
    let static_dir = "../demo-webapp/static";
    let build_dir = "build";

    let opt = Opt::from_args();

    match opt.cmd {
        Command::Build(arg) => {
            log::trace!("Building into {}", build_dir);
            arg.execute(crate_name, static_dir, build_dir)?;
            log::trace!("Builded");
        }
        Command::StartServer(arg) => {
            log::trace!("Starting dev server");
            arg.serve(build_dir)?;
            log::trace!("Shutting down dev server");
        }
        Command::Serve(arg) => {
            log::trace!("Starting to serve");
            arg.watch(build_dir, build_command)?;
            log::trace!("Serve stopped");
        }
        Command::Watch(arg) => {
            log::trace!("Starting to watch");
            arg.execute(build_dir, build_command)?;
            log::trace!("Watch stopped");
        }
    }

    Ok(())
}
