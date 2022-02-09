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
    Dist(Build),
    Watch(xtask_wasm::Watch),
    Start(xtask_wasm::DevServer),
}

#[derive(clap::Parser)]
struct Build {
    #[clap(long)]
    optimize: bool,

    #[clap(flatten)]
    base: xtask_wasm::Dist,
}

fn main() -> Result<()> {
    let opt: Opt = clap::Parser::parse();

    env_logger::builder()
        .filter(Some("xtask"), opt.log_level)
        .init();

    match opt.cmd {
        Command::Dist(arg) => {
            log::info!("Generating package...");

            let dist_result = arg
                .base
                .static_dir_path("webapp/static")
                .app_name("web_app")
                .run("webapp")?;

            if arg.optimize {
                xtask_wasm::WasmOpt::level(1)
                    .shrink(2)
                    .optimize(dist_result.wasm)?;
            }
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
