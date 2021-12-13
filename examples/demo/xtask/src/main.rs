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
    Watch(xtask_wasm::Watch),
    Serve(xtask_wasm::Serve),
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter(Some("xtask"), log::LevelFilter::Trace)
        .init();

    let mut build_command = process::Command::new("cargo");
    build_command.args(["xtask", "build"]);

    let crate_name = "demo-webapp";
    let static_dir = "demo-webapp/static";
    let build_dir = "build";

    let opt = Opt::from_args();

    match opt.cmd {
        Command::Build(arg) => {
            log::trace!("Building into {}", build_dir);
            arg.execute(crate_name, static_dir, build_dir)?;
            log::trace!("Builded");
        }
        Command::Watch(mut arg) => {
            log::trace!("Starting to watch");
            arg.execute(build_command)?;
        }
        Command::Serve(arg) => {
            log::trace!("Starting to serve");
            arg.execute(build_dir, build_command)?;
        }
    }

    Ok(())
}
