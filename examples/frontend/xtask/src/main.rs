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
    build_command.args(["run", "--example", "xtask", "--", "build"]);

    let crate_name = "frontend";
    let static_dir = "examples/frontend/static";
    let build_dir = "examples/frontend/build";

    let opt = Opt::from_args();

    match opt.cmd {
        Command::Build(arg) => {
            arg.execute(crate_name, static_dir, build_dir)?;
        }
        Command::StartServer(arg) => {
            arg.serve(build_dir)?;
        }
        Command::Serve(arg) => {
            arg.watch(build_dir, build_command)?;
        }
        Command::Watch(arg) => {
            arg.execute(build_dir, build_command)?;
        }
    }

    Ok(())
}
