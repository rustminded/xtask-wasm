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
    Dist(xtask_wasm::Dist),
    Watch(xtask_wasm::Watch),
    Start(xtask_wasm::DevServer),
}

fn main() -> Result<()> {
    let opt: Opt = clap::Parser::parse();

    env_logger::builder()
        .filter(Some("xtask"), opt.log_level)
        .init();

    match opt.cmd {
        Command::Dist(arg) => {
            log::info!("Generating package...");

            arg.static_dir_path("webapp/static")
                .app_name("web_app")
                .run("webapp")?;

        }
        Command::Watch(arg) => {
            log::info!("Watching for changes and check...");

            let mut command = process::Command::new("cargo");
            command.arg("check");

            arg.run(command)?;
        }
        Command::Start(arg) => {
            log::info!("Starting the development server...");

            arg.arg("dist").start(xtask_wasm::default_dist_dir(false))?;
        }
    }

    Ok(())
}
