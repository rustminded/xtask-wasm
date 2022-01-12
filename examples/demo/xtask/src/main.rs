use anyhow::Result;
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    #[structopt(long = "log", default_value = "Info")]
    log_level: log::LevelFilter,
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

    env_logger::builder().filter(Some("xtask"), opt.log_level).init();

    let mut build_command = process::Command::new("cargo");
    build_command.args(["xtask", "build"]);

    match opt.cmd {
        Command::Build(arg) => {
            log::info!("Starting to build");
            arg.execute("demo-webapp", "demo-webapp/static")?;
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
