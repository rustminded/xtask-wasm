use anyhow::Result;
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, default_value = "Info")]
    log: log::LevelFilter,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt)]
enum Command {
    Build(xtask_wasm::Build),
    Watch(xtask_wasm::Watch),
    Serve(xtask_wasm::DevServer),
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    env_logger::builder().filter(Some("xtask"), opt.log).init();

    let mut build_command = process::Command::new("cargo");
    build_command.args(["xtask", "build"]);

    let crate_name = "demo-webapp";
    let static_dir = "demo-webapp/static";

    let opt = Opt::from_args();

    match opt.cmd {
        Command::Build(arg) => {
            log::info!("Starting to build");
            arg.execute(crate_name, static_dir)?;
        },
        Command::Watch(mut arg) => {
            log::info!("Starting to watch");
            arg.execute(build_command)?;
        },
        Command::Serve(arg) => {
            log::info!("Starting to serve");
            arg.serve_and_watch(build_command)?;
        },
    }

    Ok(())
}
