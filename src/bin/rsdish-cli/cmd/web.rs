use clap::Args;
use rsdish::web::{WebOptions, serve};

#[derive(Debug, Args)]
#[command(about = "Serve the rsdish web UI.")]
pub struct WebCmd {
    #[arg(long, default_value = "127.0.0.1", help = "Address to bind.")]
    pub host: String,

    #[arg(short, long, default_value_t = 8787, help = "Port to bind.")]
    pub port: u16,

    #[arg(long, help = "Open the web UI in the default browser.")]
    pub open: bool,
}

pub fn handle_web(cmd: WebCmd) {
    let options = WebOptions {
        host: cmd.host,
        port: cmd.port,
        open: cmd.open,
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("Failed to start async runtime: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = runtime.block_on(serve(options)) {
        eprintln!("{:#}", e);
        std::process::exit(1);
    }
}
